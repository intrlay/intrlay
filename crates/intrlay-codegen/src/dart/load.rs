//! Multi-step reads use locals, without closure initializers or repeated offsets.

use intrlay_schema::{Exposure, Scalar};

use crate::plan::{Id, Kind, TypeGraph, ViewCache};

pub(super) struct Read<'a> {
    pub id: Id,
    pub offset: &'a str,
    pub exposure: Exposure,
    pub tag: Option<(u32, &'a str)>,
}

/// Unit values and empty numeric regions do not consult the collection index.
pub(super) fn index_parameter(plan: &TypeGraph, id: Id, exposure: Exposure) -> &'static str {
    let read = Read {
        id,
        exposure,
        offset: "$[argument:index]",
        tag: None,
    };
    if body(plan, &read).contains("$[argument:index]") {
        "index"
    } else {
        "_index"
    }
}

pub(super) fn value(plan: &TypeGraph, read: &Read<'_>) -> Option<String> {
    let value = expression(plan, read.id, read.offset, read.exposure)?;
    Some(if let Some((tag, offset)) = read.tag {
        let value = wrapped(&value, usize::from(plan.nullable_value(read.id)));
        format!("this.$[storage].u32({offset}) == {tag} ? {value} : null")
    } else {
        value
    })
}

pub(super) fn body(plan: &TypeGraph, read: &Read<'_>) -> String {
    cached_body(plan, read, None)
}

pub(super) fn cached_body(plan: &TypeGraph, read: &Read<'_>, cache: Option<&ViewCache>) -> String {
    if cache.is_none()
        && let Some(value) = value(plan, read)
    {
        return finish(&value, cache, read.id);
    }
    let mut lines = Vec::new();
    if let Some((tag, offset)) = read.tag {
        lines.push(format!(
            "if (this.$[storage].u32({offset}) != {tag}) return null;"
        ));
    }
    let wrappers = usize::from(read.tag.is_some() && plan.nullable_value(read.id));
    if let Some(value) = expression(plan, read.id, read.offset, read.exposure) {
        lines.push(finish(&wrapped(&value, wrappers), cache, read.id));
    } else {
        lines.push(format!("var position = {};", read.offset));
        follow(plan, read.id, read.exposure, wrappers, cache, &mut lines);
    }
    lines.join("\n")
}

fn follow(
    plan: &TypeGraph,
    id: Id,
    exposure: Exposure,
    wrappers: usize,
    cache: Option<&ViewCache>,
    lines: &mut Vec<String>,
) {
    if let Some(value) = expression(plan, id, "position", exposure) {
        lines.push(finish(&wrapped(&value, wrappers), cache, id));
        return;
    }
    if !matches!(exposure, Exposure::TypedArray(_))
        && let Some(field) = plan.folded_field(id)
    {
        follow(
            plan,
            field.ty,
            field.options.exposure,
            wrappers,
            cache,
            lines,
        );
        return;
    }
    if let Exposure::TypedArray(scalar) = exposure {
        let Kind::Sequence(element) = plan.nodes[id].kind else {
            unreachable!("fixed numeric regions are expressions")
        };
        lines.push("final payloadOffset = position + this.$[storage].u32(position);".into());
        lines.push(format!(
            "final length = this.$[storage].u32(position + 4) * {};",
            plan.nodes[element].layout.size() / scalar.layout().size()
        ));
        lines.push(finish(
            &wrapped(&numeric(scalar, "payloadOffset", "length"), wrappers),
            cache,
            id,
        ));
        return;
    }
    match plan.nodes[id].kind {
        Kind::Boxed(value) => {
            lines.push("position += this.$[storage].u32(position);".into());
            follow(
                plan,
                value,
                plan.nodes[value].options.exposure,
                wrappers,
                cache,
                lines,
            );
        }
        Kind::Optional { value, offset } => {
            let absent = finish(&wrapped("null", wrappers), cache, id).replace('\n', "\n  ");
            lines.push(format!(
                "if (this.$[storage].bytes[position] == 0) {{\n  {absent}\n}}"
            ));
            lines.push(format!("position += {offset};"));
            follow(
                plan,
                value,
                plan.nodes[value].options.exposure,
                wrappers + usize::from(plan.nullable_value(value)),
                cache,
                lines,
            );
        }
        _ => unreachable!("only wrappers and numeric sequences need statements"),
    }
}

fn finish(value: &str, cache: Option<&ViewCache>, key: usize) -> String {
    if let Some(cache) = cache
        && let Some(state) = &cache.state
    {
        let field = &cache.value;
        format!(
            "if ({state} == {key}) return {field};\n{field} = {value};\n{state} = {key};\nreturn {field};"
        )
    } else {
        format!("return {value};")
    }
}

pub(super) fn expression(
    plan: &TypeGraph,
    id: Id,
    offset: &str,
    exposure: Exposure,
) -> Option<String> {
    if !matches!(exposure, Exposure::TypedArray(_))
        && let Some(field) = plan.folded_field(id)
    {
        return expression(plan, field.ty, offset, field.options.exposure);
    }
    let node = &plan.nodes[id];
    if let Exposure::TypedArray(scalar) = exposure {
        return if matches!(node.kind, Kind::Sequence(_)) {
            None
        } else {
            Some(numeric(
                scalar,
                offset,
                &(node.layout.size() / scalar.layout().size()).to_string(),
            ))
        };
    }
    Some(match node.kind {
        Kind::Scalar(kind) => scalar(kind, offset),
        Kind::Enum(_) if node.is_value_enum() => format!(
            "{}.{}(this.$[storage].u32({offset}))",
            node.tag_type_name(),
            super::names::enum_from_tag(plan)
        ),
        Kind::Unit => "null".into(),
        Kind::String => format!("this.$[storage].stringAt({offset})"),
        Kind::Optional { .. } | Kind::Boxed(_) => return None,
        _ => format!("{}._(this.$[storage], {offset})", plan.view_name(id)),
    })
}

pub(super) fn scalar(kind: Scalar, offset: &str) -> String {
    match kind {
        Scalar::Bool => format!("(this.$[storage].bytes[{offset}] != 0)"),
        Scalar::U8 => format!("this.$[storage].bytes[{offset}]"),
        Scalar::I8 => format!("this.$[storage].data.getInt8({offset})"),
        Scalar::I64 | Scalar::U64 => format!(
            "intrlay.read{}(this.$[storage].data, {offset})",
            super::method(kind)
        ),
        _ => format!(
            "this.$[storage].data.get{}({offset}, typed.Endian.little)",
            super::method(kind)
        ),
    }
}

fn numeric(scalar: Scalar, offset: &str, length: &str) -> String {
    // An empty projected record can have alignment one. No byte is borrowed,
    // but typed-data constructors still require an aligned absolute offset.
    let offset = if length == "0" {
        "0".into()
    } else if length == "length" {
        format!("length == 0 ? 0 : this.$[storage].bytes.offsetInBytes + ({offset})")
    } else {
        format!("this.$[storage].bytes.offsetInBytes + ({offset})")
    };
    if matches!(scalar, Scalar::I64 | Scalar::U64) {
        let helper = if scalar == Scalar::I64 {
            "int64View"
        } else {
            "uint64View"
        };
        return format!("intrlay.{helper}(this.$[storage].bytes.buffer, {offset}, {length})");
    }
    format!(
        "this.$[storage].bytes.buffer.as{}(\n  {offset},\n  {length},\n)",
        super::numeric_type(scalar).trim_start_matches("typed.")
    )
}

fn wrapped(value: &str, wrappers: usize) -> String {
    let mut value = value.to_owned();
    for _ in 0..wrappers {
        value = format!("intrlay.Some({value})");
    }
    value
}

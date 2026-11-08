//! Statement-based loads follow each relative pointer once, including in arrays.

use intrlay_schema::Exposure;

use crate::plan::{Id, Kind, TypeGraph, ViewCache};

pub(super) struct Read<'a> {
    pub id: Id,
    pub offset: &'a str,
    pub exposure: Exposure,
    pub tag: Option<u32>,
}

/// Unit values and empty numeric regions do not consult the collection index.
pub(super) fn index_parameter(plan: &TypeGraph, id: Id, exposure: Exposure) -> &'static str {
    let read = Read {
        id,
        exposure,
        offset: "$[argument:index]",
        tag: None,
    };
    if body(plan, &read, None).contains("$[argument:index]") {
        "index"
    } else {
        "_index"
    }
}

pub(super) fn body(plan: &TypeGraph, read: &Read<'_>, cache: Option<&ViewCache>) -> String {
    let mut lines = Vec::new();
    if let Some(cache) = cache.filter(|cache| cache.state.is_none()) {
        let cache = &cache.value;
        lines.push(format!(
            "if (this.{cache} !== intrlay.uninitialized) {{\n    return this.{cache};\n}}"
        ));
    }
    if let Some(tag) = read.tag {
        let absent = finish("undefined", 0, cache, usize::MAX).replace('\n', "\n    ");
        lines.push(format!(
            "if (this.$[storage].u32[this.$[offset] >>> 2] !== {tag}) {{\n    {absent}\n}}"
        ));
    }
    if let Some(value) = expression(plan, read.id, read.offset, read.exposure) {
        lines.push(finish(&value, 0, cache, read.id));
    } else {
        lines.push(format!("let position = {};", read.offset));
        follow(plan, read.id, read.exposure, 0, cache, &mut lines);
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
        lines.push(finish(&value, wrappers, cache, id));
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
        lines.push("const payloadOffset = position + this.$[storage].u32[position >>> 2]!;".into());
        lines.push(format!(
            "const length = this.$[storage].u32[(position >>> 2) + 1]! * {};",
            plan.nodes[element].layout.size() / scalar.layout().size()
        ));
        lines.push(finish(
            &numeric(scalar, "payloadOffset", "length"),
            wrappers,
            cache,
            id,
        ));
        return;
    }
    match plan.nodes[id].kind {
        Kind::Boxed(value) => {
            lines.push("position += this.$[storage].u32[position >>> 2]!;".into());
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
            let absent = finish("null", wrappers, cache, id).replace('\n', "\n    ");
            lines.push(format!(
                "if (this.$[storage].u8[position] === 0) {{\n    {absent}\n}}"
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

fn expression(plan: &TypeGraph, id: Id, offset: &str, exposure: Exposure) -> Option<String> {
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
        Kind::Scalar(scalar) => {
            let value = format!(
                "this.$[storage].{}[{}]!",
                super::storage_name(scalar),
                super::storage_index(scalar, offset)
            );
            if scalar == intrlay_schema::Scalar::Bool {
                format!("({value} !== 0)")
            } else {
                value
            }
        }
        Kind::Unit => "null".into(),
        Kind::String => format!("intrlay.readString(this.$[storage], {offset})"),
        Kind::Boxed(_) | Kind::Optional { .. } => return None,
        _ => format!("new {}(this.$[storage], {offset})", plan.view_name(id)),
    })
}

fn numeric(scalar: intrlay_schema::Scalar, offset: &str, length: &str) -> String {
    // A zero-sized projected region may be unaligned even though no bytes are
    // borrowed. TypedArray constructors still require an aligned byte offset.
    let offset = if length == "0" {
        "0".into()
    } else if length == "length" {
        format!("length === 0 ? 0 : this.$[storage].bytes.byteOffset + ({offset})")
    } else {
        format!("this.$[storage].bytes.byteOffset + ({offset})")
    };
    format!(
        "new {}(\n    this.$[storage].bytes.buffer,\n    {offset},\n    {length}\n)",
        super::array_type(scalar)
    )
}

fn finish(value: &str, wrappers: usize, cache: Option<&ViewCache>, key: usize) -> String {
    let mut value = value.to_owned();
    for _ in 0..wrappers {
        value = format!("{{ value: {value} }}");
    }
    if let Some(cache) = cache {
        let field = &cache.value;
        if let Some(state) = &cache.state {
            // Keys identify branches in this getter, not runtime object identities.
            let key = if key == usize::MAX {
                "-1".into()
            } else {
                key.to_string()
            };
            format!(
                "if (this.{state} === {key} && this.{field} !== intrlay.uninitialized) {{\n    return this.{field};\n}}\nthis.{field} = {value};\nthis.{state} = {key};\nreturn this.{field};"
            )
        } else {
            format!("this.{field} = {value};\nreturn this.{field};")
        }
    } else {
        format!("return {value};")
    }
}

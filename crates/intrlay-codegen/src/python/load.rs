//! Read bodies keep pointer traversal linear and preserve every optional state.

use intrlay_schema::Exposure;

use crate::plan::{Id, Kind, TypeGraph, ViewCache};

pub(super) struct Read<'a> {
    pub id: Id,
    pub offset: &'a str,
    pub exposure: Exposure,
    pub tag: Option<u32>,
}

pub(super) fn body(plan: &TypeGraph, read: &Read<'_>, cache: Option<&ViewCache>) -> String {
    let mut lines = Vec::new();
    if let Some(tag) = read.tag {
        lines.push(format!(
            "if self._storage.u32[self._offset // 4] != {tag}:\n    return None"
        ));
    }
    if let Some(cache) = cache.filter(|cache| cache.state.is_none()) {
        let cache = &cache.value;
        lines.push(format!(
            "if self.{cache} is not _intrlay.CacheState.UNINITIALIZED:\n    return self.{cache}"
        ));
    }
    let wrappers = usize::from(read.tag.is_some() && plan.nullable_value(read.id));
    if let Some(value) = expression(plan, read.id, read.offset, read.exposure) {
        lines.push(finish(&value, wrappers, cache, read.id));
    } else {
        lines.push(format!("position = {}", read.offset));
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
        let length = if let Kind::Sequence(element) = plan.nodes[id].kind {
            lines.push(format!(
                "length = self._storage.u32[position // 4 + 1] * {}",
                plan.nodes[element].layout.size() / scalar.layout().size()
            ));
            lines.push("position += self._storage.u32[position // 4]".into());
            "length".to_owned()
        } else {
            (plan.nodes[id].layout.size() / scalar.layout().size()).to_string()
        };
        let storage = super::scalar_access(scalar, "self._storage", "0");
        let storage = storage.split('[').next().unwrap_or(&storage);
        lines.push(format!("start = position // {}", scalar.layout().size()));
        lines.push(finish(
            &format!("{storage}[start:start + {length}]"),
            wrappers,
            cache,
            id,
        ));
        return;
    }
    match plan.nodes[id].kind {
        Kind::Boxed(value) => {
            lines.push("position += self._storage.u32[position // 4]".into());
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
            let absent = finish("None", wrappers, cache, id).replace('\n', "\n    ");
            lines.push(format!("if self._storage.u8[position] == 0:\n    {absent}"));
            lines.push(format!("position += {offset}"));
            follow(
                plan,
                value,
                plan.nodes[value].options.exposure,
                wrappers + usize::from(plan.nullable_value(value)),
                cache,
                lines,
            );
        }
        _ => unreachable!("only wrappers and numeric regions need statements"),
    }
}

pub(super) fn expression(
    plan: &TypeGraph,
    id: Id,
    offset: &str,
    exposure: Exposure,
) -> Option<String> {
    if let Exposure::TypedArray(_) = exposure {
        return None;
    }
    if let Some(field) = plan.folded_field(id) {
        return expression(plan, field.ty, offset, field.options.exposure);
    }
    Some(match plan.nodes[id].kind {
        Kind::Scalar(scalar) => {
            let value = super::scalar_access(scalar, "self._storage", offset);
            if scalar == intrlay_schema::Scalar::Bool {
                format!("({value} != 0)")
            } else {
                value
            }
        }
        Kind::Unit => "None".into(),
        Kind::String => format!("self._storage.string_at({offset})"),
        Kind::Optional { .. } | Kind::Boxed(_) => return None,
        _ => format!(
            "{}(self._storage, {offset})",
            super::names::reference(&plan.view_name(id))
        ),
    })
}

fn finish(value: &str, wrappers: usize, cache: Option<&ViewCache>, key: usize) -> String {
    let mut value = value.to_owned();
    for _ in 0..wrappers {
        value = format!("_intrlay.Some({value})");
    }
    if let Some(cache) = cache {
        let field = &cache.value;
        if let Some(state) = &cache.state {
            format!(
                "if self.{field} is not _intrlay.CacheState.UNINITIALIZED and self.{state} == {key}:\n    return self.{field}\nself.{field} = {value}\nself.{state} = {key}\nreturn self.{field}"
            )
        } else {
            format!("self.{field} = {value}\nreturn self.{field}")
        }
    } else {
        format!("return {value}")
    }
}

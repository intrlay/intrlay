//! Direct field helpers avoid allocating memoryview slices or child wrappers.

use std::fmt::Write;

use super::{documentation, scalar_type};
use crate::{
    GenerateError,
    api::FieldHelpers,
    plan::{ByteRange, Field, NumericRegion, RegionStep, TypeGraph},
};

pub(super) fn emit(
    output: &mut String,
    plan: &TypeGraph,
    api: &FieldHelpers,
    field: &Field,
    offset: &str,
    tag: Option<(u32, &str)>,
    indexed: bool,
) -> Result<(), GenerateError> {
    ranges(output, plan, api, field, offset, tag, indexed)?;
    let Some(region) = plan.numeric_region(field) else {
        return Ok(());
    };
    let index = if indexed {
        ", index: _builtins.int"
    } else {
        ""
    };
    let lane = format!(
        "self._storage.{}[position // {} + component]",
        format!("{:?}", region.scalar).to_lowercase(),
        region.scalar.layout().size()
    );
    let docs = FieldHelpers::component_docs(field, indexed);
    if let Some(name) = &api.component_getter {
        let optional = if region.optional() || tag.is_some() {
            " | None"
        } else {
            ""
        };
        writeln!(
            output,
            "\n    def {name}(self{index}, component: _builtins.int) -> {}{optional}:",
            scalar_type(region.scalar)
        )?;
        documentation::write(output, &docs, "        ")?;
        writeln!(
            output,
            "        {}return {lane}",
            position(&region, offset, tag).replace('\n', "\n        ")
        )?;
    }
    if let Some(name) = &api.component_setter {
        if api.component_getter.is_none() {
            writeln!(output)?;
        }
        writeln!(
            output,
            "    def {name}(self{index}, component: _builtins.int, value: {}) -> None:",
            scalar_type(region.scalar)
        )?;
        documentation::write(output, &docs, "        ")?;
        writeln!(
            output,
            "        {}{lane} = value",
            position(&region, offset, tag).replace('\n', "\n        ")
        )?;
    }
    Ok(())
}

fn ranges(
    output: &mut String,
    plan: &TypeGraph,
    api: &FieldHelpers,
    field: &Field,
    offset: &str,
    tag: Option<(u32, &str)>,
    indexed: bool,
) -> Result<(), GenerateError> {
    for (name, is_offset) in [
        (api.byte_offset.as_ref(), true),
        (api.byte_length.as_ref(), false),
    ] {
        if let Some(name) = name {
            let range = plan.byte_range(field);
            let guarded = matches!(range, ByteRange::Sequence { .. }) && tag.is_some();
            let uses_index =
                is_offset || guarded || matches!(range, ByteRange::Sequence { stride: 1.. });
            writeln!(output)?;
            if !indexed {
                writeln!(output, "    @_builtins.property")?;
            }
            let index = if indexed {
                ", index: _builtins.int"
            } else {
                ""
            };
            let unused = if indexed && !uses_index {
                "  # noqa: ARG002 - Keep the public index keyword for a fixed wire extent."
            } else {
                ""
            };
            let optional = if guarded { " | None" } else { "" };
            writeln!(
                output,
                "    def {name}(self{index}) -> _builtins.int{optional}:{unused}"
            )?;
            documentation::write(
                output,
                &FieldHelpers::range_docs(plan, field, indexed, is_offset),
                "        ",
            )?;
            if guarded && let Some((tag, tag_offset)) = tag {
                writeln!(
                    output,
                    "        if self._storage.u32[({tag_offset}) // 4] != {tag}:\n            return None"
                )?;
            }
            let body = match (range, is_offset) {
                (ByteRange::Inline { .. }, true) => format!("return {offset}"),
                (ByteRange::Inline { length }, false) => format!("return {length}"),
                (ByteRange::Sequence { .. }, true) => format!(
                    "position = {offset}\n        return position + self._storage.u32[position // 4]"
                ),
                (ByteRange::Sequence { stride: 0 }, false) => "return 0".into(),
                (ByteRange::Sequence { stride }, false) => {
                    format!("return self._storage.u32[(({offset}) + 4) // 4] * {stride}")
                }
            };
            writeln!(output, "        {body}")?;
        }
    }
    Ok(())
}

fn position(region: &NumericRegion, offset: &str, tag: Option<(u32, &str)>) -> String {
    let mut text = String::new();
    if let Some((tag, tag_offset)) = tag {
        let _ = writeln!(
            text,
            "if self._storage.u32[({tag_offset}) // 4] != {tag}:\n    return None"
        );
    }
    let _ = writeln!(text, "position = {offset}");
    for step in &region.steps {
        match step {
            RegionStep::Optional(offset) => {
                let _ = writeln!(
                    text,
                    "if self._storage.u8[position] == 0:\n    return None\nposition += {offset}"
                );
            }
            RegionStep::Boxed => text.push_str("position += self._storage.u32[position // 4]\n"),
        }
    }
    if region.sequence {
        text.push_str("position += self._storage.u32[position // 4]\n");
    }
    text
}

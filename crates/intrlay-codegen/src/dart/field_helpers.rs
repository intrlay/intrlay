//! Direct field helpers avoid constructing typed-data or structural child views.

use std::fmt::Write;

use super::{documentation, load, scalar_type, store};
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
    let index = if indexed { "core.int index, " } else { "" };
    let lane = format!("position + component * {}", region.scalar.layout().size());
    let docs = FieldHelpers::component_docs(field, indexed);
    if let Some(name) = &api.component_getter {
        let optional = if region.optional() || tag.is_some() {
            "?"
        } else {
            ""
        };
        writeln!(output)?;
        documentation::write(output, &docs, "  ")?;
        inline_hints(output)?;
        writeln!(
            output,
            "  {}{optional} {name}({index}core.int component) {{\n    {}return {};\n  }}",
            scalar_type(region.scalar),
            position(&region, offset, tag, false).replace('\n', "\n    "),
            load::scalar(region.scalar, &lane)
        )?;
    }
    if let Some(name) = &api.component_setter {
        if api.component_getter.is_none() {
            writeln!(output)?;
        }
        documentation::write(output, &docs, "  ")?;
        inline_hints(output)?;
        let write = store(region.scalar, &lane, "value").replace("storage.", "this.$[storage].");
        writeln!(
            output,
            "  void {name}({index}core.int component, {} value) {{\n    {}{write};\n  }}",
            scalar_type(region.scalar),
            position(&region, offset, tag, true).replace('\n', "\n    ")
        )?;
    }
    Ok(())
}

fn inline_hints(output: &mut String) -> Result<(), GenerateError> {
    // dart2js otherwise leaves repeated component calls out of line while
    // scalar getters with literal offsets are routinely folded into the caller.
    writeln!(output, "  @core.pragma('dart2js:tryInline')")?;
    writeln!(output, "  @core.pragma('vm:prefer-inline')")?;
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
            documentation::write(
                output,
                &FieldHelpers::range_docs(plan, field, indexed, is_offset),
                "  ",
            )?;
            let optional = if guarded { "?" } else { "" };
            let signature = if indexed {
                let index = if uses_index { "index" } else { "_index" };
                format!("core.int{optional} {name}(core.int {index})")
            } else {
                format!("core.int{optional} get {name}")
            };
            let value = match (range, is_offset) {
                (ByteRange::Inline { .. }, true) => offset.to_owned(),
                (ByteRange::Inline { length }, false) => length.to_string(),
                (ByteRange::Sequence { .. }, true) => {
                    "position + this.$[storage].u32(position)".into()
                }
                (ByteRange::Sequence { stride: 0 }, false) => "0".into(),
                (ByteRange::Sequence { stride }, false) => {
                    format!("this.$[storage].u32(({offset}) + 4) * {stride}")
                }
            };
            let follows_pointer =
                is_offset && matches!(plan.byte_range(field), ByteRange::Sequence { .. });
            if guarded || follows_pointer {
                writeln!(output, "  {signature} {{")?;
                if guarded && let Some((tag, tag_offset)) = tag {
                    writeln!(
                        output,
                        "    if (this.$[storage].u32({tag_offset}) != {tag}) return null;"
                    )?;
                }
                if follows_pointer {
                    writeln!(output, "    final position = {offset};")?;
                }
                writeln!(output, "    return {value};\n  }}")?;
            } else {
                writeln!(output, "  {signature} => {value};")?;
            }
        }
    }
    Ok(())
}

fn position(
    region: &NumericRegion,
    offset: &str,
    tag: Option<(u32, &str)>,
    setter: bool,
) -> String {
    let mut text = String::new();
    let value = if setter { "" } else { " null" };
    if let Some((tag, tag_offset)) = tag {
        let _ = writeln!(
            text,
            "if (this.$[storage].u32({tag_offset}) != {tag}) return{value};"
        );
    }
    let _ = writeln!(text, "var position = {offset};");
    for step in &region.steps {
        match step {
            RegionStep::Optional(offset) => {
                let _ = writeln!(
                    text,
                    "if (this.$[storage].bytes[position] == 0) return{value};\nposition += {offset};"
                );
            }
            RegionStep::Boxed => text.push_str("position += this.$[storage].u32(position);\n"),
        }
    }
    if region.sequence {
        text.push_str("position += this.$[storage].u32(position);\n");
    }
    text
}

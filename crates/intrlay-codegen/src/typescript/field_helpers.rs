//! Direct field helpers avoid constructing numeric or structural child views.

use std::fmt::Write;

use super::{method, scalar_type, storage_index, storage_name};
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
    let parameters = if indexed { "index: number, " } else { "" };
    let lane = format!(
        "this.$[storage].{}[({}) + component]",
        storage_name(region.scalar),
        storage_index(region.scalar, "position")
    );
    let docs = FieldHelpers::component_docs(field, indexed);
    if let Some(name) = &api.component_getter {
        let mut ty = scalar_type(region.scalar).to_owned();
        if region.optional() {
            ty.push_str(" | null");
        }
        if tag.is_some() {
            ty.push_str(" | undefined");
        }
        writeln!(output)?;
        method(
            output,
            &format!("public {name}({parameters}component: number): {ty}"),
            &format!("{}return {lane}!;", position(&region, offset, tag, false)),
            &docs,
        )?;
    }
    if let Some(name) = &api.component_setter {
        if api.component_getter.is_none() {
            writeln!(output)?;
        }
        method(
            output,
            &format!(
                "public {name}({parameters}component: number, value: {}): void",
                scalar_type(region.scalar)
            ),
            &format!("{}{lane} = value;", position(&region, offset, tag, true)),
            &docs,
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
            let result_type = if guarded {
                "number | undefined"
            } else {
                "number"
            };
            let signature = if indexed {
                let index = if uses_index { "index" } else { "_index" };
                format!("public {name}({index}: number): {result_type}")
            } else {
                format!("public get {name}(): {result_type}")
            };
            let mut body = match (range, is_offset) {
                (ByteRange::Inline { .. }, true) => format!("return {offset};"),
                (ByteRange::Inline { length }, false) => format!("return {length};"),
                (ByteRange::Sequence { .. }, true) => format!(
                    "const position = {offset};\nreturn position + this.$[storage].u32[position >>> 2]!;"
                ),
                (ByteRange::Sequence { stride: 0 }, false) => "return 0;".into(),
                (ByteRange::Sequence { stride }, false) => {
                    format!("return this.$[storage].u32[(({offset}) + 4) >>> 2]! * {stride};")
                }
            };
            if guarded && let Some((tag, tag_offset)) = tag {
                body = format!(
                    "if (this.$[storage].u32[({tag_offset}) >>> 2] !== {tag}) {{ return undefined; }}\n{body}"
                );
            }
            writeln!(output)?;
            method(
                output,
                &signature,
                &body,
                &FieldHelpers::range_docs(plan, field, indexed, is_offset),
            )?;
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
    if let Some((tag, tag_offset)) = tag {
        let value = if setter { "" } else { " undefined" };
        let _ = writeln!(
            text,
            "if (this.$[storage].u32[({tag_offset}) >>> 2] !== {tag}) {{ return{value}; }}"
        );
    }
    let _ = writeln!(text, "let position = {offset};");
    for step in &region.steps {
        match step {
            RegionStep::Optional(offset) => {
                let value = if setter { "" } else { " null" };
                let _ = writeln!(
                    text,
                    "if (this.$[storage].u8[position] === 0) {{ return{value}; }}\nposition += {offset};"
                );
            }
            RegionStep::Boxed => {
                text.push_str("position += this.$[storage].u32[position >>> 2]!;\n");
            }
        }
    }
    if region.sequence {
        text.push_str("position += this.$[storage].u32[position >>> 2]!;\n");
    }
    text
}

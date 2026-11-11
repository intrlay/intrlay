//! Collection field access, including guarded enum payloads.

use std::fmt::Write;

use super::{documentation, input_type, load, mutation, optional_type, output_type, read_method};
use crate::{
    GenerateError,
    api::IndexedAccessors,
    plan::{Id, Node, TypeGraph},
};

pub(super) fn fields(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
    position: &str,
) -> Result<(), GenerateError> {
    let child = &plan.nodes[element];
    let element_offset = format!("({position}) + index * {}", child.layout.size());
    if let Some(getter) = IndexedAccessors::tag(plan, node, child) {
        writeln!(output)?;
        documentation::write(
            output,
            &crate::documentation::indexed("Returns the element's active variant tag."),
            "  ",
        )?;
        writeln!(
            output,
            "  core.int {getter}(core.int index) => this.$[storage].u32({element_offset});"
        )?;
    }
    if plan.folded_field(element).is_none() {
        for (field, variant) in child.fields() {
            let offset = format!(
                "({position}) + index * {} + {}",
                plan.nodes[element].layout.size(),
                field.offset
            );
            let tag = variant.map(|variant| (variant.tag, element_offset.as_str()));
            let api = IndexedAccessors::new(plan, node, child, field, variant);
            super::field_helpers::emit(output, plan, &api.helpers, field, &offset, tag, true)?;
            if let Some(getter) = &api.getter {
                let index = if tag.is_some() {
                    "index"
                } else {
                    load::index_parameter(plan, field.ty, field.options.exposure)
                };
                let ty = output_type(plan, field.ty);
                let ty = if tag.is_some() {
                    optional_type(plan, field.ty, &ty)
                } else {
                    ty
                };
                writeln!(output)?;
                documentation::write(output, &TypeGraph::indexed_field_docs(field, false), "  ")?;
                read_method(
                    output,
                    plan,
                    &format!("{ty} {getter}(core.int {index})"),
                    &load::Read {
                        id: field.ty,
                        offset: &offset,
                        exposure: field.options.exposure,
                        tag,
                    },
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if api.getter.is_none() {
                    writeln!(output)?;
                }
                documentation::write(output, &TypeGraph::indexed_field_docs(field, false), "  ")?;
                writeln!(
                    output,
                    "  void {}(core.int index, {} value) {{\n    {}\n  }}",
                    setter.name,
                    input_type(plan, mutation.value),
                    format_args!(
                        "{}{}",
                        indexed_guard(tag, ""),
                        mutation::body(plan, mutation, &offset)
                    )
                )?;
            }
        }
    }
    Ok(())
}

fn indexed_guard(tag: Option<(u32, &str)>, value: &str) -> String {
    tag.map_or_else(String::new, |(tag, offset)| {
        format!("if (this.$[storage].u32({offset}) != {tag}) return{value};\n")
    })
}

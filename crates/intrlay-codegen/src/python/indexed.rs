//! Collection field access, including guarded enum payloads.

use std::fmt::Write;

use super::{documentation, input_type, load, mutation, optional_type, output_type};
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
        writeln!(
            output,
            "\n    def {getter}(self, index: _builtins.int) -> _builtins.int:"
        )?;
        documentation::write(
            output,
            &crate::documentation::indexed("Returns the element's active variant tag."),
            "        ",
        )?;
        writeln!(
            output,
            "        return self._storage.u32[({element_offset}) // 4]"
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
                let unused_index =
                    if tag.is_some() || load::uses_index(plan, field.ty, field.options.exposure) {
                        ""
                    } else {
                        "  # noqa: ARG002 - Keep the public index keyword for unit values."
                    };
                let ty = if tag.is_some() {
                    optional_type(plan, field.ty, false)
                } else {
                    output_type(plan, field.ty)
                };
                writeln!(
                    output,
                    "\n    def {getter}(self, index: _builtins.int) -> {ty}:{unused_index}",
                )?;
                documentation::write(
                    output,
                    &TypeGraph::indexed_field_docs(field, false),
                    "        ",
                )?;
                writeln!(
                    output,
                    "        {}",
                    load::body(
                        plan,
                        &load::Read {
                            id: field.ty,
                            offset: &offset,
                            exposure: field.options.exposure,
                            tag
                        },
                        None
                    )
                    .replace('\n', "\n        ")
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if api.getter.is_none() {
                    writeln!(output)?;
                }
                writeln!(
                    output,
                    "    def {}(self, index: _builtins.int, value: {}) -> None:",
                    setter.name,
                    input_type(plan, mutation.value),
                )?;
                documentation::write(
                    output,
                    &TypeGraph::indexed_field_docs(field, false),
                    "        ",
                )?;
                writeln!(
                    output,
                    "        {}",
                    format_args!(
                        "{}{}",
                        indexed_guard(tag, "").replace('\n', "\n        "),
                        mutation::body(plan, mutation, &offset)?
                    )
                )?;
            }
        }
    }
    Ok(())
}

fn indexed_guard(tag: Option<(u32, &str)>, value: &str) -> String {
    tag.map_or_else(String::new, |(tag, offset)| {
        format!("if self._storage.u32[({offset}) // 4] != {tag}:\n    return{value}\n")
    })
}

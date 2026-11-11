//! Collection field access, including guarded enum payloads.

use std::fmt::Write;

use super::{input_type, load, method, mutation, output_type};
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
        method(
            output,
            &format!("public {getter}(index: number): {}", child.tag_type_name()),
            &format!("return this.$[storage].u32[({element_offset}) >>> 2]!;"),
            &crate::documentation::indexed("Returns the element's active variant tag."),
        )?;
    }
    if plan.folded_node(child).is_none() {
        for (field, variant) in child.fields() {
            let p = format!(
                "({position}) + index * {} + {}",
                child.layout.size(),
                field.offset
            );
            let tag = variant.map(|variant| (variant.tag, element_offset.as_str()));
            let api = IndexedAccessors::new(plan, node, child, field, variant);
            super::field_helpers::emit(output, plan, &api.helpers, field, &p, tag, true)?;
            if let Some(getter) = &api.getter {
                let index = if tag.is_some() {
                    "index"
                } else {
                    load::index_parameter(plan, field.ty, field.options.exposure)
                };
                let optional = if tag.is_some() { " | undefined" } else { "" };
                writeln!(output)?;
                method(
                    output,
                    &format!(
                        "public {}({index}: number): {}{optional}",
                        getter,
                        output_type(plan, field.ty, field.options.exposure)
                    ),
                    &load::body(
                        plan,
                        &load::Read {
                            id: field.ty,
                            offset: &p,
                            exposure: field.options.exposure,
                            tag,
                        },
                        None,
                    ),
                    &TypeGraph::indexed_field_docs(field, false),
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if api.getter.is_none() {
                    writeln!(output)?;
                }
                method(
                    output,
                    &format!(
                        "public {}(index: number, value: {}): void",
                        setter.name,
                        input_type(plan, mutation.value)
                    ),
                    &format!(
                        "{}{}",
                        indexed_guard(tag, ""),
                        mutation::body(plan, mutation, &p)
                    ),
                    &TypeGraph::indexed_field_docs(field, false),
                )?;
            }
        }
    }
    Ok(())
}

fn indexed_guard(tag: Option<(u32, &str)>, value: &str) -> String {
    tag.map_or_else(String::new, |(tag, offset)| {
        format!("if (this.$[storage].u32[({offset}) >>> 2] !== {tag}) {{\n    return{value};\n}}\n")
    })
}

use crate::plan::{Kind, Mutation, MutationStep, TypeGraph};
use std::fmt::Write;

pub(super) fn body(plan: &TypeGraph, mutation: &Mutation, offset: &str) -> String {
    let mut code = String::new();
    let position = if mutation.steps.is_empty() {
        offset
    } else {
        "position"
    };
    if !mutation.steps.is_empty() {
        let _ = writeln!(code, "var position = {offset};");
    }
    for step in &mutation.steps {
        match step {
            MutationStep::Optional(offset) => {
                let _ = writeln!(
                    code,
                    "if (this.$[storage].bytes[position] == 0) return;\nposition += {offset};"
                );
            }
            MutationStep::Boxed => code.push_str("position += this.$[storage].u32(position);\n"),
            MutationStep::Inline(offset) => {
                let _ = writeln!(code, "position += {offset};");
            }
        }
    }
    if let Kind::Scalar(scalar) = plan.nodes[mutation.value].kind
        && plan.nodes[mutation.value].numeric_input.is_none()
    {
        let _ = write!(
            code,
            "{};",
            super::store(scalar, position, "value").replace("storage.", "this.$[storage].")
        );
    } else {
        let _ = write!(
            code,
            "$[write:{}](this.$[storage].writer, {position}, value, null);",
            mutation.value
        );
    }
    code.replace('\n', "\n    ")
}

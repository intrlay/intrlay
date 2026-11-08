use std::fmt::Write;

use crate::plan::{Kind, Mutation, MutationStep, TypeGraph};

pub(super) fn body(plan: &TypeGraph, mutation: &Mutation, offset: &str) -> String {
    let mut code = String::from("const storage = this.$[storage];\n");
    let position = if mutation.steps.is_empty() {
        offset
    } else {
        "position"
    };
    if !mutation.steps.is_empty() {
        let _ = writeln!(code, "let position = {offset};");
    }
    for step in &mutation.steps {
        match step {
            MutationStep::Optional(offset) => {
                let _ = writeln!(
                    code,
                    "if (storage.u8[position] === 0) {{\n    return;\n}}\nposition += {offset};"
                );
            }
            MutationStep::Boxed => code.push_str("position += storage.u32[position >>> 2]!;\n"),
            MutationStep::Inline(offset) => {
                let _ = writeln!(code, "position += {offset};");
            }
        }
    }
    if let Kind::Scalar(scalar) = plan.nodes[mutation.value].kind {
        let _ = write!(code, "{};", super::store(scalar, position, "value"));
    } else {
        let _ = write!(
            code,
            "$[write:{}](storage, {position}, value, undefined);",
            mutation.value
        );
    }
    code
}

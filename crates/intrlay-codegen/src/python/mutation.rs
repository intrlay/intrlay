use crate::plan::{Kind, Mutation, MutationStep, TypeGraph};
use std::fmt::Write;

pub(super) fn body(
    plan: &TypeGraph,
    mutation: &Mutation,
    offset: &str,
) -> Result<String, crate::GenerateError> {
    let mut code = String::new();
    let position = if mutation.steps.is_empty() {
        offset
    } else {
        "position"
    };
    if !mutation.steps.is_empty() {
        let _ = writeln!(code, "position = {offset}");
    }
    for step in &mutation.steps {
        match step {
            MutationStep::Optional(offset) => {
                let _ = writeln!(
                    code,
                    "if self._storage.u8[position] == 0:\n    return\nposition += {offset}"
                );
            }
            MutationStep::Boxed => code.push_str("position += self._storage.u32[position // 4]\n"),
            MutationStep::Inline(offset) => {
                let _ = writeln!(code, "position += {offset}");
            }
        }
    }
    if let Kind::Scalar(scalar) = plan.nodes[mutation.value].kind {
        let _ = write!(
            code,
            "{} = value",
            super::scalar_access(scalar, "self._storage", position)
        );
    } else {
        let writer = super::names::reference(&crate::symbols::codec_function(
            plan,
            "write",
            mutation.value,
        )?);
        let _ = write!(code, "{writer}(self._storage, {position}, value, None)");
    }
    Ok(code.replace('\n', "\n        "))
}

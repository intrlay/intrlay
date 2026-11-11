//! `TypedArray` construction writes the explicit numeric projection directly.

use std::fmt::Write;

use crate::{
    GenerateError,
    plan::{Kind, Node, NumericInput, TypeGraph},
};

pub(super) fn input_type(plan: &TypeGraph, node: &Node) -> Option<String> {
    let numeric = node.numeric_input?;
    let zeroed = if plan.numeric_input_zeroable(node) {
        if numeric.sequence {
            " | intrlay.ZeroedSequence"
        } else {
            " | intrlay.Zeroed"
        }
    } else {
        ""
    };
    Some(format!("{}{zeroed}", super::array_type(numeric.scalar)))
}

fn count(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    numeric: NumericInput,
) -> Result<(), GenerateError> {
    let lanes = numeric.lanes;
    if plan.numeric_input_zeroable(node) {
        writeln!(
            output,
            "    const zeroed = typeof value === \"number\";\n    const length = zeroed ? 0 : value.length;"
        )?;
    } else {
        writeln!(output, "    const length = value.length;")?;
    }
    let invalid = if lanes == 0 {
        "length !== 0".into()
    } else {
        format!("length % {lanes} !== 0")
    };
    writeln!(
        output,
        "    if ({invalid}) {{\n        throw new globalThis.RangeError(\"numeric projection length\");\n    }}"
    )?;
    let count = if lanes == 0 {
        "0".into()
    } else {
        format!("length / {lanes}")
    };
    let count = if plan.numeric_input_zeroable(node) {
        format!("zeroed ? intrlay.count(value) : {count}")
    } else {
        count
    };
    writeln!(output, "    const count = {count};")?;
    Ok(())
}

pub(super) fn measure(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    numeric: NumericInput,
) -> Result<(), GenerateError> {
    let Kind::Sequence(element) = node.kind else {
        unreachable!("only sequences allocate")
    };
    count(output, plan, node, numeric)?;
    let layout = plan.nodes[element].layout;
    writeln!(
        output,
        "    cursor.reserve(count * {}, {});",
        layout.size(),
        layout.alignment()
    )?;
    Ok(())
}

pub(super) fn write(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    numeric: NumericInput,
) -> Result<(), GenerateError> {
    let target = if let Kind::Sequence(element) = node.kind {
        count(output, plan, node, numeric)?;
        let layout = plan.nodes[element].layout;
        writeln!(
            output,
            "    const payloadOffset = cursor.reserve(count * {}, {});\n    storage.u32[offset >>> 2] = payloadOffset - offset;\n    storage.u32[(offset >>> 2) + 1] = count;",
            layout.size(),
            layout.alignment()
        )?;
        if numeric.lanes != 0 && plan.numeric_input_zeroable(node) {
            writeln!(
                output,
                "    if (zeroed) {{\n        storage.u8.fill(0, payloadOffset, payloadOffset + count * {});\n        return;\n    }}",
                layout.size()
            )?;
        }
        "payloadOffset"
    } else {
        if plan.numeric_input_zeroable(node) {
            writeln!(output, "    if (typeof value === \"symbol\") {{")?;
            if numeric.lanes != 0 {
                writeln!(
                    output,
                    "        storage.u8.fill(0, offset, offset + {});",
                    node.layout.size()
                )?;
            }
            writeln!(output, "        return;\n    }}")?;
        }
        writeln!(
            output,
            "    if (value.length !== {}) {{\n        throw new globalThis.RangeError(\"numeric projection length\");\n    }}",
            numeric.lanes
        )?;
        "offset"
    };
    // Empty projections validate their extent and retain sequence counts, but
    // need no scalar storage whose alignment may exceed the wire type's.
    if numeric.lanes == 0 {
        return Ok(());
    }
    let array = super::storage_name(numeric.scalar);
    let shift = numeric.scalar.layout().size().trailing_zeros();
    writeln!(
        output,
        "    storage.{array}.set(value, {target} >>> {shift});"
    )?;
    Ok(())
}

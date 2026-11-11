//! Explicit numeric inputs accept an already packed, native-format memoryview.

use std::fmt::Write;

use crate::{
    GenerateError,
    plan::{Kind, Node, NumericInput, TypeGraph},
};

pub(super) fn input_type(plan: &TypeGraph, node: &Node) -> Option<String> {
    let numeric = node.numeric_input?;
    let zeroed = if plan.numeric_input_zeroable(node) {
        if numeric.sequence {
            " | _intrlay.ZeroedSequence"
        } else {
            " | _intrlay.Zeroed"
        }
    } else {
        ""
    };
    Some(format!("{}{zeroed}", super::numeric_type(numeric.scalar)))
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
            "    length = 0 if _builtins.isinstance(value, _builtins.int) else _builtins.len(value)"
        )?;
    } else {
        writeln!(output, "    length = _builtins.len(value)")?;
    }
    let invalid = if lanes == 0 {
        "length != 0".into()
    } else {
        format!("length % {lanes} != 0")
    };
    writeln!(
        output,
        "    if {invalid}:\n        raise _builtins.ValueError('numeric projection length')"
    )?;
    let count = if lanes == 0 {
        "0".into()
    } else {
        format!("length // {lanes}")
    };
    let count = if plan.numeric_input_zeroable(node) {
        format!("value if _builtins.isinstance(value, _builtins.int) else {count}")
    } else {
        count
    };
    writeln!(output, "    count = {count}")?;
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
        "    cursor.reserve(count * {}, {})",
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
    let (target, length) = if let Kind::Sequence(element) = node.kind {
        count(output, plan, node, numeric)?;
        let layout = plan.nodes[element].layout;
        writeln!(
            output,
            "    payload_offset = cursor.reserve(count * {}, {})\n    storage.u32[offset // 4] = payload_offset - offset\n    storage.u32[offset // 4 + 1] = count",
            layout.size(),
            layout.alignment()
        )?;
        if numeric.lanes != 0 && plan.numeric_input_zeroable(node) {
            writeln!(
                output,
                "    if _builtins.isinstance(value, _builtins.int):\n        storage.u8[payload_offset:payload_offset + count * {}] = _builtins.bytes(count * {})\n        return",
                layout.size(),
                layout.size()
            )?;
        }
        ("payload_offset", "length".into())
    } else {
        if plan.numeric_input_zeroable(node) {
            writeln!(
                output,
                "    if _builtins.isinstance(value, _intrlay.Zeroed):"
            )?;
            if numeric.lanes != 0 {
                writeln!(
                    output,
                    "        storage.u8[offset:offset + {}] = _builtins.bytes({})",
                    node.layout.size(),
                    node.layout.size()
                )?;
            }
            writeln!(output, "        return")?;
        }
        writeln!(
            output,
            "    if _builtins.len(value) != {}:\n        raise _builtins.ValueError('numeric projection length')",
            numeric.lanes
        )?;
        ("offset", numeric.lanes.to_string())
    };
    if numeric.lanes == 0 {
        return Ok(());
    }
    let array = format!("{:?}", numeric.scalar).to_lowercase();
    let size = numeric.scalar.layout().size();
    writeln!(
        output,
        "    start = {target} // {size}\n    storage.{array}[start:start + {length}] = value"
    )?;
    Ok(())
}

//! Flat scalar inputs retain Dart's allocation-free zero-initialization wrappers.

use std::fmt::Write;

use crate::{
    GenerateError,
    plan::{Kind, Node, NumericInput, TypeGraph},
};

pub(super) fn input_type(plan: &TypeGraph, node: &Node) -> Option<String> {
    let numeric = node.numeric_input?;
    let ty = if plan.numeric_input_zeroable(node) {
        let wrapper = if numeric.sequence {
            "SequenceInput"
        } else {
            "FixedArrayInput"
        };
        format!("intrlay.{wrapper}<{}>", super::scalar_type(numeric.scalar))
    } else {
        super::numeric_type(numeric.scalar)
    };
    Some(ty)
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
            "  final items = value.values;\n  final length = items?.length ?? 0;"
        )?;
    } else {
        writeln!(
            output,
            "  final items = value;\n  final length = items.length;"
        )?;
    }
    let invalid = if lanes == 0 {
        "length != 0".into()
    } else {
        format!("length % {lanes} != 0")
    };
    writeln!(
        output,
        "  if ({invalid}) {{\n    throw core.ArgumentError('numeric projection length');\n  }}"
    )?;
    let count = if lanes == 0 {
        "0".into()
    } else {
        format!("length ~/ {lanes}")
    };
    let count = if plan.numeric_input_zeroable(node) {
        format!("items == null ? value.length : {count}")
    } else {
        count
    };
    super::sequence_count(output, &count)
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
        "  cursor.reserve(count * {}, {});",
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
            "  final payloadOffset = cursor.reserve(count * {}, {});\n  storage.writeUint32(offset, payloadOffset - offset);\n  storage.writeUint32(offset + 4, count);",
            layout.size(),
            layout.alignment()
        )?;
        if numeric.lanes != 0 && plan.numeric_input_zeroable(node) {
            writeln!(
                output,
                "  if (items == null) {{\n    storage.bytes.fillRange(payloadOffset, payloadOffset + count * {}, 0);\n    return;\n  }}",
                layout.size()
            )?;
        }
        "payloadOffset"
    } else {
        if plan.numeric_input_zeroable(node) {
            writeln!(
                output,
                "  final items = value.values;\n  if (items == null) {{"
            )?;
            if numeric.lanes != 0 {
                writeln!(
                    output,
                    "    storage.bytes.fillRange(offset, offset + {}, 0);",
                    node.layout.size()
                )?;
            }
            writeln!(output, "    return;\n  }}")?;
        } else {
            writeln!(output, "  final items = value;")?;
        }
        writeln!(
            output,
            "  if (items.length != {}) {{\n    throw core.ArgumentError('numeric projection length');\n  }}",
            numeric.lanes
        )?;
        "offset"
    };
    if numeric.lanes == 0 {
        return Ok(());
    }
    // Reuse the runtime's AOT/dart2js bulk-copy choices, including 64-bit storage.
    writeln!(
        output,
        "  intrlay.write{}Array(storage, {target}, items);",
        super::method(numeric.scalar)
    )?;
    Ok(())
}

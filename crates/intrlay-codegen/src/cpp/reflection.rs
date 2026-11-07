//! Compile-time field metadata shared by construction and borrowed access.

use std::fmt::Write;

use crate::{
    GenerateError, SymbolKind, Visibility,
    plan::{Field, Id, Kind, TypeGraph},
};

pub fn name(plan: &TypeGraph, id: Id) -> String {
    plan.names.symbol(
        &format!("{}_descriptor", plan.nodes[id].name),
        SymbolKind::Type,
        Some(Visibility::Internal),
        &[],
    )
}

/// Primitive fields use shared descriptors; they do not need generated types.
pub fn reference(plan: &TypeGraph, id: Id, qualified: bool) -> String {
    match plan.nodes[id].kind {
        Kind::Scalar(intrlay_schema::Scalar::Bool) => {
            "::intrlay::Scalar<bool, ::std::uint8_t>".into()
        }
        Kind::Scalar(scalar) => format!("::intrlay::Scalar<{}>", super::scalar_type(scalar)),
        Kind::Unit => "::intrlay::Unit".into(),
        Kind::String => "::intrlay::String".into(),
        _ if qualified => super::qualified(plan, &format!("detail::{}", name(plan, id))),
        _ => name(plan, id),
    }
}

fn descriptors(plan: &TypeGraph) -> impl Iterator<Item = Id> + '_ {
    plan.nodes.iter().enumerate().filter_map(|(id, node)| {
        (!matches!(node.kind, Kind::Scalar(_) | Kind::Unit | Kind::String)).then_some(id)
    })
}

pub fn declarations(output: &mut String, plan: &TypeGraph) -> Result<(), GenerateError> {
    writeln!(output, "\nnamespace detail {{")?;
    for id in descriptors(plan) {
        writeln!(output, "struct {};", name(plan, id))?;
    }
    writeln!(output, "}} // namespace detail")?;
    Ok(())
}

pub fn generate(output: &mut String, plan: &TypeGraph) -> Result<(), GenerateError> {
    writeln!(
        output,
        "\n// Resolved wire layouts and native member mappings.\nnamespace detail {{"
    )?;
    for id in descriptors(plan) {
        let node = &plan.nodes[id];
        writeln!(output)?;
        write!(output, "struct {} : ", name(plan, id))?;
        let view = super::qualified(plan, &format!("Basic{}", plan.view_name(id)));
        match &node.kind {
            Kind::Record(fields) => {
                write!(output, "::intrlay::Record<{view}, {}", node.layout.size())?;
                fields_metadata(output, plan, fields, &node.name, false, "    ")?;
                if !fields.is_empty() {
                    writeln!(output)?;
                }
                write!(output, ">")?;
            }
            Kind::Enum(variants) => {
                write!(output, "::intrlay::Enum<{view}")?;
                for variant in variants {
                    write!(output, ",\n    ::intrlay::Variant<{}", variant.tag)?;
                    fields_metadata(
                        output,
                        plan,
                        &variant.fields,
                        &format!("{}{}", node.name, variant.name),
                        variant.synthetic_payload,
                        "        ",
                    )?;
                    if !variant.fields.is_empty() {
                        write!(output, "\n    ")?;
                    }
                    write!(output, ">")?;
                }
                write!(output, "\n>")?;
            }
            Kind::Sequence(element) => write!(
                output,
                "::intrlay::Sequence<\n    {view},\n    {}, {}, {}\n>",
                reference(plan, *element, false),
                plan.nodes[*element].layout.size(),
                plan.nodes[*element].layout.alignment()
            )?,
            Kind::Array { element, length } => write!(
                output,
                "::intrlay::Array<\n    {view},\n    {}, {}, {length}\n>",
                reference(plan, *element, false),
                plan.nodes[*element].layout.size()
            )?,
            Kind::Optional { value, offset } => write!(
                output,
                "::intrlay::Optional<{}, {offset}>",
                reference(plan, *value, false)
            )?,
            Kind::Boxed(value) => write!(
                output,
                "::intrlay::Box<{}, {}, {}>",
                reference(plan, *value, false),
                plan.nodes[*value].layout.size(),
                plan.nodes[*value].layout.alignment()
            )?,
            Kind::Scalar(_) | Kind::String | Kind::Unit => unreachable!(),
        }
        if super::is_collection(node) {
            write!(
                output,
                ", ::intrlay::BufferLayout<{}, {}, {}>",
                node.layout.size(),
                node.buffer_alignment,
                plan.writes_all_bytes(id)
            )?;
        }
        writeln!(output, " {{}};")?;
    }
    writeln!(output, "}} // namespace detail\n")?;
    Ok(())
}

fn fields_metadata(
    output: &mut String,
    plan: &TypeGraph,
    fields: &[Field],
    native: &str,
    synthetic_payload: bool,
    indent: &str,
) -> Result<(), GenerateError> {
    for field in fields {
        write!(
            output,
            ",\n{indent}::intrlay::Field<\"{}\", {}, {}, ",
            field.name,
            field.offset,
            reference(plan, field.ty, false)
        )?;
        let member = format!("&{}::{}", super::qualified(plan, native), field.name);
        if synthetic_payload {
            write!(output, "::intrlay::AlternativeValue{{}}")?;
        } else if field.name != field.source_name
            && !super::names::KEYWORDS
                .split_whitespace()
                .any(|keyword| keyword == field.source_name)
        {
            write!(
                output,
                "::intrlay::MappedMember<{member}, INTRLAY_MEMBER({})>{{}}",
                field.source_name
            )?;
        } else {
            // A source-language keyword (e.g. Rust's r#class) cannot name a C++
            // member. In that case only the explicitly renamed API is usable.
            write!(output, "INTRLAY_MEMBER({})", field.name)?;
        }
        write!(output, ">")?;
    }
    Ok(())
}

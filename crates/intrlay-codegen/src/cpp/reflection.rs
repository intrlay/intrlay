//! Compile-time field metadata shared by construction and borrowed access.

use std::{collections::BTreeSet, fmt::Write};

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
pub fn reference(plan: &TypeGraph, id: Id, context: Option<&super::Context>) -> String {
    match plan.nodes[id].kind {
        Kind::Scalar(intrlay_schema::Scalar::Bool) => {
            "::intrlay::Scalar<bool, ::std::uint8_t>".into()
        }
        Kind::Scalar(scalar) => format!("::intrlay::Scalar<{}>", super::scalar_type(scalar)),
        Kind::Unit => "::intrlay::Unit".into(),
        Kind::String => "::intrlay::String".into(),
        _ => context.map_or_else(
            || name(plan, id),
            |context| context.qualified(&format!("detail::{}", name(plan, id))),
        ),
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

pub fn generate(
    output: &mut String,
    plan: &TypeGraph,
    context: &super::Context,
) -> Result<(), GenerateError> {
    writeln!(
        output,
        "\n// Resolved wire layouts and native member mappings.\nnamespace detail {{"
    )?;
    member_accessors(output, plan)?;
    for id in descriptors(plan) {
        let node = &plan.nodes[id];
        let members = format!("members::{}", name(plan, id));
        writeln!(output)?;
        write!(output, "struct {} : ", name(plan, id))?;
        let view = context.qualified(&format!("Basic{}", plan.view_name(id)));
        match &node.kind {
            Kind::Record(fields) => {
                write!(output, "::intrlay::Record<{view}, {}", node.layout.size())?;
                fields_metadata(output, plan, fields, &members, false, "    ")?;
                if !fields.is_empty() {
                    writeln!(output)?;
                }
                write!(output, ">")?;
            }
            Kind::Enum(variants) => {
                if node.is_value_enum() {
                    write!(
                        output,
                        "::intrlay::ValueEnum<{}, {}",
                        context.qualified(&node.input_name()),
                        node.layout.size()
                    )?;
                    for variant in variants {
                        write!(output, ", {}", variant.tag)?;
                    }
                    write!(output, ">")?;
                } else {
                    write!(output, "::intrlay::Enum<{view}, {}", node.layout.size())?;
                    for variant in variants {
                        write!(output, ",\n    ::intrlay::Variant<{}", variant.tag)?;
                        fields_metadata(
                            output,
                            plan,
                            &variant.fields,
                            &members,
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
            }
            Kind::Sequence(element) => write!(
                output,
                "::intrlay::Sequence<\n    {view},\n    {}, {}, {}\n>",
                reference(plan, *element, None),
                plan.nodes[*element].layout.size(),
                plan.nodes[*element].layout.alignment()
            )?,
            Kind::Array { element, length } => write!(
                output,
                "::intrlay::Array<\n    {view},\n    {}, {}, {length}\n>",
                reference(plan, *element, None),
                plan.nodes[*element].layout.size()
            )?,
            Kind::Optional { value, offset } => write!(
                output,
                "::intrlay::Optional<{}, {offset}, {}>",
                reference(plan, *value, None),
                node.layout.size()
            )?,
            Kind::Boxed(value) => write!(
                output,
                "::intrlay::Box<{}, {}, {}>",
                reference(plan, *value, None),
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
        writeln!(
            output,
            " {{\n    static constexpr bool zeroable = {};\n}};",
            plan.zeroable(id)
        )?;
    }
    writeln!(output, "}} // namespace detail\n")?;
    Ok(())
}

/// Inline variables give accessor closure types one identity across translation
/// units. Lambdas directly in a descriptor's base list lack that named context.
fn member_accessors(output: &mut String, plan: &TypeGraph) -> Result<(), GenerateError> {
    for id in descriptors(plan) {
        let mut members = BTreeSet::new();
        match &plan.nodes[id].kind {
            Kind::Record(fields) => {
                members.extend(fields.iter().map(source_member));
                members.extend(fields.iter().map(|field| field.name.as_str()));
            }
            Kind::Enum(variants) => {
                for variant in variants {
                    if !variant.synthetic_payload {
                        members.extend(variant.fields.iter().map(source_member));
                        members.extend(variant.fields.iter().map(|field| field.name.as_str()));
                    }
                }
            }
            _ => {}
        }
        if members.is_empty() {
            continue;
        }
        // Independent headers may use the same C++ namespace and field names.
        // Scope accessors by descriptor so their definitions do not collide.
        let scope = format!("members::{}", name(plan, id));
        writeln!(output, "\nnamespace {scope} {{")?;
        for member in members {
            writeln!(
                output,
                "inline constexpr auto {member} = INTRLAY_MEMBER({member});"
            )?;
        }
        writeln!(output, "}} // namespace {scope}")?;
    }
    Ok(())
}

fn source_member(field: &Field) -> &str {
    if super::names::is_reserved_identifier(&field.source_name) {
        // Another source language may use C++ keywords or runtime macro names.
        // Only the resolved C++ spelling can name a member in that case.
        &field.name
    } else {
        &field.source_name
    }
}

fn fields_metadata(
    output: &mut String,
    plan: &TypeGraph,
    fields: &[Field],
    members: &str,
    synthetic_payload: bool,
    indent: &str,
) -> Result<(), GenerateError> {
    for field in fields {
        write!(
            output,
            ",\n{indent}::intrlay::Field<\"{}\", {}, {}, ",
            field.name,
            field.offset,
            reference(plan, field.ty, None)
        )?;
        let source = source_member(field);
        if synthetic_payload {
            write!(output, "::intrlay::AlternativeValue{{}}")?;
        } else if field.name != source {
            write!(
                output,
                "::intrlay::MappedMember<{members}::{}, {members}::{source}>{{}}",
                field.name
            )?;
        } else {
            write!(output, "{members}::{source}")?;
        }
        write!(output, ">")?;
    }
    Ok(())
}

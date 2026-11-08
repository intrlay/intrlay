//! C# native inputs, span companions, inline arrays, and binding-class helpers.

use std::collections::BTreeSet;

use crate::{
    GenerateError,
    name_validation::{self as shared, NameKind, Scope},
    plan::{Field, GenerationPlan, Id, Kind, TypeGraph},
    symbols,
};

pub fn is_reserved(name: &str, kind: NameKind) -> bool {
    (kind == NameKind::Declaration && super::INTRINSIC_KEYWORDS.contains(&name))
        || shared::is_keyword(super::names::KEYWORDS, name)
}

pub fn validate(
    generation: &GenerationPlan,
    context: &super::Context,
) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let mut declarations = Scope::module(plan);
    declarations.declaration(&super::binding_name(plan), "generated binding class")?;
    inputs(
        plan,
        context,
        &generation.encoding.encoders,
        &mut declarations,
    )?;
    let lengths = plan
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            Kind::Array { length, .. } if length != 0 => Some(length),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for length in lengths {
        declarations.declaration(&super::array_name(plan, length), "native inline array")?;
    }

    let views = &generation.views.types;
    for &id in &generation.views.emitted {
        let name = super::view_name(views, id);
        let span = super::span_name(&name);
        declarations.declaration(&name, "view")?;
        declarations.declaration(&span, "span view")?;
        if views.nodes[id].options.generation.setters {
            declarations.declaration(&format!("{span}ReadOnly"), "read-only span view")?;
            view_members(views, id, &format!("{span}ReadOnly"), true, true)?;
        }
        view_members(views, id, &span, false, true)?;
        if views.nodes[id].options.generation.setters {
            declarations.declaration(&format!("{name}ReadOnly"), "read-only view")?;
            view_members(views, id, &format!("{name}ReadOnly"), true, false)?;
        }
        view_members(views, id, &name, false, false)?;
    }
    shared::input_view_aliases(generation, &mut declarations)?;
    for (alias, id) in &views.exports {
        if *alias != views.nodes[*id].name {
            let name = format!("{alias}View");
            let span = super::span_name(&name);
            declarations.declaration(&span, "export span view alias")?;
            let read_only = !views.nodes[*id].options.generation.setters;
            view_members(views, *id, &name, read_only, false)?;
            view_members(views, *id, &span, read_only, true)?;
        }
    }

    // C# codecs belong to the binding class, not the module's type namespace.
    let mut helpers = Scope::new(plan);
    helpers.reserve(&super::binding_name(plan), "enclosing binding class");
    shared::codec_helpers(generation, &mut helpers)?;
    for id in 0..plan.nodes.len() {
        if super::layout::native_layout(plan, id) {
            helpers.declaration(
                &symbols::codec_function(plan, "read_native", id)?,
                "native reader",
            )?;
        }
    }
    Ok(())
}

fn inputs(
    plan: &TypeGraph,
    context: &super::Context,
    encoders: &BTreeSet<Id>,
    declarations: &mut Scope<'_>,
) -> Result<(), GenerateError> {
    let sources = super::source_input_nodes(plan)
        .intersection(encoders)
        .copied()
        .collect::<BTreeSet<_>>();
    for (id, node) in plan.nodes.iter().enumerate() {
        let input = node.input_name();
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            native_type_name(&node.name)?;
            native_type_name(&input)?;
            declarations.input(node, &input)?;
            shared::input_fields(plan, node, &[])?;
        }
        if let Kind::Record(fields) = &node.kind
            && plan.folded_node(node).is_none()
        {
            native_fields(plan, &input, fields, sources.contains(&id))?;
            if let Some(span) = super::span_input_name(plan, id) {
                declarations.declaration(&span, "span input")?;
                native_fields(plan, &span, fields, sources.contains(&id))?;
            }
        }
        if let Kind::Enum(variants) = &node.kind {
            let mut members = Scope::new(plan);
            if node.is_value_enum() {
                members.reserve("value__", "native enum storage");
                members.reserve(&input, "enclosing enum type");
            } else {
                members.extend(inherited_members(plan));
                members.reserve(&input, "enclosing enum type");
                members.runtime_member("tag", "enum discriminant")?;
                if context.language_version >= 15 {
                    members.reserve("IUnionMembers", "C# union member provider");
                }
                for variant in variants {
                    members.member(
                        &plan.names.method(&format!("from_{}", variant.name), true),
                        "variant factory",
                    )?;
                }
            }
            for variant in variants {
                members.declaration(&variant.name, "enum variant")?;
                if !node.is_value_enum() {
                    let payload = node.variant_input_name(variant);
                    native_type_name(&payload)?;
                    native_fields(
                        plan,
                        &payload,
                        &variant.fields,
                        variant
                            .fields
                            .iter()
                            .any(|field| sources.contains(&field.ty)),
                    )?;
                    declarations.declaration(&payload, "variant payload")?;
                }
            }
        }
        if sources.contains(&id) && !matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            declarations
                .declaration(&super::source_input_name(plan, id), "generic source input")?;
        }
    }
    Ok(())
}

fn native_type_name(name: &str) -> Result<(), GenerateError> {
    if matches!(name, "var" | "record" | "scoped" | "required" | "file") {
        return Err(GenerateError(format!(
            "{name} is an unsupported C# native type name in generated declarations; assign an explicit target name"
        )));
    }
    Ok(())
}

fn native_fields(
    plan: &TypeGraph,
    enclosing: &str,
    fields: &[Field],
    source: bool,
) -> Result<(), GenerateError> {
    let mut members = inherited_members(plan);
    members.reserve(enclosing, "enclosing native type");
    if source {
        members.runtime_member("from_sources", "generic input factory")?;
    }
    for field in fields {
        members.member(&field.name, "native field")?;
    }
    Ok(())
}

fn view_members(
    plan: &TypeGraph,
    id: Id,
    name: &str,
    read_only: bool,
    scoped: bool,
) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    let mut members = inherited_members(plan);
    members.reserve(name, "enclosing view type");
    for key in ["byte_size", "alignment", "buffer", "offset", "bytes"] {
        members.runtime_member(key, "view infrastructure")?;
    }
    if node.options.generation.create {
        for key in ["create", "create_in_place", "compute_byte_length"] {
            members.runtime_member(key, "view construction")?;
        }
    }
    if !scoped {
        members.runtime_member("as_span", "native view access")?;
    }
    if super::layout::native_layout(plan, id) {
        members.runtime_member("to_value", "native view access")?;
    }
    if node.options.generation.setters && !plan.has_payload(id) && !read_only {
        members.runtime_member("replace", "in-place value replacement")?;
    }
    shared::collection_layout(plan, id, &mut members)?;
    if matches!(node.kind, Kind::Enum(_)) && node.options.generation.getters {
        members.runtime_member("tag", "enum tag accessor")?;
    }
    match node.kind {
        Kind::Record(_) | Kind::Enum(_) => {
            shared::field_accessors(plan, id, &mut members, read_only)
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            if super::layout::native_layout(plan, element) {
                members.runtime_member("try_get_native_span", "native view access")?;
            }
            members.runtime_member("count", "collection length")?;
            if node.options.generation.getters
                || (!read_only && node.options.generation.setters && plan.borrows_content(element))
            {
                // C# indexers declare a property named Item, even with `this[int]` syntax.
                members.member("Item", "collection indexer")?;
                if !scoped {
                    members.declaration("ElementAccess", "collection element reader")?;
                }
            }
            if !read_only && node.options.generation.setters && plan.mutation(element).is_some() {
                members.runtime_member("set", "collection element setter")?;
            }
            Ok(())
        }
        _ => shared::root_accessors(plan, id, &mut members, read_only),
    }
}

fn inherited_members(plan: &TypeGraph) -> Scope<'_> {
    let mut members = Scope::new(plan);
    members.reserve_all(
        &[
            "Equals",
            "Finalize",
            "GetHashCode",
            "GetType",
            "MemberwiseClone",
            "ReferenceEquals",
            "ToString",
        ],
        "inherited object member",
    );
    members
}

pub(super) fn namespace(namespace: &str) -> Result<(), GenerateError> {
    for segment in namespace.split('.') {
        if crate::plan::identifier(segment).is_err()
            || super::INTRINSIC_KEYWORDS.contains(&segment)
            || super::names::KEYWORDS
                .split_whitespace()
                .any(|keyword| keyword == segment)
        {
            return Err(GenerateError(format!("invalid C# namespace {namespace}")));
        }
    }
    Ok(())
}

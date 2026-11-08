//! C++ template parameters, reflection descriptors, and generated class scopes.

use crate::{
    GenerateError,
    name_validation::{self as shared, NameKind, Scope},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph},
    symbols,
};

pub fn is_reserved(name: &str, _kind: NameKind) -> bool {
    super::names::is_reserved_identifier(name)
}

pub fn validate(generation: &GenerationPlan) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let mut declarations = Scope::module(plan);
    declarations.reserve("INTRLAY_MEMBER", "generated member-access macro");
    for node in &plan.nodes {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            declarations.input(node, &node.input_name())?;
            shared::input_fields(plan, node, &[])?;
        }
        if let Kind::Record(fields) = &node.kind
            && plan.folded_node(node).is_none()
        {
            native_fields(plan, fields)?;
        }
        if let Kind::Enum(variants) = &node.kind {
            let mut members = Scope::new(plan);
            for variant in variants {
                members.declaration(&variant.name, "enum variant")?;
                if !node.is_value_enum() && !variant.synthetic_payload {
                    native_fields(plan, &variant.fields)?;
                    declarations
                        .declaration(&node.variant_input_name(variant), "variant payload")?;
                }
            }
        }
    }

    let views = &generation.views.types;
    for &id in &generation.views.emitted {
        let name = &views.nodes[id].view_name;
        declarations.declaration(name, "view")?;
        declarations.declaration(&format!("{name}ReadOnly"), "read-only view")?;
        declarations.declaration(&format!("Basic{name}"), "view template")?;
        view_members(views, id)?;
    }
    shared::input_view_aliases(generation, &mut declarations)?;

    // Wire descriptors live in a separate implementation namespace.
    let mut helpers = Scope::new(plan);
    helpers.reserve("members", "native member accessor namespace");
    for (id, node) in views.nodes.iter().enumerate() {
        if !matches!(node.kind, Kind::Scalar(_) | Kind::Unit | Kind::String) {
            helpers.declaration(&super::reflection::name(views, id), "wire descriptor")?;
        }
    }
    Ok(())
}

fn native_fields(plan: &TypeGraph, fields: &[Field]) -> Result<(), GenerateError> {
    let mut members = Scope::new(plan);
    members.reserve("INTRLAY_MEMBER", "generated member-access macro");
    members.reserve("intrlay_input", "input source marker");
    for field in fields {
        members.member(&field.name, "native field")?;
    }
    Ok(())
}

fn view_members(plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    let mut members = infrastructure(plan, node)?;
    let collection = matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. });
    if !collection && node.options.generation.setters && !plan.has_payload(id) {
        members.runtime_member("replace", "in-place value replacement")?;
    }
    if !collection {
        members.declaration(&format!("Basic{}", node.view_name), "enclosing view type")?;
        members.declaration("Byte", "view template parameter")?;
        members.declaration(
            &symbols::runtime_symbol(plan, "metadata")?,
            "view metadata alias",
        )?;
    }
    if matches!(node.kind, Kind::Enum(_)) && node.options.generation.getters {
        members.runtime_member("tag", "enum tag accessor")?;
    }
    match node.kind {
        Kind::Record(_) | Kind::Enum(_) => shared::field_accessors(plan, id, &mut members, false),
        Kind::Sequence(_) | Kind::Array { .. } => Ok(()),
        _ => shared::root_accessors(plan, id, &mut members, false),
    }
}

fn infrastructure<'a>(plan: &'a TypeGraph, node: &Node) -> Result<Scope<'a>, GenerateError> {
    // Fields, methods, aliases, and static members share a C++ class scope.
    // Check actual declarations instead of merging their resolved spellings.
    let mut members = Scope::new(plan);
    let collection = matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. });
    if collection && !super::collections::has_custom_names(plan)? {
        return Ok(members);
    }
    let mut keys = vec!["bytes", "byte_size", "alignment"];
    if collection {
        keys.extend(["storage", "metadata", "size_property", "get", "set"]);
        for name in ["Descriptor", "Byte", "Constructible", "extent"] {
            members.declaration(name, "collection infrastructure")?;
        }
        members.declaration(
            &super::collections::facade_name(plan),
            "enclosing collection type",
        )?;
    } else {
        keys.extend(["buffer", "offset"]);
    }
    if collection || node.options.generation.create {
        keys.extend(["create", "create_in_place", "compute_byte_length"]);
    }
    for key in keys {
        members.runtime_member(key, "view infrastructure")?;
    }
    if collection {
        // A constrained replace declaration still occupies this scope when
        // the descriptor or const Byte makes replacement unavailable.
        members.runtime_member("replace", "in-place value replacement")?;
    }
    members.declaration("INTRLAY_MEMBER", "generated member-access macro")?;
    Ok(members)
}

pub(super) fn namespace(namespace: &str) -> Result<(), GenerateError> {
    for segment in namespace.split("::") {
        let mut characters = segment.chars();
        if !characters
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !characters.all(|c| c.is_ascii_alphanumeric() || c == '_')
            || super::names::KEYWORDS
                .split_whitespace()
                .any(|keyword| keyword == segment)
            || super::names::OBJECT_MACROS.contains(&segment)
        {
            return Err(GenerateError(format!(
                "invalid C++ namespace {namespace:?}"
            )));
        }
    }
    Ok(())
}

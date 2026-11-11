//! Dart class scopes include unqualified references and constructor parameters.

mod references;

use crate::{
    GenerateError,
    name_validation::{self as shared, NameKind, Scope},
    plan::{Field, GenerationPlan, Id, Kind, TypeGraph},
};

pub fn is_reserved(name: &str, kind: NameKind) -> bool {
    // Accessor keywords can also name ordinary members, including root `set`.
    !(kind == NameKind::Member && matches!(name, "get" | "set"))
        && shared::is_keyword(super::names::KEYWORDS, name)
}

pub fn validate(generation: &GenerationPlan) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let mut declarations = Scope::module(plan);
    for (id, node) in plan.nodes.iter().enumerate() {
        if node.is_enum_input_context() {
            continue;
        }
        let input = node.input_name();
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            declarations.schema(node)?;
        }
        if !generation.encoding.needs_input(id) {
            continue;
        }
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            declarations.declaration(&input, "input value")?;
            shared::input_fields(plan, node, &[])?;
        }
        if let Kind::Record(fields) = &node.kind
            && plan.folded_node(node).is_none()
            && node.numeric_input.is_none()
        {
            native_fields(plan, &input, fields)?;
        }
        if let Kind::Enum(variants) = &node.kind {
            let mut members = Scope::new(plan);
            if node.is_value_enum() {
                let mut locals = Scope::new(plan);
                locals.reserve("tag", "enum tag conversion parameter");
                locals.declaration(&input, "referenced enum")?;
                members.reserve_all(
                    &[
                        "values",
                        "index",
                        "hashCode",
                        "runtimeType",
                        "toString",
                        "noSuchMethod",
                        "core",
                        &input,
                    ],
                    "native enum infrastructure",
                );
                members.runtime_member("tag", "enum discriminant")?;
                members.member(&super::names::enum_from_tag(plan), "enum tag conversion")?;
            }
            for variant in variants {
                members.declaration(&variant.name, "enum variant")?;
                if !node.is_value_enum() {
                    let payload = node.variant_input_name(variant);
                    native_fields(plan, &payload, &variant.fields)?;
                    declarations.declaration(&payload, "variant payload")?;
                }
            }
        }
    }

    let views = &generation.views.types;
    for &id in &generation.views.emitted {
        declarations.declaration(&views.nodes[id].view_name, "view")?;
        view_members(views, id)?;
    }
    shared::input_view_aliases(generation, &mut declarations)?;
    shared::codec_helpers(generation, &mut declarations)
}

fn native_fields(plan: &TypeGraph, enclosing: &str, fields: &[Field]) -> Result<(), GenerateError> {
    let mut members = inherited_members(plan);
    for reference in references::native_fields(plan, fields) {
        members.member(&reference, "referenced native type")?;
    }
    members.reserve(enclosing, "enclosing native type");
    let mut parameters = Scope::new(plan);
    for field in fields {
        members.member(&field.name, "native field")?;
        let parameter = super::constructor_parameter(&field.name);
        if parameter.is_empty() {
            return Err(GenerateError(format!(
                "private Dart field {} has no public constructor parameter name; assign an explicit target name",
                field.name
            )));
        }
        parameters.member(parameter, "constructor parameter")?;
    }
    Ok(())
}

fn view_members(plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    let mut members = infrastructure(plan, id)?;
    for reference in references::view(plan, id)? {
        // A recursive accessor may refer to its own enclosing class.
        if reference != node.view_name {
            members.member(&reference, "referenced type or codec")?;
        }
    }
    if node.options.generation.setters && node.options.values() && !plan.has_payload(id) {
        members.runtime_member("replace", "in-place value replacement")?;
    }
    if matches!(node.kind, Kind::Enum(_)) && node.options.generation.getters {
        members.runtime_member("tag", "enum tag accessor")?;
    }
    shared::field_accessors(plan, id, &mut members, false)?;
    match node.kind {
        Kind::Sequence(_) | Kind::Array { .. } => {
            // Element operators keep their []/[]= spelling; only length is a named member.
            members.runtime_member("length", "collection length")?;
            shared::indexed_accessors(plan, id, &mut members)
        }
        Kind::Scalar(_) | Kind::Unit | Kind::String | Kind::Optional { .. } | Kind::Boxed(_) => {
            shared::root_accessors(plan, id, &mut members, false)
        }
        _ => Ok(()),
    }
}

fn infrastructure(plan: &TypeGraph, id: Id) -> Result<Scope<'_>, GenerateError> {
    let node = &plan.nodes[id];
    let mut members = inherited_members(plan);
    members.reserve(&node.view_name, "enclosing view type");
    for key in ["byte_size", "alignment", "storage"] {
        members.runtime_member(key, "view infrastructure")?;
    }
    if node.options.generation.create {
        for key in ["create", "create_in_place", "compute_byte_length"] {
            members.runtime_member(key, "view construction")?;
        }
    }

    // Dart omits unused storage to satisfy its analyzer. Use the same semantic
    // markers as view emission, including zero-sized and omitted accessors.
    let mut body = String::new();
    super::view_members(&mut body, plan, id, node)?;
    if matches!(node.kind, Kind::Sequence(_)) {
        members.runtime_member("element_count", "cached collection layout")?;
        if body.contains("$[element_offset]") {
            members.runtime_member("element_offset", "cached collection layout")?;
        }
    } else if body.contains("$[offset]") {
        members.runtime_member("offset", "view infrastructure")?;
    }
    Ok(members)
}

fn inherited_members(plan: &TypeGraph) -> Scope<'_> {
    let mut members = Scope::new(plan);
    // Import prefixes used inside class bodies cannot be shadowed by members.
    // `convert` is used only by module-level codecs.
    members.reserve_all(
        &[
            "hashCode",
            "runtimeType",
            "toString",
            "noSuchMethod",
            "core",
            "typed",
            "intrlay",
        ],
        "inherited object member",
    );
    members
}

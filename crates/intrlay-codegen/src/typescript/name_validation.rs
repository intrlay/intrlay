//! TypeScript input objects, native enum objects, and class declaration scopes.

use crate::{
    GenerateError,
    api::CollectionAccessors,
    name_validation::{self as shared, NameKind, Scope},
    plan::{GenerationPlan, Id, Kind, TypeGraph},
};

pub fn is_reserved(name: &str, kind: NameKind) -> bool {
    kind == NameKind::Declaration && shared::is_keyword(super::names::KEYWORDS, name)
}

pub fn validate(generation: &GenerationPlan) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let mut declarations = Scope::module(plan);
    for (id, node) in plan.nodes.iter().enumerate() {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            declarations.schema(node)?;
            if generation.encoding.needs_input(id) {
                declarations.declaration(&node.input_name(), "input value")?;
                shared::input_fields(plan, node, &["kind"])?;
            }
        }
        if let Kind::Enum(variants) = &node.kind {
            if !node.is_value_enum() {
                declarations.declaration(&node.kind_name(), "enum kind")?;
            }
            let mut members = Scope::new(plan);
            // isolatedModules preserves const enums as objects. Assigning
            // __proto__ invokes the prototype setter instead of storing a tag.
            members.reserve("__proto__", "numeric enum object prototype");
            for variant in variants {
                members.declaration(&variant.name, "enum variant")?;
            }
        }
    }

    let views = &generation.views.types;
    for &id in &generation.views.emitted {
        declarations.declaration(&views.nodes[id].view_name, "view")?;
        view_members(views, id)?;
    }
    shared::input_view_aliases(generation, &mut declarations)?;
    for (alias, id) in &views.exports {
        let node = &views.nodes[*id];
        if *alias != node.name && matches!(node.kind, Kind::Enum(_)) && !node.is_value_enum() {
            declarations.declaration(&format!("{alias}Kind"), "export enum kind alias")?;
        }
    }
    shared::codec_helpers(generation, &mut declarations)
}

fn view_members(plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let mut members = infrastructure(plan, id)?;
    shared::field_accessors(plan, id, &mut members, false)?;
    shared::indexed_accessors(plan, id, &mut members)
}

fn infrastructure(plan: &TypeGraph, id: Id) -> Result<Scope<'_>, GenerateError> {
    let node = &plan.nodes[id];
    // JavaScript classes have distinct static and instance scopes. Validate
    // each before combining reservations against schema-defined members.
    let mut instances = Scope::new(plan);
    instances.reserve("constructor", "view constructor");
    for key in ["bytes", "storage"] {
        instances.runtime_member(key, "view instance infrastructure")?;
    }
    let mut body = String::new();
    super::view_members(&mut body, plan, id, node)?;
    for key in ["offset", "element_offset", "element_count"] {
        if body.contains(&format!("$[{key}]")) {
            instances.runtime_member(key, "view storage")?;
        }
    }
    if node.options.generation.setters && !plan.has_payload(id) {
        instances.runtime_member("replace", "in-place value replacement")?;
    }
    match node.kind {
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            instances.runtime_member("length", "collection length")?;
            let api = CollectionAccessors::new(plan, node, element);
            if api.getter {
                instances.runtime_member("get", "collection element getter")?;
            }
            if api.setter.is_some() {
                instances.runtime_member("set", "collection element setter")?;
            }
        }
        Kind::Enum(_) if node.options.generation.getters => {
            instances.runtime_member("tag", "enum tag accessor")?;
        }
        Kind::Scalar(_) | Kind::Unit | Kind::String | Kind::Optional { .. } | Kind::Boxed(_) => {
            shared::root_accessors(plan, id, &mut instances, false)?;
        }
        _ => {}
    }
    let mut statics = Scope::new(plan);
    for key in ["byte_size", "alignment"] {
        statics.runtime_member(key, "view static infrastructure")?;
    }
    if node.options.generation.create {
        for key in ["create", "create_in_place", "compute_byte_length"] {
            statics.runtime_member(key, "view static infrastructure")?;
        }
    }
    instances.extend(statics);
    Ok(instances)
}

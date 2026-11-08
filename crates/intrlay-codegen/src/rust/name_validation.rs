//! Names emitted directly or added by the Rust derive to owned schema types.

use crate::{
    GenerateError,
    name_validation::{self as shared, NameKind, Scope},
    plan::{Field, GenerationPlan, Id, Kind, TypeGraph},
};

pub fn is_reserved(name: &str, _kind: NameKind) -> bool {
    shared::is_keyword(super::names::KEYWORDS, name)
}

pub fn validate(generation: &GenerationPlan) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let mut declarations = Scope::module(plan);
    for node in &plan.nodes {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            declarations.input(node, &node.name)?;
            declarations.declaration(&node.input_name(), "encoding input")?;
            shared::input_fields(plan, node, &[])?;
        }
        if let Kind::Record(fields) = &node.kind {
            derive_field_names(plan, fields)?;
        }
        if let Kind::Enum(variants) = &node.kind {
            let mut members = Scope::new(plan);
            for variant in variants {
                members.declaration(&variant.name, "enum variant")?;
                if !variant.synthetic_payload {
                    derive_field_names(plan, &variant.fields)?;
                }
            }
        }
    }

    let views = &generation.views.types;
    for &id in &generation.views.emitted {
        declarations.declaration(&views.nodes[id].view_name, "view")?;
        view_members(views, id)?;
    }
    for (alias, id) in &views.exports {
        let node = &views.nodes[*id];
        let declared = matches!(node.kind, Kind::Record(_) | Kind::Enum(_));
        if !declared || *alias != node.name {
            declarations.declaration(alias, "export input alias")?;
            if declared {
                declarations.declaration(&format!("{alias}Input"), "export input alias")?;
            }
        }
        if *alias != node.name {
            declarations.declaration(&format!("{alias}View"), "export view alias")?;
        }
    }
    for node in &plan.nodes {
        if let Kind::Enum(variants) = &node.kind
            && variants.iter().any(|variant| !variant.fields.is_empty())
        {
            declarations.declaration(&format!("{}VariantView", node.name), "variant view")?;
        }
    }
    Ok(())
}

fn derive_field_names(plan: &TypeGraph, fields: &[Field]) -> Result<(), GenerateError> {
    for field in fields {
        if field.name != field.property_name {
            // field_name emits an annotation parsed as a standalone identifier by
            // derive, even when only prefixed enum mutators appear in the view.
            // This checks its spelling without declaring a nonexistent getter.
            Scope::new(plan).member(&field.property_name, "derive field identifier")?;
        }
    }
    Ok(())
}

fn view_members(plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let mut members = Scope::new(plan);
    // Derive supplies these methods with fixed names; runtime naming rules do
    // not rename them. Struct fields occupy a separate namespace from methods.
    members.reserve_all(
        &[
            "as_bytes",
            "as_view",
            "as_view_mut",
            "from_bytes_unchecked",
            "from_parts_unchecked",
            "into_buffer",
            "replace",
            "LAYOUT",
        ],
        "view infrastructure",
    );
    if plan.nodes[id].options.generation.create {
        members.reserve_all(
            &[
                "compute_layout",
                "compute_layout_from",
                "create",
                "create_from",
                "create_in_place",
                "create_in_place_from",
            ],
            "view construction",
        );
    }
    if matches!(plan.nodes[id].kind, Kind::Enum(_)) {
        members.member("variant", "active variant accessor")?;
    }
    shared::field_accessors(plan, id, &mut members, false)
}

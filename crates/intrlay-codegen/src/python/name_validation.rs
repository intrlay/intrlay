//! Python `IntEnum` declarations, inherited descriptors, and annotation lookup.

mod references;

use crate::{
    GenerateError,
    api::CollectionAccessors,
    name_validation::{self as shared, NameKind, Scope},
    plan::{GenerationPlan, Id, Kind, TypeGraph},
};

pub fn is_reserved(name: &str, _kind: NameKind) -> bool {
    shared::is_keyword(super::names::KEYWORDS, name)
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
            let kind = if node.is_value_enum() {
                node.input_name()
            } else {
                let kind = node.kind_name();
                declarations.declaration(&kind, "enum kind")?;
                kind
            };
            let mut members = Scope::new(plan);
            for variant in variants {
                enum_member(&kind, &variant.name)?;
                members.declaration(&variant.name, "enum variant")?;
                if !node.is_value_enum() && generation.encoding.needs_input(id) {
                    declarations
                        .declaration(&node.variant_input_name(variant), "variant payload")?;
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
    for (alias, id) in &views.exports {
        let node = &views.nodes[*id];
        if *alias != node.name && matches!(node.kind, Kind::Enum(_)) && !node.is_value_enum() {
            declarations.declaration(&format!("{alias}Kind"), "export enum kind alias")?;
        }
    }
    for (alias, _) in super::private_aliases(generation)? {
        declarations.declaration(&alias, "private-name reference")?;
    }
    shared::codec_helpers(generation, &mut declarations)
}

fn enum_member(input: &str, name: &str) -> Result<(), GenerateError> {
    let sunder = name.len() > 2
        && name.starts_with('_')
        && name.ends_with('_')
        && !name.starts_with("__")
        && !name.ends_with("__");
    // Double-underscore names may be ignored by Enum or fail to denote enum
    // literals in type checkers, even when they survive class construction.
    let private = name.starts_with("__")
        || (!name.ends_with("__")
            && name
                .strip_prefix(&format!("_{input}__"))
                .is_some_and(|suffix| !suffix.is_empty()));
    // IntEnum members must also type-check against inherited Enum/int members.
    // `value`, `real`, and `numerator` remain compatible integer-valued names.
    let inherited = matches!(
        name,
        "name"
            | "mro"
            | "as_integer_ratio"
            | "bit_count"
            | "bit_length"
            | "conjugate"
            | "denominator"
            | "from_bytes"
            | "imag"
            | "is_integer"
            | "to_bytes"
    );
    if inherited || sunder || private {
        return Err(GenerateError(format!(
            "{name} conflicts with Python IntEnum members or enum literal typing; assign an explicit target name"
        )));
    }
    Ok(())
}

fn view_members(plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    let mut members = Scope::new(plan);
    members.reserve_all(
        &[
            "__new__",
            "__class__",
            "__dict__",
            "__weakref__",
            "__getattribute__",
            "__getattr__",
            "__setattr__",
            "__delattr__",
            "__init_subclass__",
            "__class_getitem__",
            "__del__",
            "__dir__",
            "__doc__",
            "__eq__",
            "__format__",
            "__ge__",
            "__getstate__",
            "__gt__",
            "__hash__",
            "__le__",
            "__lt__",
            "__ne__",
            "__reduce__",
            "__reduce_ex__",
            "__repr__",
            "__sizeof__",
            "__str__",
            "__subclasshook__",
        ],
        "inherited object member",
    );
    // Imported runtime View members do not follow the generated naming policy.
    members.reserve_all(
        &[
            "__init__",
            "__slots__",
            "_storage",
            "buffer",
            "_intrlay",
            "_builtins",
            "_typing",
            "_abc",
        ],
        "view infrastructure",
    );
    let mut body = String::new();
    let descriptors = super::view_members(&mut body, id, node, plan)?;
    if descriptors || body.contains("self._offset") {
        members.member("_offset", "view infrastructure")?;
    }
    for key in ["element_offset", "element_count"] {
        if body.contains(&format!("$[{key}]")) {
            members.runtime_member(key, "cached collection layout")?;
        }
    }
    // Inherited storage names are literal; only these generated declarations
    // follow the naming policy. Check them against each other and the base API.
    for key in ["byte_size", "alignment"] {
        members.runtime_member(key, "view infrastructure")?;
    }
    if node.options.generation.create {
        for key in ["create", "create_in_place", "compute_byte_length"] {
            members.runtime_member(key, "view construction")?;
        }
    }
    for reference in references::view(plan, id)? {
        members.member(&reference, "referenced type or codec")?;
    }
    if node.options.generation.setters && !plan.has_payload(id) {
        members.runtime_member("replace", "in-place value replacement")?;
    }
    if matches!(node.kind, Kind::Enum(_)) && node.options.generation.getters {
        members.runtime_member("tag", "enum tag accessor")?;
    }
    shared::field_accessors(plan, id, &mut members, false)?;
    match node.kind {
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            // Python's collection methods retain their language-defined names.
            members.member("__len__", "collection length")?;
            let api = CollectionAccessors::new(plan, node, element);
            if api.getter {
                members.member("__getitem__", "collection element getter")?;
                members.member("__iter__", "collection iterator")?;
            }
            if api.setter.is_some() {
                members.member("__setitem__", "collection element setter")?;
            }
            shared::indexed_accessors(plan, id, &mut members)
        }
        Kind::Scalar(_) | Kind::Unit | Kind::String | Kind::Optional { .. } | Kind::Boxed(_) => {
            shared::root_accessors(plan, id, &mut members, false)
        }
        _ => Ok(()),
    }
}

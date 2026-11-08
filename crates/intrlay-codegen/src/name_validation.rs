//! Shared name scopes and accessor traversal. Target declaration rules belong
//! to each backend's `name_validation` module, beside the code that emits them.

use std::collections::BTreeMap;

use crate::{
    GenerateError,
    api::{Accessor, IndexedAccessors},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph, Variant},
    symbols,
};

/// Identifier context matters: some languages allow keywords as member names.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Declaration,
    Member,
}

/// One target-language lookup scope. A backend decides which declarations and
/// inherited names share the scope; this type checks their final spellings.
pub struct Scope<'a> {
    plan: &'a TypeGraph,
    names: BTreeMap<String, String>,
}

impl<'a> Scope<'a> {
    pub fn new(plan: &'a TypeGraph) -> Self {
        Self {
            plan,
            names: BTreeMap::new(),
        }
    }

    pub fn module(plan: &'a TypeGraph) -> Self {
        let mut scope = Self::new(plan);
        scope.reserve_all(
            plan.backend.reserved_declarations,
            "generated module infrastructure",
        );
        scope
    }

    pub fn declaration(&mut self, name: &str, role: &str) -> Result<(), GenerateError> {
        self.claim(name, role, NameKind::Declaration)
    }

    pub fn member(&mut self, name: &str, role: &str) -> Result<(), GenerateError> {
        self.claim(name, role, NameKind::Member)
    }

    fn claim(&mut self, name: &str, role: &str, kind: NameKind) -> Result<(), GenerateError> {
        if (self.plan.backend.is_reserved_name)(name, kind) {
            return Err(GenerateError(format!(
                "{name} is a reserved {:?} identifier; assign an explicit target name",
                self.plan.backend.language
            )));
        }
        if let Some(previous) = self.names.insert(name.into(), role.into()) {
            return Err(GenerateError(format!(
                "generated name {name} conflicts between {previous} and {role}; assign an explicit target name"
            )));
        }
        Ok(())
    }

    /// Reserve a runtime or inherited name without declaring it again. This
    /// also lets backends combine separate scopes when checking user members.
    pub fn reserve(&mut self, name: &str, role: &str) {
        self.names.insert(name.into(), role.into());
    }

    pub fn reserve_all(&mut self, names: &[&str], role: &str) {
        for name in names {
            self.reserve(name, role);
        }
    }

    pub fn extend(&mut self, other: Self) {
        self.names.extend(other.names);
    }

    pub fn runtime_member(&mut self, key: &str, role: &str) -> Result<(), GenerateError> {
        self.member(&symbols::runtime_symbol(self.plan, key)?, role)
    }

    pub fn schema(&self, node: &Node) -> Result<(), GenerateError> {
        // A schema name must remain usable if the target becomes a frontend.
        // Check it separately from the emitted name: Input is not keyword escaping.
        Self::module(self.plan).declaration(&node.name, "schema name")
    }

    pub fn input(&mut self, node: &Node, name: &str) -> Result<(), GenerateError> {
        self.schema(node)?;
        self.declaration(name, "input value")
    }
}

pub fn is_keyword(keywords: &str, name: &str) -> bool {
    keywords.split_whitespace().any(|keyword| name == keyword)
}

pub fn collection_layout(
    plan: &TypeGraph,
    id: Id,
    members: &mut Scope<'_>,
) -> Result<(), GenerateError> {
    if matches!(plan.nodes[id].kind, Kind::Sequence(_)) {
        for key in ["element_offset", "element_count"] {
            members.runtime_member(key, "cached collection layout")?;
        }
    }
    Ok(())
}

/// Input field scopes exist independently of emitted views. The backend supplies
/// any discriminant that shares each variant's payload scope.
pub fn input_fields(
    plan: &TypeGraph,
    node: &Node,
    payload_reserved: &[&str],
) -> Result<(), GenerateError> {
    let fields = |fields: &[Field], reserved: &[&str]| {
        let mut input = Scope::new(plan);
        input.reserve_all(reserved, "variant discriminant");
        for field in fields {
            input.member(&field.name, "input field")?;
        }
        Ok::<(), GenerateError>(())
    };
    match &node.kind {
        Kind::Record(record) => fields(record, &[])?,
        Kind::Enum(variants) => {
            for variant in variants {
                fields(&variant.fields, payload_reserved)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Names of accessors actually emitted into a view's member scope.
pub fn field_accessors(
    plan: &TypeGraph,
    id: Id,
    members: &mut Scope<'_>,
    read_only: bool,
) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    match &node.kind {
        Kind::Record(fields) => fields_accessors(plan, node, fields, None, members, read_only)?,
        Kind::Enum(variants) => {
            for variant in variants {
                fields_accessors(
                    plan,
                    node,
                    &variant.fields,
                    Some(variant),
                    members,
                    read_only,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn fields_accessors(
    plan: &TypeGraph,
    node: &Node,
    fields: &[Field],
    variant: Option<&Variant>,
    members: &mut Scope<'_>,
    read_only: bool,
) -> Result<(), GenerateError> {
    for field in fields {
        for (name, role) in Accessor::field(plan, node, field, variant).declarations(read_only) {
            members.member(name, role)?;
        }
    }
    Ok(())
}

/// Indexed record accessors share a scope with the backend's collection API.
/// The backend registers its own named methods or language-defined operators.
pub fn indexed_accessors(
    plan: &TypeGraph,
    id: Id,
    members: &mut Scope<'_>,
) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    match node.kind {
        Kind::Sequence(element) | Kind::Array { element, .. }
            if plan.folded_field(element).is_none() =>
        {
            if let Kind::Record(fields) = &plan.nodes[element].kind {
                for field in fields {
                    for (name, role) in
                        IndexedAccessors::new(plan, node, &plan.nodes[element], field)
                            .declarations()
                    {
                        members.member(name, role)?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Root getters and setters use the same final names as the accessor planner.
pub fn root_accessors(
    plan: &TypeGraph,
    id: Id,
    members: &mut Scope<'_>,
    read_only: bool,
) -> Result<(), GenerateError> {
    for (name, role) in Accessor::root(plan, id)?.declarations(read_only) {
        let role = match role {
            "field accessor" => "value accessor",
            "field setter" => "value setter method",
            other => other,
        };
        members.member(name, role)?;
    }
    Ok(())
}

/// The usual target aliases export an Input and a View, without an owned value.
pub fn input_view_aliases(
    generation: &GenerationPlan,
    declarations: &mut Scope<'_>,
) -> Result<(), GenerateError> {
    let plan = &generation.views.types;
    for (alias, id) in &plan.exports {
        let node = &plan.nodes[*id];
        if generation.encoding.needs_input(node.codec)
            && (!matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) || *alias != node.name)
        {
            declarations.declaration(&format!("{alias}Input"), "export input alias")?;
        }
        if *alias != node.name && generation.views.emitted.contains(id) {
            declarations.declaration(&format!("{alias}View"), "export view alias")?;
        }
    }
    Ok(())
}

/// Check the final helper spellings in the scope chosen by the backend.
pub fn codec_helpers(
    generation: &GenerationPlan,
    scope: &mut Scope<'_>,
) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    for (operation, helpers) in [
        ("measure", &generation.encoding.measures),
        ("write", &generation.encoding.writers),
    ] {
        for &id in helpers {
            scope.declaration(
                &symbols::codec_function(plan, operation, id)?,
                "codec helper",
            )?;
        }
    }
    Ok(())
}

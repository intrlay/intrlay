//! Unqualified type and codec references share a Dart class's member namespace.

use std::collections::BTreeSet;

use intrlay_schema::Exposure;

use crate::{
    GenerateError,
    api::{Accessor, CollectionAccessors, IndexedAccessors},
    name_validation::Scope,
    plan::{Field, Id, Kind, TypeGraph},
    symbols,
};

pub(super) fn native_fields(plan: &TypeGraph, fields: &[Field]) -> BTreeSet<String> {
    let mut references = References::new(plan);
    for field in fields {
        references.input(field.ty);
    }
    references.names
}

pub(super) fn view(plan: &TypeGraph, id: Id) -> Result<BTreeSet<String>, GenerateError> {
    let mut references = References::new(plan);
    let node = &plan.nodes[id];
    if node.options.generation.create {
        references.input(id);
        references.codec("write", id)?;
        if plan.has_payload(id) {
            references.codec("measure", id)?;
        }
    }
    if node.options.generation.setters && node.options.values() && !plan.has_payload(id) {
        references.input(id);
        references.codec("write", node.codec)?;
    }
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                references.field(id, field, None)?;
            }
        }
        Kind::Enum(variants) => {
            for variant in variants {
                for field in &variant.fields {
                    references.field(id, field, Some(variant))?;
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            let api = CollectionAccessors::new(plan, node, *element);
            if api.getter {
                references.output(*element);
                references.load(*element, plan.nodes[*element].options.exposure, true, false)?;
            }
            if api.setter.is_some() {
                references.mutation(*element)?;
            }
            if plan.folded_field(*element).is_none() {
                for (field, variant) in plan.nodes[*element].fields() {
                    let api =
                        IndexedAccessors::new(plan, node, &plan.nodes[*element], field, variant);
                    if api.getter.is_some() {
                        references.output(field.ty);
                        references.load(
                            field.ty,
                            field.options.exposure,
                            true,
                            variant.is_some(),
                        )?;
                    }
                    if api.setter.is_some() {
                        references.mutation(field.ty)?;
                    }
                }
            }
        }
        _ => {
            let api = Accessor::root(plan, id)?;
            if api.getter {
                references.output(id);
                references.load(id, node.options.exposure, false, false)?;
            }
            if api.setter.is_some() {
                references.mutation(id)?;
            }
        }
    }
    Ok(references.names)
}

struct References<'a> {
    plan: &'a TypeGraph,
    names: BTreeSet<String>,
}

impl<'a> References<'a> {
    fn new(plan: &'a TypeGraph) -> Self {
        Self {
            plan,
            names: BTreeSet::new(),
        }
    }

    fn insert(&mut self, name: String) {
        self.names.insert(name);
    }

    /// Native aliases remain named in input signatures, even when folded.
    fn input(&mut self, id: Id) {
        if self.plan.nodes[id].numeric_input.is_some() {
            return;
        }
        match self.plan.nodes[id].kind {
            Kind::Record(_) | Kind::Enum(_) => self.insert(self.plan.input_name(id)),
            Kind::Sequence(value)
            | Kind::Array { element: value, .. }
            | Kind::Optional { value, .. }
            | Kind::Boxed(value) => self.input(value),
            _ => {}
        }
    }

    fn output(&mut self, id: Id) {
        let node = &self.plan.nodes[id];
        if matches!(node.options.exposure, Exposure::TypedArray(_)) {
            return;
        }
        if let Some(field) = self.plan.folded_field(id) {
            self.output(field.ty);
            return;
        }
        match node.kind {
            Kind::Optional { value, .. } | Kind::Boxed(value) => self.output(value),
            Kind::Enum(_) if node.is_value_enum() => self.insert(node.tag_type_name()),
            Kind::Scalar(_) | Kind::String | Kind::Unit => {}
            _ => self.insert(self.plan.view_name(id)),
        }
    }

    fn projected(&mut self, id: Id, exposure: Exposure) {
        if matches!(exposure, Exposure::TypedArray(_)) {
            return;
        }
        if matches!(
            self.plan.nodes[id].options.exposure,
            Exposure::TypedArray(_)
        ) {
            self.insert(if self.plan.nodes[id].is_value_enum() {
                self.plan.nodes[id].tag_type_name()
            } else {
                self.plan.view_name(id)
            });
        } else {
            self.output(id);
        }
    }

    /// A folded accessor can use a field-specific projection in its body.
    fn load(
        &mut self,
        id: Id,
        exposure: Exposure,
        indexed: bool,
        position: bool,
    ) -> Result<(), GenerateError> {
        if matches!(exposure, Exposure::TypedArray(_)) {
            return Ok(());
        }
        if let Some(field) = self.plan.folded_field(id) {
            return self.load(field.ty, field.options.exposure, indexed, position);
        }
        match self.plan.nodes[id].kind {
            Kind::Optional { value, .. } | Kind::Boxed(value) => {
                self.load(
                    value,
                    self.plan.nodes[value].options.exposure,
                    indexed,
                    true,
                )?;
            }
            Kind::Enum(_) if self.plan.nodes[id].is_value_enum() => {
                let name = self.plan.nodes[id].tag_type_name();
                let mut locals = Scope::new(self.plan);
                if indexed {
                    locals.reserve("index", "enum reader parameter");
                }
                if position {
                    locals.reserve("position", "enum reader local");
                }
                locals.declaration(&name, "referenced enum")?;
                self.insert(name);
            }
            Kind::Scalar(_) | Kind::String | Kind::Unit => {}
            _ => self.insert(self.plan.view_name(id)),
        }
        Ok(())
    }

    fn codec(&mut self, operation: &str, id: Id) -> Result<(), GenerateError> {
        self.insert(symbols::codec_function(self.plan, operation, id)?);
        Ok(())
    }

    fn mutation(&mut self, id: Id) -> Result<(), GenerateError> {
        if let Some(mutation) = self.plan.mutation(id) {
            self.input(mutation.value);
            let value = &self.plan.nodes[mutation.value];
            if value.numeric_input.is_some() || !matches!(value.kind, Kind::Scalar(_)) {
                self.codec("write", mutation.value)?;
            }
        }
        Ok(())
    }

    fn field(
        &mut self,
        owner: Id,
        field: &Field,
        variant: Option<&crate::plan::Variant>,
    ) -> Result<(), GenerateError> {
        let api = Accessor::field(self.plan, &self.plan.nodes[owner], field, variant);
        if api.getter {
            self.projected(field.ty, field.options.exposure);
            self.load(field.ty, field.options.exposure, false, false)?;
        }
        if api.setter.is_some() {
            self.mutation(field.ty)?;
        }
        Ok(())
    }
}

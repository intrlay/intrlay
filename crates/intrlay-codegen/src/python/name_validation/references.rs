//! Python annotations see class members; method bodies resolve codec functions
//! at module scope. Only annotation and cached-slot references are reserved here.

use std::collections::BTreeSet;

use intrlay_schema::Exposure;

use crate::{
    GenerateError,
    api::{Accessor, IndexedAccessors},
    plan::{Field, Id, Kind, TypeGraph, Variant},
};

pub(super) fn view(plan: &TypeGraph, id: Id) -> Result<BTreeSet<String>, GenerateError> {
    let mut references = References {
        plan,
        names: BTreeSet::new(),
    };
    let node = &plan.nodes[id];
    if node.options.generation.create || (node.options.generation.setters && !plan.has_payload(id))
    {
        references.input(id);
    }
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                references.field(id, field, None);
            }
        }
        Kind::Enum(variants) => {
            for variant in variants {
                for field in &variant.fields {
                    references.field(id, field, Some(variant));
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            if node.options.generation.getters
                || (node.options.generation.setters && plan.borrows_content(*element))
            {
                references.output(*element);
            }
            if node.options.generation.setters && plan.nodes[*element].options.generation.setters {
                references.mutation(*element);
            }
            if plan.folded_field(*element).is_none()
                && let Kind::Record(fields) = &plan.nodes[*element].kind
            {
                for field in fields {
                    let api = IndexedAccessors::new(plan, node, &plan.nodes[*element], field);
                    if api.getter.is_some() {
                        references.output(field.ty);
                    }
                    if api.setter.is_some() {
                        references.mutation(field.ty);
                    }
                }
            }
        }
        _ => {
            let api = Accessor::root(plan, id)?;
            if api.getter {
                references.output(id);
            }
            if api.setter.is_some() {
                references.mutation(id);
            }
        }
    }
    Ok(references.names)
}

struct References<'a> {
    plan: &'a TypeGraph,
    names: BTreeSet<String>,
}

impl References<'_> {
    fn insert(&mut self, name: &str) {
        self.names.insert(super::super::reference_name(name));
    }

    /// Native aliases remain named in input signatures, even when folded.
    fn input(&mut self, id: Id) {
        match self.plan.nodes[id].kind {
            Kind::Record(_) | Kind::Enum(_) => self.insert(&self.plan.input_name(id)),
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
            Kind::Scalar(_) | Kind::String | Kind::Unit => {}
            _ => self.insert(&self.plan.view_name(id)),
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
            self.insert(&self.plan.view_name(id));
        } else {
            self.output(id);
        }
    }

    fn mutation(&mut self, id: Id) {
        if let Some(mutation) = self.plan.mutation(id) {
            self.input(mutation.value);
        }
    }

    fn field(&mut self, owner: Id, field: &Field, variant: Option<&Variant>) {
        let api = Accessor::field(self.plan, &self.plan.nodes[owner], field, variant);
        if api.getter {
            self.projected(field.ty, field.options.exposure);
        }
        // Cached slots carry annotations even if only a mutator is emitted.
        if api.cache.is_some() {
            if variant.is_some() {
                self.output(field.ty);
            } else {
                self.projected(field.ty, field.options.exposure);
            }
        }
        if api.setter.is_some() {
            self.mutation(field.ty);
        }
    }
}

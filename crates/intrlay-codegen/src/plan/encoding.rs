//! Canonical native types and reachable encoding helpers.

use std::collections::BTreeSet;

use super::{Id, Kind, TypeGraph, field_generation};

pub struct EncodingPlan {
    pub types: TypeGraph,
    pub encoders: BTreeSet<Id>,
    /// Canonical entry points before applying field-specific view projections.
    pub entry_points: BTreeSet<Id>,
}

impl EncodingPlan {
    pub(super) fn new(types: TypeGraph) -> Self {
        let entry_points = types.collect_views();
        let encoders = types.collect_encoders(&entry_points);
        Self {
            types,
            encoders,
            entry_points,
        }
    }
}

impl TypeGraph {
    /// Construction and in-place replacement share the same encoding helpers.
    fn collect_encoders(&self, views: &BTreeSet<Id>) -> BTreeSet<Id> {
        let mut encoders = BTreeSet::new();
        let mut pending: Vec<_> = views
            .iter()
            .copied()
            .filter(|&id| self.nodes[id].options.generation.create)
            .collect();
        for &id in views {
            let node = &self.nodes[id];
            if node.options.generation.setters && !self.has_payload(id) {
                pending.push(id);
            }
            match &node.kind {
                Kind::Record(fields) => {
                    for field in fields {
                        if field_generation(node, field).setters
                            && let Some(mutation) = self.mutation(field.ty)
                        {
                            pending.push(mutation.value);
                        }
                    }
                }
                Kind::Enum(variants) => {
                    for field in variants.iter().flat_map(|variant| &variant.fields) {
                        if field_generation(node, field).setters
                            && let Some(mutation) = self.mutation(field.ty)
                        {
                            pending.push(mutation.value);
                        }
                    }
                }
                Kind::Sequence(element) | Kind::Array { element, .. } => {
                    if node.options.generation.setters
                        && let Some(mutation) = self.mutation(*element)
                    {
                        pending.push(mutation.value);
                    }
                }
                _ => {
                    if let Some(mutation) = self.mutation(id) {
                        pending.push(mutation.value);
                    }
                }
            }
        }
        while let Some(id) = pending.pop() {
            if !encoders.insert(id) {
                continue;
            }
            match &self.nodes[id].kind {
                Kind::Record(fields) => pending.extend(fields.iter().map(|field| field.ty)),
                Kind::Enum(variants) => pending.extend(
                    variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| field.ty)),
                ),
                Kind::Sequence(value)
                | Kind::Array { element: value, .. }
                | Kind::Optional { value, .. }
                | Kind::Boxed(value) => pending.push(*value),
                _ => {}
            }
        }
        encoders
    }
}

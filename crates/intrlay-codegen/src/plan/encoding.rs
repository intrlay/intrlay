//! Canonical native types and reachable encoding helpers.

use std::collections::BTreeSet;

use intrlay_schema::Scalar;

use super::{Id, Kind, TypeGraph, field_generation};

pub struct EncodingPlan {
    pub types: TypeGraph,
    /// Types referenced by encoding inputs, even if no helper call is required.
    pub encoders: BTreeSet<Id>,
    /// Helpers actually called by generated views or other encoding helpers.
    pub writers: BTreeSet<Id>,
    pub measures: BTreeSet<Id>,
}

/// Positional writer arguments keep a uniform callback signature. Prefix inputs
/// unused by this layout so generated code also passes unused-parameter checks.
pub struct WriterParameters {
    pub storage: &'static str,
    pub offset: &'static str,
    pub value: &'static str,
    pub cursor: &'static str,
}

impl EncodingPlan {
    /// Reference-type targets omit inputs that no writer accepts. Native value
    /// enums also serve as tag constants, so their declarations remain available
    /// to readers. Value-type targets retain their complete native input APIs.
    pub fn needs_input(&self, id: Id) -> bool {
        !self.types.backend.language.supports_accessor_generation()
            || self.encoders.contains(&id)
            || self.types.nodes[id].is_value_enum()
    }

    pub(super) fn new(types: TypeGraph) -> Self {
        let entry_points = types.collect_views();
        let encoders = types.collect_encoders(&entry_points);
        let (writers, measures) = if types.backend.language.has_value_types() {
            // Native backends expose complete codecs through their runtime APIs.
            (
                encoders.clone(),
                encoders
                    .iter()
                    .copied()
                    .filter(|&id| types.has_payload(id))
                    .collect(),
            )
        } else {
            types.collect_helpers(&entry_points)
        };
        Self {
            types,
            encoders,
            writers,
            measures,
        }
    }
}

impl TypeGraph {
    pub fn writer_parameters(&self, id: Id) -> WriterParameters {
        let node = &self.nodes[id];
        let empty = matches!(&node.kind, Kind::Unit)
            || matches!(&node.kind, Kind::Record(fields) if fields.is_empty());
        let cursor = match &node.kind {
            Kind::Scalar(_) | Kind::Unit | Kind::Array { length: 0, .. } => false,
            Kind::Record(fields) => !fields.is_empty(),
            Kind::Enum(variants) => variants.iter().any(|variant| !variant.fields.is_empty()),
            Kind::Array { element, .. }
                if self.backend.runtime_numeric_array_writers
                    && matches!(self.nodes[*element].kind, Kind::Scalar(scalar) if scalar != Scalar::Bool) =>
            {
                false
            }
            _ => true,
        };
        WriterParameters {
            storage: if empty { "_storage" } else { "storage" },
            offset: if empty { "_offset" } else { "offset" },
            value: if empty { "_value" } else { "value" },
            cursor: if cursor { "cursor" } else { "_cursor" },
        }
    }

    /// A boxed fixed payload has a known allocation size without reading input.
    pub fn measure_parameter(&self, id: Id) -> &'static str {
        if matches!(self.nodes[id].kind, Kind::Boxed(value) if !self.has_payload(value)) {
            "_value"
        } else {
            "value"
        }
    }

    /// Mutation signatures still need scalar types when writes are inlined.
    fn encoding_roots(&self, views: &BTreeSet<Id>, inline_scalars: bool) -> Vec<Id> {
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
            let mut mutation_root = |value: Id| {
                if !inline_scalars || !matches!(self.nodes[value].kind, Kind::Scalar(_)) {
                    pending.push(value);
                }
            };
            match &node.kind {
                Kind::Record(fields) => {
                    for field in fields {
                        if field_generation(node, field).setters
                            && let Some(mutation) = self.mutation(field.ty)
                        {
                            mutation_root(mutation.value);
                        }
                    }
                }
                Kind::Enum(variants) => {
                    for field in variants.iter().flat_map(|variant| &variant.fields) {
                        if field_generation(node, field).setters
                            && let Some(mutation) = self.mutation(field.ty)
                        {
                            mutation_root(mutation.value);
                        }
                    }
                }
                Kind::Sequence(element) | Kind::Array { element, .. } => {
                    if node.options.generation.setters
                        && let Some(mutation) = self.mutation(*element)
                    {
                        mutation_root(mutation.value);
                    }
                }
                _ => {
                    if let Some(mutation) = self.mutation(id) {
                        mutation_root(mutation.value);
                    }
                }
            }
        }
        pending
    }

    fn collect_encoders(&self, views: &BTreeSet<Id>) -> BTreeSet<Id> {
        let mut encoders = BTreeSet::new();
        let mut pending = self.encoding_roots(views, false);
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

    /// Input references and helper calls differ for empty arrays, inlined scalar
    /// mutation, and runtime bulk writers. Follow write and measurement calls
    /// independently so unused helpers and their names can both be omitted.
    fn collect_helpers(&self, views: &BTreeSet<Id>) -> (BTreeSet<Id>, BTreeSet<Id>) {
        let mut writers = BTreeSet::new();
        let mut pending = self.encoding_roots(views, true);
        while let Some(id) = pending.pop() {
            if !writers.insert(id) {
                continue;
            }
            match &self.nodes[id].kind {
                Kind::Record(fields) => pending.extend(fields.iter().map(|field| field.ty)),
                Kind::Enum(variants) => pending.extend(
                    variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| field.ty)),
                ),
                Kind::Array { length: 0, .. } => {}
                Kind::Sequence(element) | Kind::Array { element, .. }
                    if !self.backend.runtime_numeric_array_writers
                        || !matches!(self.nodes[*element].kind, Kind::Scalar(scalar) if scalar != Scalar::Bool) =>
                {
                    pending.push(*element);
                }
                Kind::Optional { value, .. } | Kind::Boxed(value) => pending.push(*value),
                _ => {}
            }
        }

        let mut measures = BTreeSet::new();
        let mut pending: Vec<_> = views
            .iter()
            .copied()
            .filter(|&id| self.nodes[id].options.generation.create && self.has_payload(id))
            .collect();
        while let Some(id) = pending.pop() {
            if !measures.insert(id) {
                continue;
            }
            let mut child = |id| {
                if self.has_payload(id) {
                    pending.push(id);
                }
            };
            match &self.nodes[id].kind {
                Kind::Record(fields) => fields.iter().for_each(|field| child(field.ty)),
                Kind::Enum(variants) => variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .for_each(|field| child(field.ty)),
                Kind::Array { length: 0, .. } => {}
                Kind::Sequence(value)
                | Kind::Array { element: value, .. }
                | Kind::Optional { value, .. }
                | Kind::Boxed(value) => child(*value),
                _ => {}
            }
        }
        (writers, measures)
    }
}

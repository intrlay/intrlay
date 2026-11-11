//! Canonical native types and reachable encoding helpers.

use std::collections::BTreeSet;

use intrlay_schema::Scalar;

use super::{Id, Kind, TypeGraph, ViewPlan};
use crate::api::{Accessor, CollectionAccessors, IndexedAccessors};

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

    pub(super) fn new(types: TypeGraph, views: &ViewPlan) -> Self {
        // Writers follow the APIs actually emitted by projected views. A flattened
        // field can expose leaf setters without exposing its aggregate setter.
        let roots = |inline_scalars| {
            views
                .types
                .encoding_roots(&views.emitted, inline_scalars)
                .into_iter()
                .map(|id| views.types.nodes[id].codec)
                .collect()
        };
        let encoders = types.collect_encoders(roots(false));
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
            let measures = views
                .emitted
                .iter()
                .filter_map(|&id| {
                    let node = &views.types.nodes[id];
                    (node.options.generation.create && types.has_payload(node.codec))
                        .then_some(node.codec)
                })
                .collect();
            types.collect_helpers(roots(true), measures)
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
        if let Some(numeric) = node.numeric_input {
            let stores_bytes = numeric.sequence || numeric.lanes != 0;
            return WriterParameters {
                storage: if stores_bytes { "storage" } else { "_storage" },
                offset: if stores_bytes { "offset" } else { "_offset" },
                value: "value",
                cursor: if numeric.sequence {
                    "cursor"
                } else {
                    "_cursor"
                },
            };
        }
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
            if node.options.generation.setters && node.options.values() && !self.has_payload(id) {
                pending.push(id);
            }
            let mut mutation_root = |value: Id| {
                if !inline_scalars
                    || self.nodes[value].numeric_input.is_some()
                    || !matches!(self.nodes[value].kind, Kind::Scalar(_))
                {
                    pending.push(value);
                }
            };
            match &node.kind {
                Kind::Record(fields) => {
                    for field in fields {
                        if let Some(setter) = Accessor::field(self, node, field, None).setter {
                            mutation_root(setter.mutation.value);
                        }
                    }
                }
                Kind::Enum(variants) => {
                    for variant in variants {
                        for field in &variant.fields {
                            if let Some(setter) =
                                Accessor::field(self, node, field, Some(variant)).setter
                            {
                                mutation_root(setter.mutation.value);
                            }
                        }
                    }
                }
                Kind::Sequence(element) | Kind::Array { element, .. } => {
                    if let Some(mutation) = CollectionAccessors::new(self, node, *element).setter {
                        mutation_root(mutation.value);
                    }
                    let child = &self.nodes[*element];
                    if self.folded_node(child).is_none() {
                        for (field, variant) in child.fields() {
                            if let Some(setter) =
                                IndexedAccessors::new(self, node, child, field, variant).setter
                            {
                                mutation_root(setter.mutation.value);
                            }
                        }
                    }
                }
                _ => {
                    if node.options.values()
                        && node.options.generation.setters
                        && let Some(mutation) = self.mutation(id)
                    {
                        mutation_root(mutation.value);
                    }
                }
            }
        }
        pending
    }

    fn collect_encoders(&self, mut pending: Vec<Id>) -> BTreeSet<Id> {
        let mut encoders = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !encoders.insert(id) {
                continue;
            }
            if self.nodes[id].numeric_input.is_some() {
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
    fn collect_helpers(
        &self,
        mut pending: Vec<Id>,
        measure_roots: Vec<Id>,
    ) -> (BTreeSet<Id>, BTreeSet<Id>) {
        let mut writers = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !writers.insert(id) {
                continue;
            }
            if self.nodes[id].numeric_input.is_some() {
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
        let mut pending = measure_roots;
        while let Some(id) = pending.pop() {
            if !measures.insert(id) {
                continue;
            }
            if self.nodes[id].numeric_input.is_some() {
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

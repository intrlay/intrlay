//! Field-specific projections share the canonical encoding graph. Accessor
//! generation belongs to each declaration and does not restrict child views.

use std::collections::{BTreeMap, BTreeSet};

use intrlay_schema::Exposure;

use super::{Id, Kind, TypeGraph};

/// Context-specific projections and the views actually instantiated by the API.
pub struct ViewPlan {
    pub types: TypeGraph,
    pub emitted: BTreeSet<Id>,
    pub borrowed: BTreeSet<Id>,
}

impl ViewPlan {
    pub(super) fn new(source: &TypeGraph) -> Self {
        let mut builder = ProjectionBuilder {
            source,
            plan: source.clone(),
            contexts: source
                .nodes
                .iter()
                .enumerate()
                .map(|(id, node)| ((id, node.options.exposure), id))
                .collect(),
        };
        // Reserve canonical contexts before following recursive child references.
        for id in 0..source.nodes.len() {
            builder.populate(id, id);
        }
        let types = builder.plan;
        let emitted = types.collect_views();
        let borrowed = types.collect_borrowed_views(&emitted);
        Self {
            types,
            emitted,
            borrowed,
        }
    }
}

struct ProjectionBuilder<'a> {
    source: &'a TypeGraph,
    plan: TypeGraph,
    contexts: BTreeMap<(Id, Exposure), Id>,
}

impl ProjectionBuilder<'_> {
    fn project(&mut self, id: Id, exposure: Exposure) -> Id {
        let key = (id, exposure);
        if let Some(&known) = self.contexts.get(&key) {
            return known;
        }
        let mut node = self.source.nodes[id].clone();
        node.options.exposure = exposure;
        node.options.generation.create = false;
        node.view_name.push_str(match exposure {
            Exposure::Fields => "Fields",
            Exposure::Indexed => "Indexed",
            Exposure::TypedArray(_) => "Numeric",
        });
        let target = self.plan.nodes.len();
        self.contexts.insert(key, target);
        self.plan.nodes.push(node);
        self.populate(id, target);
        target
    }

    fn populate(&mut self, source: Id, target: Id) {
        let mut kind = self.source.nodes[source].kind.clone();
        match &mut kind {
            Kind::Record(fields) => {
                for field in fields {
                    field.ty = self.project(field.ty, field.options.exposure);
                }
            }
            Kind::Enum(variants) => {
                for variant in variants {
                    for field in &mut variant.fields {
                        field.ty = self.project(field.ty, field.options.exposure);
                    }
                }
            }
            _ => {}
        }
        self.plan.nodes[target].kind = kind;
    }
}

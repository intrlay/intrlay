//! Anonymous collection/wrapper APIs inherit the union of their users' needs.
//! Named declarations and explicitly exported roots keep their independent API.

use intrlay_schema::ApiGeneration;

use super::{Kind, TypeGraph};

impl TypeGraph {
    pub(super) fn resolve_synthetic_generation(&mut self) {
        let empty = ApiGeneration {
            getters: false,
            setters: false,
            create: false,
        };
        let independent: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(id, node)| {
                matches!(node.kind, Kind::Record(_) | Kind::Enum(_))
                    || self.exports.values().any(|&root| root == id)
            })
            .collect();
        let limits: Vec<_> = self
            .nodes
            .iter()
            .map(|node| node.options.generation)
            .collect();
        let mut required = vec![empty; self.nodes.len()];
        let mut pending: Vec<_> = independent
            .iter()
            .enumerate()
            .filter_map(|(id, &root)| root.then_some((id, limits[id])))
            .collect();

        while let Some((id, requested)) = pending.pop() {
            let previous = required[id];
            let current = ApiGeneration {
                getters: previous.getters || (requested.getters && limits[id].getters),
                setters: previous.setters || (requested.setters && limits[id].setters),
                create: previous.create || (requested.create && limits[id].create),
            };
            if previous == current {
                continue;
            }
            required[id] = current;
            let mut enqueue = |child: super::Id, options: ApiGeneration| {
                if !independent[child] {
                    pending.push((child, options));
                }
            };
            let mut fields = |fields: &[super::Field]| {
                for field in fields {
                    enqueue(
                        field.ty,
                        ApiGeneration {
                            getters: current.getters && field.options.generation.getters,
                            setters: current.setters && field.options.generation.setters,
                            create: current.create && field.options.generation.create,
                        },
                    );
                }
            };
            match &self.nodes[id].kind {
                Kind::Record(values) => fields(values),
                Kind::Enum(variants) => {
                    for variant in variants {
                        fields(&variant.fields);
                    }
                }
                Kind::Sequence(child)
                | Kind::Array { element: child, .. }
                | Kind::Boxed(child)
                | Kind::Optional { value: child, .. } => enqueue(*child, current),
                _ => {}
            }
        }
        for (id, node) in self.nodes.iter_mut().enumerate() {
            if !independent[id] {
                node.options.generation = required[id];
                if self.backend.language.has_value_types() {
                    node.options.generation.getters = true;
                    node.options.generation.setters = true;
                }
            }
        }
    }
}

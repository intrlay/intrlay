//! Encoding properties depend on inline edges, not on how many paths reach a
//! shared subtype. Classify each node once; projected views retain these facts.

use intrlay_schema::Scalar;

use super::{Id, Kind, Node, TypeGraph};

#[derive(Clone, Copy, Default)]
pub(super) struct Properties {
    pub has_payload: bool,
    pub zeroable: bool,
    pub homogeneous: Homogeneous,
}

#[derive(Clone, Copy, Default)]
pub(super) enum Homogeneous {
    Empty,
    Numeric(Scalar),
    #[default]
    Mixed,
}

impl Homogeneous {
    pub fn matches(self, scalar: Scalar) -> bool {
        match self {
            Self::Empty => true,
            Self::Numeric(actual) => actual == scalar,
            Self::Mixed => false,
        }
    }

    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Empty, value) | (value, Self::Empty) => value,
            (Self::Numeric(left), Self::Numeric(right)) if left == right => self,
            _ => Self::Mixed,
        }
    }
}

impl TypeGraph {
    pub(super) fn classify_nodes(&mut self) {
        let mut cache = vec![None; self.nodes.len()];
        for id in 0..self.nodes.len() {
            self.nodes[id].properties = classify(id, &self.nodes, &mut cache);
        }
    }
}

fn classify(id: Id, nodes: &[Node], cache: &mut [Option<Properties>]) -> Properties {
    if let Some(properties) = cache[id] {
        return properties;
    }
    // Layout validation has rejected inline cycles. Box/sequence edges stop
    // here, so recursive payload types do not revisit an unfinished node.
    let properties = match &nodes[id].kind {
        Kind::Scalar(scalar) => Properties {
            zeroable: true,
            homogeneous: if *scalar == Scalar::Bool {
                Homogeneous::Mixed
            } else {
                Homogeneous::Numeric(*scalar)
            },
            ..Properties::default()
        },
        Kind::Unit => Properties {
            zeroable: true,
            ..Properties::default()
        },
        Kind::String | Kind::Sequence(_) | Kind::Boxed(_) => Properties {
            has_payload: true,
            ..Properties::default()
        },
        Kind::Array { length: 0, .. } => Properties {
            zeroable: true,
            homogeneous: Homogeneous::Empty,
            ..Properties::default()
        },
        Kind::Array { element, .. } => classify(*element, nodes, cache),
        Kind::Optional { value, .. } => Properties {
            has_payload: classify(*value, nodes, cache).has_payload,
            ..Properties::default()
        },
        Kind::Record(fields) => {
            let mut properties = Properties {
                zeroable: true,
                homogeneous: Homogeneous::Empty,
                ..Properties::default()
            };
            for field in fields {
                let child = classify(field.ty, nodes, cache);
                properties.has_payload |= child.has_payload;
                properties.zeroable &= child.zeroable;
                properties.homogeneous = properties.homogeneous.merge(child.homogeneous);
            }
            properties
        }
        Kind::Enum(variants) => Properties {
            has_payload: variants.iter().any(|variant| {
                variant
                    .fields
                    .iter()
                    .any(|field| classify(field.ty, nodes, cache).has_payload)
            }),
            ..Properties::default()
        },
    };
    cache[id] = Some(properties);
    properties
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, error::Error};

    use intrlay::{Intrlay, schema::Registry};
    use intrlay_schema::{Definition, DefinitionKind, Field, Language, Schema, Type, Variant};

    use super::*;
    use crate::backend;

    fn plan(schema: &Schema) -> Result<TypeGraph, crate::GenerateError> {
        TypeGraph::new(schema, backend::for_language(Language::TypeScript), None)
    }

    #[test]
    fn shared_subtypes_are_classified_without_expanding_every_path() -> Result<(), Box<dyn Error>> {
        for is_enum in [false, true] {
            for leaf in [Type::Scalar(Scalar::U32), Type::String] {
                let dynamic = leaf == Type::String;
                let mut schema = Schema::default();
                let mut child = leaf;
                for level in 0..27 {
                    let name = format!("Level{level}");
                    let fields = vec![
                        Field::new("left", child.clone()),
                        Field::new("right", child),
                    ];
                    let kind = if is_enum {
                        DefinitionKind::Enum(
                            fields
                                .into_iter()
                                .zip(0_u32..)
                                .map(|(field, tag)| Variant {
                                    name: field.name.clone(),
                                    tag,
                                    fields: vec![field],
                                    docs: String::new(),
                                    targets: BTreeMap::new(),
                                    synthetic_payload: false,
                                })
                                .collect(),
                        )
                    } else {
                        DefinitionKind::Record(fields)
                    };
                    schema.definitions.insert(
                        name.clone(),
                        Definition {
                            name: name.clone(),
                            docs: String::new(),
                            kind,
                            alignment: 1,
                            targets: BTreeMap::new(),
                            transparent: false,
                        },
                    );
                    child = Type::Named(name);
                }
                schema.exports.insert("Root".into(), child);
                let plan = plan(&schema)?;
                let projected = crate::plan::ViewPlan::new(&plan);
                for plan in [&plan, &projected.types] {
                    let root = plan.exports["Root"];
                    assert_eq!(plan.has_payload(root), dynamic);
                    assert_eq!(plan.zeroable(root), !is_enum && !dynamic);
                    assert_eq!(
                        plan.nodes[root].properties.homogeneous.matches(Scalar::U32),
                        !is_enum && !dynamic
                    );
                    assert!(!plan.nodes[root].properties.homogeneous.matches(Scalar::F32));
                }
            }
        }
        Ok(())
    }

    #[derive(Intrlay)]
    struct Recursive {
        children: Vec<Self>,
        parent: Option<Box<Self>>,
    }

    #[derive(Intrlay)]
    struct Empty {}

    #[derive(Intrlay)]
    struct EmptyPayloadFields {
        empty: [String; 0],
        nested: [[Vec<u64>; 0]; 2],
        number: u32,
    }

    #[derive(Intrlay)]
    enum EmptyPayloadChoice {
        Empty,
        Value(EmptyPayloadFields),
    }

    #[test]
    fn zero_length_arrays_do_not_contribute_element_properties() -> Result<(), Box<dyn Error>> {
        let mut registry = Registry::new();
        registry.export::<[String; 0]>("EmptyStrings");
        registry.export::<[[Vec<u64>; 0]; 2]>("NestedEmptySequences");
        registry.export::<[[String; 1]; 0]>("EmptyOuterArray");
        registry.export::<EmptyPayloadFields>("Fields");
        registry.export::<EmptyPayloadChoice>("Choice");
        registry.export::<Option<EmptyPayloadFields>>("OptionalFields");
        registry.export::<[String; 1]>("NonemptyStrings");
        registry.export::<Vec<[String; 0]>>("Sequence");
        registry.export::<Box<[String; 0]>>("Boxed");
        let plan = plan(&registry.finish())?;
        let projected = crate::plan::ViewPlan::new(&plan);
        for plan in [&plan, &projected.types] {
            for name in ["EmptyStrings", "NestedEmptySequences", "EmptyOuterArray"] {
                let id = plan.exports[name];
                assert!(!plan.has_payload(id), "{name}");
                assert!(plan.zeroable(id), "{name}");
                assert!(matches!(
                    plan.nodes[id].properties.homogeneous,
                    Homogeneous::Empty
                ));
            }
            let fields = plan.exports["Fields"];
            assert!(!plan.has_payload(fields));
            assert!(plan.zeroable(fields));
            assert!(
                plan.nodes[fields]
                    .properties
                    .homogeneous
                    .matches(Scalar::U32)
            );
            assert!(
                !plan.nodes[fields]
                    .properties
                    .homogeneous
                    .matches(Scalar::F32)
            );
            for name in ["Choice", "OptionalFields"] {
                assert!(!plan.has_payload(plan.exports[name]), "{name}");
            }
            for name in ["NonemptyStrings", "Sequence", "Boxed"] {
                assert!(plan.has_payload(plan.exports[name]), "{name}");
            }
        }
        Ok(())
    }

    #[test]
    fn recursion_and_empty_headers_preserve_encoding_properties() -> Result<(), Box<dyn Error>> {
        let mut registry = Registry::new();
        registry.export::<Recursive>("Recursive");
        registry.export::<Option<u32>>("Optional");
        registry.export::<Empty>("Empty");
        let plan = plan(&registry.finish())?;
        assert!(plan.has_payload(plan.exports["Recursive"]));
        assert!(!plan.zeroable(plan.exports["Recursive"]));
        assert!(!plan.has_payload(plan.exports["Optional"]));
        assert!(!plan.zeroable(plan.exports["Optional"]));
        let empty = plan.exports["Empty"];
        assert!(!plan.has_payload(empty));
        assert!(plan.zeroable(empty));
        assert!(
            plan.nodes[empty]
                .properties
                .homogeneous
                .matches(Scalar::U32)
        );
        assert!(
            plan.nodes[empty]
                .properties
                .homogeneous
                .matches(Scalar::F32)
        );
        Ok(())
    }
}

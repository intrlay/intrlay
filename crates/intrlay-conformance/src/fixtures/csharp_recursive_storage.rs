//! Recursive native inputs keep value APIs without recursive CLR generic fields.

use super::Fixtures;

use std::{error::Error, path::Path};

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

#[derive(Intrlay)]
struct OptionalNode {
    id: u32,
    children: Vec<Option<OptionalNode>>,
}

#[derive(Intrlay)]
struct DirectChildren {
    children: Vec<DirectChildren>,
    #[intrlay(csharp(name = "_childrenMemory"))]
    value: u32,
}
#[derive(Intrlay)]
struct ArrayNode {
    children: Vec<[ArrayNode; 2]>,
}
#[derive(Intrlay)]
struct NestedNode {
    children: Vec<Vec<NestedNode>>,
}
#[derive(Intrlay)]
struct WrappedNode {
    children: Option<Vec<Option<WrappedNode>>>,
}
#[derive(Intrlay)]
struct InlineNode {
    children: [Vec<Option<InlineNode>>; 2],
}
#[derive(Intrlay)]
struct NullableInlineNode {
    children: [Option<Vec<Self>>; 2],
}
#[derive(Intrlay)]
struct NestedNullableInlineNode {
    children: [Option<[Vec<Self>; 2]>; 2],
}
#[derive(Intrlay)]
struct EmptyNode {
    children: [Vec<EmptyNode>; 0],
}
#[derive(Intrlay)]
struct MutualParent {
    children: Vec<MutualChild>,
}
#[derive(Intrlay)]
struct MutualChild {
    parent: Option<MutualParent>,
}
#[derive(Intrlay)]
struct PlainNode {
    children: Vec<PlainNode>,
}
#[derive(Intrlay)]
#[allow(
    clippy::vec_box,
    reason = "explicit boxed recursion is a representation control"
)]
struct BoxedNode {
    children: Vec<Box<BoxedNode>>,
}

#[derive(Intrlay)]
enum RecursiveEnum {
    Empty,
    Direct { items: [Vec<Self>; 2] },
    OptionalElements { items: [Vec<Option<Self>>; 2] },
    OptionalArray { items: Option<[Vec<Self>; 2]> },
    OptionalLists { items: [Option<Vec<Self>>; 2] },
    NestedArrays { items: [[Vec<Self>; 2]; 2] },
    Records { items: [RecursiveBranch; 2] },
    Mutual { items: [Vec<RecursiveParent>; 2] },
    Zero { items: [Vec<Self>; 0] },
    NestedZero { items: [[Vec<Self>; 0]; 2] },
    Plain { items: Vec<Self> },
    Acyclic { items: [Vec<u32>; 2] },
    Pod { items: [u32; 4] },
}

#[derive(Intrlay)]
struct RecursiveBranch {
    id: u32,
    children: Vec<RecursiveEnum>,
}

#[derive(Intrlay)]
enum RecursiveParent {
    Empty,
    Child { child: Option<RecursiveEnum> },
}

#[derive(Intrlay)]
enum RecursiveEnvelope {
    Child { child: RecursiveEnum },
}

#[derive(Intrlay)]
#[allow(
    clippy::large_enum_variant,
    reason = "large inline arrays verify storage generation without per-element code expansion"
)]
enum LargeRecursiveEnum {
    Empty,
    Children { items: [Vec<Self>; 32] },
}

fn recursive_native_storage_preserves_wire_and_public_types(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<OptionalNode>("OptionalNode");
    registry.export::<DirectChildren>("DirectChildren");
    registry.export::<ArrayNode>("ArrayNode");
    registry.export::<NestedNode>("NestedNode");
    registry.export::<WrappedNode>("WrappedNode");
    registry.export::<InlineNode>("InlineNode");
    registry.export::<NullableInlineNode>("NullableInlineNode");
    registry.export::<NestedNullableInlineNode>("NestedNullableInlineNode");
    registry.export::<EmptyNode>("EmptyNode");
    registry.export::<MutualParent>("MutualParent");
    registry.export::<PlainNode>("PlainNode");
    registry.export::<BoxedNode>("BoxedNode");
    registry.export::<RecursiveEnum>("RecursiveEnum");
    registry.export::<RecursiveEnvelope>("RecursiveEnvelope");
    registry.export::<LargeRecursiveEnum>("LargeRecursiveEnum");
    let schema = registry.finish();
    let generated = generate_with_options(
        &schema,
        Language::CSharp,
        GenerateOptions {
            csharp: intrlay_codegen::CSharpOptions {
                namespace: Some("RecursiveStorage".into()),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let directory = std::path::PathBuf::from("csharp-recursive-storage");

    fixtures.insert(directory.join("bindings.cs"), generated)?;
    let value = OptionalNode {
        id: 1,
        children: vec![
            None,
            Some(OptionalNode {
                id: 2,
                children: vec![],
            }),
        ],
    };
    fixtures.insert(
        directory.join("optional.bin"),
        intrlay::create::<OptionalNode>(&value)?.as_ref(),
    )?;

    write_record_fixtures(fixtures, &directory)?;
    write_enum_fixtures(fixtures, &directory)?;

    // A generated backing field participates in the same collision policy as
    // ordinary members, including explicit target spelling and naming rules.
    let mut conflicting = schema;
    let definition = conflicting
        .definitions
        .values_mut()
        .find(|definition| definition.name == "OptionalNode")
        .ok_or("missing OptionalNode")?;
    let intrlay::schema::DefinitionKind::Record(fields) = &mut definition.kind else {
        unreachable!()
    };
    let mut field = intrlay::schema::Field::new(
        "number",
        intrlay::schema::Type::Scalar(intrlay::schema::Scalar::U32),
    );
    field.targets.insert(
        Language::CSharp,
        intrlay::schema::TargetOptions {
            name: Some("_childrenMemory".into()),
            ..Default::default()
        },
    );
    fields.push(field);
    fixtures.insert(
        directory.join("collision.cs"),
        generate_with_options(&conflicting, Language::CSharp, GenerateOptions::default())?,
    )?;
    Ok(())
}

fn write_record_fixtures(fixtures: &mut Fixtures, directory: &Path) -> Result<(), Box<dyn Error>> {
    let value = NullableInlineNode {
        children: [
            Some(vec![NullableInlineNode {
                children: [None, Some(vec![])],
            }]),
            None,
        ],
    };
    fixtures.insert(
        directory.join("record-nullable-inline.bin"),
        intrlay::create::<NullableInlineNode>(&value)?.as_ref(),
    )?;
    let value = NestedNullableInlineNode {
        children: [
            Some([
                vec![NestedNullableInlineNode {
                    children: [None, None],
                }],
                vec![],
            ]),
            None,
        ],
    };
    fixtures.insert(
        directory.join("record-nested-nullable-inline.bin"),
        intrlay::create::<NestedNullableInlineNode>(&value)?.as_ref(),
    )?;
    Ok(())
}

fn write_enum_fixtures(fixtures: &mut Fixtures, directory: &Path) -> Result<(), Box<dyn Error>> {
    for (name, value) in [
        (
            "enum-direct",
            RecursiveEnum::Direct {
                items: [vec![RecursiveEnum::Empty], vec![]],
            },
        ),
        (
            "enum-optional-elements",
            RecursiveEnum::OptionalElements {
                items: [vec![None, Some(RecursiveEnum::Empty)], vec![]],
            },
        ),
        (
            "enum-optional-array",
            RecursiveEnum::OptionalArray {
                items: Some([vec![RecursiveEnum::Empty], vec![]]),
            },
        ),
        (
            "enum-optional-lists",
            RecursiveEnum::OptionalLists {
                items: [None, Some(vec![RecursiveEnum::Empty])],
            },
        ),
        (
            "enum-nested-arrays",
            RecursiveEnum::NestedArrays {
                items: [[vec![RecursiveEnum::Empty], vec![]], [vec![], vec![]]],
            },
        ),
        (
            "enum-records",
            RecursiveEnum::Records {
                items: [
                    RecursiveBranch {
                        id: 7,
                        children: vec![RecursiveEnum::Empty],
                    },
                    RecursiveBranch {
                        id: 9,
                        children: vec![],
                    },
                ],
            },
        ),
        (
            "enum-mutual",
            RecursiveEnum::Mutual {
                items: [
                    vec![RecursiveParent::Child {
                        child: Some(RecursiveEnum::Empty),
                    }],
                    vec![],
                ],
            },
        ),
    ] {
        fixtures.insert(
            directory.join(format!("{name}.bin")),
            intrlay::create::<RecursiveEnum>(&value)?.as_ref(),
        )?;
    }
    fixtures.insert(
        directory.join("enum-envelope.bin"),
        intrlay::create::<RecursiveEnvelope>(&RecursiveEnvelope::Child {
            child: RecursiveEnum::Direct {
                items: [vec![RecursiveEnum::Empty], vec![]],
            },
        })?
        .as_ref(),
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    recursive_native_storage_preserves_wire_and_public_types(fixtures)?;
    Ok(())
}

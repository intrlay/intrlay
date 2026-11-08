//! Native fixed arrays and scoped C# views share the resolved wire representation.

use super::Fixtures;

use std::error::Error;

use intrlay::{Intrlay, schema::Registry};
use intrlay_codegen::{GenerateOptions, generate_with_options};
use intrlay_schema::Language;

#[derive(Clone, Intrlay)]
#[repr(C)]
struct Vertex {
    id: u32,
    position: [f32; 3],
    color: [f32; 4],
}

#[derive(Intrlay)]
struct Padded {
    flag: u8,
    value: u64,
}

#[derive(Intrlay)]
struct FlatPacket {
    sequence: u64,
    vertices: Vec<Vertex>,
    samples: Vec<f32>,
    label: String,
    color: [u32; 3],
    enabled: bool,
}

#[derive(Intrlay)]
struct LargeInlinePacket {
    left: [u32; 32_768],
    right: [u32; 32_768],
    label: String,
}

// The canonical input name intentionally matches the generic Create parameter.
#[derive(Intrlay)]
struct T {
    values: Vec<u32>,
}

#[derive(Intrlay)]
enum Nested {
    Number(u64),
    Text(String),
}

#[derive(Intrlay)]
enum ProtocolNames {
    Value(u32),
    HasValue(u32),
    TryGetValue(u32),
    Create(u32),
}

#[derive(Intrlay)]
enum SparseUnion {
    #[intrlay(tag = 7)]
    Empty,
    #[intrlay(tag = 91)]
    Value(u32),
}

#[derive(Intrlay)]
enum ProviderCollision {
    #[intrlay(csharp(name = "IUnionMembers"))]
    Member(u32),
}

#[derive(Intrlay)]
enum Payload {
    Data {
        numbers: [u32; 3],
        texts: [String; 2],
        vertices: [Vertex; 2],
    },
    Again {
        numbers: [u32; 3],
        texts: [String; 2],
    },
    Nested {
        items: [Nested; 2],
    },
}

#[derive(Intrlay)]
struct Scene {
    vertices: Vec<Vertex>,
    matrix: [[u32; 2]; 2],
    absent: Option<Vertex>,
    present: Option<Vertex>,
    #[allow(
        clippy::option_option,
        reason = "fixture distinguishes None from Some(None)"
    )]
    nested: Option<Option<Vertex>>,
    words: [String; 2],
    batches: Vec<[u32; 3]>,
    empty: [u32; 0],
    units: [(); 2],
    padded: Padded,
    flag: bool,
    variants: Vec<Payload>,
}

fn generate_native_and_scoped_view_cases(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let vertex = Vertex {
        id: 7,
        position: [1.0, 2.0, 3.0],
        color: [0.0, 0.0, 0.0, 1.0],
    };
    let value = Scene {
        vertices: vec![vertex.clone(), vertex.clone()],
        matrix: [[1, 2], [3, 4]],
        absent: None,
        present: Some(vertex.clone()),
        nested: Some(None),
        words: ["first".into(), "second".into()],
        batches: vec![[5, 6, 7]],
        empty: [],
        units: [(); 2],
        padded: Padded {
            flag: 9,
            value: 123,
        },
        flag: true,
        variants: vec![
            Payload::Data {
                numbers: [10, 20, 30],
                texts: ["a".into(), "b".into()],
                vertices: [vertex.clone(), vertex],
            },
            Payload::Again {
                numbers: [40, 50, 60],
                texts: ["c".into(), "d".into()],
            },
            Payload::Nested {
                items: [Nested::Number(456), Nested::Text("nested".into())],
            },
        ],
    };
    let mut registry = Registry::new();
    registry.export::<Scene>("Scene");
    registry.export::<FlatPacket>("FlatPacket");
    registry.export::<LargeInlinePacket>("LargeInlinePacket");
    registry.export::<T>("T");
    registry.export::<ProtocolNames>("ProtocolNames");
    registry.export::<SparseUnion>("SparseUnion");
    let schema = registry.finish();
    fixtures.insert(
        std::path::Path::new("native-views-csharp15").join("bindings.cs"),
        generate_with_options(
            &schema,
            Language::CSharp,
            GenerateOptions {
                csharp: intrlay_codegen::CSharpOptions {
                    namespace: Some("NativeViews.Generated".into()),
                    language_version: Some(15),
                },
                ..Default::default()
            },
        )?,
    )?;
    let code = generate_with_options(
        &schema,
        Language::CSharp,
        GenerateOptions {
            csharp: intrlay_codegen::CSharpOptions {
                namespace: Some("NativeViews.Generated".into()),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let directory = std::path::PathBuf::from("native-views");

    fixtures.insert(directory.join("bindings.cs"), code)?;
    fixtures.insert(
        directory.join("rust.bin"),
        SceneView::create(&value)?.as_bytes(),
    )?;
    let packet = FlatPacket {
        sequence: 17,
        vertices: value.vertices,
        samples: vec![1.5, -2.0, 3.25],
        label: "borrowed 🦀".into(),
        color: [11, 13, 17],
        enabled: true,
    };
    fixtures.insert(
        directory.join("flat.bin"),
        FlatPacketView::create(&packet)?.as_bytes(),
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    check_union_language_options()?;
    generate_native_and_scoped_view_cases(fixtures)?;
    Ok(())
}

fn check_union_language_options() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<ProviderCollision>("ProviderCollision");
    let schema = registry.finish();
    let default = generate_with_options(&schema, Language::CSharp, GenerateOptions::default())?;
    let explicit = generate_with_options(
        &schema,
        Language::CSharp,
        GenerateOptions {
            csharp: intrlay_codegen::CSharpOptions {
                language_version: Some(14),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    assert_eq!(default, explicit);
    for (version, expected) in [
        (13, "C# 14 or later"),
        (15, "IUnionMembers"),
        (16, "IUnionMembers"),
    ] {
        let result = generate_with_options(
            &schema,
            Language::CSharp,
            GenerateOptions {
                csharp: intrlay_codegen::CSharpOptions {
                    language_version: Some(version),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let error = result.err().ok_or("expected a C# language diagnostic")?;
        assert!(error.to_string().contains(expected), "{error}");
    }
    Ok(())
}

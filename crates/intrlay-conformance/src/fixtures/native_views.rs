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
enum Nested {
    Number(u64),
    Text(String),
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
    let code = generate_with_options(
        &registry.finish(),
        Language::CSharp,
        GenerateOptions {
            csharp_namespace: Some("NativeViews.Generated".into()),
            ..Default::default()
        },
    )?;
    let directory = std::path::PathBuf::from("native-views");

    fixtures.insert(directory.join("bindings.cs"), code)?;
    fixtures.insert(
        directory.join("rust.bin"),
        SceneView::create(&value)?.as_bytes(),
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_native_and_scoped_view_cases(fixtures)?;
    Ok(())
}

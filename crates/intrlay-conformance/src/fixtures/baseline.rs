//! Writes public synthetic fixtures for executable cross-language conformance.

use std::error::Error;

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};

#[repr(C, align(16))]
#[derive(Intrlay, Debug, PartialEq)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Intrlay, Debug, PartialEq)]
enum DrawingCommand {
    Clear,
    DrawPoint {
        position: Point,
        size: f64,
        label: String,
    },
}

#[derive(Intrlay, Debug, PartialEq)]
struct Link {
    id: u32,
    next: Option<Box<Link>>,
}

#[derive(Intrlay, Debug, PartialEq)]
struct Message {
    sequence: u64,
    title: String,
    #[intrlay(project = "f32")]
    points: Vec<Point>,
    commands: Vec<DrawingCommand>,
    tree: Link,
    optional: Option<u32>,
    fixed: [f32; 3],
}

#[derive(Intrlay)]
enum NullableVariant {
    Empty,
    Value { value: Option<u32> },
}

#[derive(Intrlay)]
#[expect(
    clippy::option_option,
    reason = "Conformance must distinguish every nested Option presence state"
)]
struct NullableCases {
    absent: Option<u32>,
    zero: Option<u32>,
    text: Option<String>,
    missing_text: Option<String>,
    absent_unit: Option<()>,
    present_unit: Option<()>,
    nested_absent: Option<Option<u32>>,
    nested_none: Option<Option<u32>>,
    nested_value: Option<Option<u32>>,
    boxed_none: Option<Box<Option<u32>>>,
    triple: Option<Option<Option<u32>>>,
    variants: Vec<NullableVariant>,
}

fn nullable_fixture() -> NullableCases {
    NullableCases {
        absent: None,
        zero: Some(0),
        text: Some(String::new()),
        missing_text: None,
        absent_unit: None,
        present_unit: Some(()),
        nested_absent: None,
        nested_none: Some(None),
        nested_value: Some(Some(7)),
        boxed_none: Some(Box::new(None)),
        triple: Some(Some(None)),
        variants: vec![
            NullableVariant::Empty,
            NullableVariant::Value { value: None },
            NullableVariant::Value { value: Some(9) },
        ],
    }
}

fn fixture() -> Message {
    Message {
        sequence: 0x1234_5678_9abc_def0,
        title: "fixture 🦀 한글".into(),
        points: vec![
            Point {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            Point {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        ],
        commands: vec![
            DrawingCommand::Clear,
            DrawingCommand::DrawPoint {
                position: Point {
                    x: 7.0,
                    y: 8.0,
                    z: 9.0,
                },
                size: 0.75,
                label: "example".into(),
            },
        ],
        tree: Link {
            id: 7,
            next: Some(Box::new(Link { id: 8, next: None })),
        },
        optional: Some(0),
        fixed: [10.0, 20.0, 30.0],
    }
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::Path::new("conformance");
    let mut registry = Registry::new();
    registry.export::<Message>("Message");
    registry.export::<NullableCases>("NullableCases");
    let schema = registry.finish();
    fixtures.insert(directory.join("schema.json"), schema.to_json()?)?;
    for (language, filename) in [
        (Language::TypeScript, "bindings.ts"),
        (Language::Python, "bindings.py"),
        (Language::Rust, "bindings.rs"),
        (Language::CSharp, "bindings.cs"),
        (Language::Dart, "bindings.dart"),
        (Language::Cpp, "bindings.hpp"),
    ] {
        fixtures.insert(
            directory.join(filename),
            intrlay_codegen::generate_with_options(
                &schema,
                language,
                intrlay_codegen::GenerateOptions {
                    module_name: Some("Conformance".into()),
                    ..Default::default()
                },
            )?,
        )?;
    }
    let view = MessageView::create(&fixture())?;
    fixtures.insert(directory.join("rust.bin"), view.as_bytes())?;
    fixtures.insert(
        directory.join("nullable.bin"),
        NullableCasesView::create(&nullable_fixture())?.as_bytes(),
    )?;
    Ok(())
}

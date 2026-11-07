//! Generated public APIs should contain only schema types and reachable views.

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

#[derive(Intrlay)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Intrlay)]
struct Frame {
    scalar: f32,
    optional: Option<Box<Point>>,
    points: Vec<Point>,
    #[intrlay(generate_getters = false, generate_setters = false)]
    hidden: Vec<String>,
}

#[derive(Intrlay)]
struct AccessModes {
    #[intrlay(generate_setters = false)]
    read_only: u32,
    #[intrlay(generate_getters = false)]
    write_only: u32,
}

#[derive(Intrlay)]
struct NumericValues {
    enabled: bool,
    signed8: i8,
    unsigned8: u8,
    signed16: i16,
    unsigned16: u16,
    signed32: i32,
    unsigned32: u32,
    signed64: i64,
    unsigned64: u64,
    real32: f32,
    real64: f64,
    label: String,
}

const VIEW_LANGUAGES: [Language; 5] = [
    Language::TypeScript,
    Language::Python,
    Language::CSharp,
    Language::Dart,
    Language::Cpp,
];

#[test]
fn internal_layout_nodes_do_not_become_public_views() -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<Frame>("Frame");
    let schema = registry.finish();
    for language in VIEW_LANGUAGES {
        let source = generate(&schema, language)?;
        let prefix = if language == Language::CSharp {
            ""
        } else {
            "Intrlay"
        };
        assert!(source.contains("FrameView"));
        assert!(source.contains("PointView"));
        assert!(source.contains(&format!("{prefix}SequenceOfPointView")));
        assert!(!source.contains(&format!("{prefix}F32View")));
        assert!(!source.contains(&format!("{prefix}OptionalBoxedPointView")));
        assert!(!has_identifier(&source, &format!("{prefix}BoxedPointView")));
        assert_eq!(
            source.contains(&format!("{prefix}SequenceOfStringView")),
            !language.supports_accessor_generation()
        );
        if language == Language::TypeScript {
            assert!(!source.contains("export type IntrlayF32"));
            assert!(source.contains("scalar: number"));
        }
    }
    Ok(())
}

#[test]
fn explicit_scalar_and_optional_exports_keep_their_entry_points()
-> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<f32>("Scalar");
    registry.export::<Option<Box<Point>>>("OptionalPoint");
    let schema = registry.finish();
    for language in VIEW_LANGUAGES {
        let source = generate(&schema, language)?;
        let prefix = if language == Language::CSharp {
            ""
        } else {
            "Intrlay"
        };
        assert!(source.contains(&format!("{prefix}F32View")));
        assert!(source.contains(&format!("{prefix}OptionalBoxedPointView")));
        assert!(source.contains("PointView"));
        assert!(!has_identifier(&source, &format!("{prefix}BoxedPointView")));
    }
    Ok(())
}

fn python_access_modes_and_explicit_exports_are_typed(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<AccessModes>("AccessModes");
    registry.export::<f32>("Scalar");
    registry.export::<Vec<Option<Box<Point>>>>("OptionalPoints");
    registry.export::<NumericValues>("NumericValues");
    let source = generate(&registry.finish(), Language::Python)?;
    let directory = std::path::PathBuf::from("python-types");

    fixtures.insert(directory.join("edge_bindings.py"), source)?;
    let numeric = NumericValues {
        enabled: true,
        signed8: i8::MIN,
        unsigned8: u8::MAX,
        signed16: i16::MIN,
        unsigned16: u16::MAX,
        signed32: i32::MIN,
        unsigned32: u32::MAX,
        signed64: i64::MIN,
        unsigned64: u64::MAX,
        real32: -1.25,
        real64: 12345.125,
        label: "x".into(),
    };
    fixtures.insert(
        directory.join("numeric.bin"),
        NumericValuesView::create(&numeric)?.as_bytes(),
    )?;
    Ok(())
}

fn has_identifier(source: &str, name: &str) -> bool {
    source
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|token| token == name)
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    python_access_modes_and_explicit_exports_are_typed(fixtures)?;
    Ok(())
}

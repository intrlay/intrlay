//! Executable fixtures for public entry points, access modes, and module coexistence.

use super::Fixtures;

#[cfg(test)]
use intrlay_codegen::generate;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

#[derive(Intrlay)]
#[repr(align(16))]
enum Aligned {
    Value { payload: u32 },
}

#[derive(Intrlay)]
enum Restricted {
    Empty,
    Value {
        visible: u32,
        #[intrlay(generate_getters = false)]
        hidden: u32,
    },
}

#[derive(Intrlay)]
#[intrlay(generate_setters = false, create = false)]
struct ReadOnly {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct BorrowOnly {
    text: String,
}

#[derive(Intrlay)]
struct Alpha {
    values: Vec<u32>,
}

#[derive(Intrlay)]
struct Beta {
    values: Vec<u32>,
}

#[derive(Intrlay)]
#[repr(C, align(16))]
struct Vector {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Intrlay)]
struct Projections {
    #[intrlay(project = "f32")]
    vectors: Vec<Vector>,
}

#[derive(Intrlay)]
struct FixedArrays {
    three: [f32; 3],
    four: [f32; 4],
    matrix: [[f32; 3]; 2],
    present: Option<[f32; 3]>,
    absent: Option<[f32; 3]>,
    #[intrlay(generate_setters = false)]
    read_only: [f32; 3],
}

#[derive(Intrlay)]
struct StructuralArray {
    #[intrlay(project = "fields")]
    values: [f32; 3],
}

#[derive(Intrlay)]
struct StructuralNumericProjection {
    values: Vec<f32>,
}

#[derive(Intrlay)]
enum NonZeroTag {
    #[intrlay(tag = 1)]
    First,
    Second,
}

#[derive(Intrlay)]
enum NonZeroPayloadTag {
    #[intrlay(tag = 1)]
    Text(String),
    Empty,
}

#[derive(Intrlay)]
struct TaggedItems {
    items: Vec<NonZeroTag>,
}

fn emit(fixtures: &mut Fixtures, registry: Registry, name: &str) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("regressions");

    let schema = registry.finish();
    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::Cpp, "hpp"),
        (Language::CSharp, "cs"),
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("{name}.{extension}")),
            generate_with_options(
                &schema,
                language,
                GenerateOptions {
                    module_name: Some(if name == "bindings" {
                        "Regression".into()
                    } else {
                        name.replace('-', "_")
                    }),
                    ..Default::default()
                },
            )?,
        )?;
    }
    Ok(())
}

fn write_cross_language_regressions(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Aligned>("Aligned");
    registry.export::<Restricted>("Restricted");
    registry.export::<ReadOnly>("ReadOnly");
    registry.export::<f32>("Scalar");
    registry.export::<Projections>("Projections");
    emit(fixtures, registry, "bindings")?;

    let mut registry = Registry::new();
    registry.export::<Alpha>("Alpha");
    emit(fixtures, registry, "alpha")?;
    let mut registry = Registry::new();
    registry.export::<Beta>("Beta");
    emit(fixtures, registry, "beta")?;

    let mut registry = Registry::new();
    registry.export::<FixedArrays>("FixedArrays");
    emit(fixtures, registry, "arrays")?;

    let mut registry = Registry::new();
    registry.export::<NonZeroTag>("NonZeroTag");
    registry.export::<NonZeroPayloadTag>("NonZeroPayloadTag");
    registry.export::<TaggedItems>("TaggedItems");
    emit(fixtures, registry, "tagged-defaults")?;

    let mut registry = Registry::new();
    registry.export::<Vec<String>>("ExtentStrings");
    emit(fixtures, registry, "construction-extents")?;
    Ok(())
}

#[test]
fn numeric_arrays_keep_their_layout_without_length_specific_views() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<FixedArrays>("FixedArrays");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        assert!(!source.contains("IntrlayArray3OfF32View"));
        assert!(!source.contains("IntrlayArray4OfF32View"));
    }
    let cpp = generate(&schema, Language::Cpp)?;
    assert!(cpp.contains("std::array<float, 3>"));
    assert!(cpp.contains("::std::array<::std::array<float, 3>, 2>"));
    assert!(!cpp.contains("NumericArray<"));
    let csharp = generate(&schema, Language::CSharp)?;
    assert!(!csharp.contains("NumericArray<"));
    assert!(csharp.contains("\"Array3OfF32View\""));
    let rust = generate(&schema, Language::Rust)?;
    assert!(rust.contains("[::core::primitive::f32; 3]"));
    assert!(rust.contains("[[::core::primitive::f32; 3]; 2]"));

    let mut registry = Registry::new();
    registry.export::<StructuralArray>("StructuralArray");
    let schema = registry.finish();
    for language in [
        Language::TypeScript,
        Language::Python,
        Language::Dart,
        Language::CSharp,
        Language::Cpp,
    ] {
        let prefix = if language == Language::CSharp {
            ""
        } else {
            "Intrlay"
        };
        assert!(generate(&schema, language)?.contains(&format!("{prefix}Array3OfF32View")));
    }
    Ok(())
}

#[test]
fn structural_backends_reject_numeric_flattening() {
    let mut registry = Registry::new();
    registry.export::<StructuralNumericProjection>("StructuralNumericProjection");
    let mut schema = registry.finish();
    for language in [Language::Rust, Language::Cpp, Language::CSharp] {
        for definition in schema.definitions.values_mut() {
            definition.targets.insert(
                language,
                intrlay::schema::TargetOptions {
                    exposure: Some(intrlay::schema::Exposure::TypedArray(
                        intrlay::schema::Scalar::F32,
                    )),
                    ..Default::default()
                },
            );
        }
        assert!(matches!(
            generate(&schema, language),
            Err(error) if error.to_string().contains("does not support numeric projections")
        ));
    }
}

#[test]
fn nonconstructible_graphs_do_not_emit_unused_encoders() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<BorrowOnly>("BorrowOnly");
    let schema = registry.finish();
    for language in [
        Language::TypeScript,
        Language::Python,
        Language::Dart,
        Language::Cpp,
        Language::CSharp,
    ] {
        let source = generate(&schema, language)?;
        assert!(
            !source.contains("writeBorrowOnly")
                && !source.contains("write_borrow_only")
                && !source.contains("WriteBorrowOnly")
        );
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    write_cross_language_regressions(fixtures)?;
    Ok(())
}

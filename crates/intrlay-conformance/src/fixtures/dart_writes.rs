//! Native and JavaScript Dart encoders agree with Rust on mixed layouts.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

#[derive(Intrlay)]
struct Numbers {
    signed_byte: i8,
    unsigned_byte: u8,
    signed_short: i16,
    unsigned_short: u16,
    signed_word: i32,
    unsigned_word: u32,
    signed_long: i64,
    unsigned_long: u64,
    single: f32,
    double: f64,
}

#[derive(Intrlay)]
enum Choice {
    Numbers(Numbers),
    Empty,
}

#[derive(Intrlay)]
#[intrlay(transparent)]
struct Identifier(u32);

#[derive(Intrlay)]
enum TaggedScalar {
    Empty,
    Value {
        number: u32,
        identifier: Identifier,
        flag: bool,
    },
}

#[derive(Intrlay)]
struct PrivateFields {
    #[intrlay(dart(name = "_payload"))]
    payload: u32,
    #[intrlay(dart(name = "__label"))]
    label: String,
    visible: u32,
}

#[derive(Intrlay)]
enum PrivateChoice {
    Value {
        #[intrlay(dart(name = "_payload"))]
        payload: u32,
    },
}

fn private_fields_use_compatible_public_constructor_parameters(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<PrivateFields>("PrivateFields");
    registry.export::<PrivateChoice>("PrivateChoice");
    let source = generate(&registry.finish(), Language::Dart)?;
    assert!(source.contains("required core.int payload,"));
    assert!(!source.contains("required this._payload"));
    let directory = std::path::PathBuf::from("dart-writes");

    fixtures.insert(
        directory.join("private-fields.dart"),
        format!("// @dart=3.3\n{source}"),
    )?;
    Ok(())
}

fn tagged_scalar_setters_support_the_minimum_dart_language(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<TaggedScalar>("TaggedScalar");
    let source = generate(&registry.finish(), Language::Dart)?;
    assert!(source.contains("void setValueNumber(core.int value)"));
    assert!(!source.contains("set valueNumber("));
    let directory = std::path::PathBuf::from("dart-writes");

    fixtures.insert(
        directory.join("tagged-scalars.dart"),
        format!("// @dart=3.3\n{source}"),
    )?;
    Ok(())
}

#[derive(Default, Intrlay)]
struct NumericArrays {
    i8_values: Vec<i8>,
    u8_values: Vec<u8>,
    i16_values: Vec<i16>,
    u16_values: Vec<u16>,
    i32_values: Vec<i32>,
    u32_values: Vec<u32>,
    i64_values: Vec<i64>,
    u64_values: Vec<u64>,
    f32_values: Vec<f32>,
    f64_values: Vec<f64>,
    flags: Vec<bool>,
    nested: Vec<Vec<f32>>,
}

#[derive(Intrlay)]
struct Frame {
    enabled: bool,
    choice: Choice,
    optional: Option<Box<[f64; 2]>>,
    label: String,
    values: Vec<f32>,
    arrays: NumericArrays,
}

#[test]
fn numeric_array_writers_do_not_emit_unused_scalar_helpers() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Vec<[f32; 3]>>("Points");
    let source = generate(&registry.finish(), Language::Dart)?;
    assert!(source.contains("intrlay.writeFloat32Array(storage, payloadOffset, value)"));
    assert!(!source.contains("void _writeIntrlayF32("));

    let mut registry = Registry::new();
    registry.export::<Vec<[f32; 3]>>("Points");
    registry.export::<f32>("Scalar");
    registry.export::<Option<Box<u32>>>("OptionalWord");
    let source = generate(&registry.finish(), Language::Dart)?;
    assert!(source.contains("void _writeIntrlayF32("));
    assert!(source.contains("void _writeIntrlayU32("));
    Ok(())
}

fn emit_mixed_writer_cases(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("dart-writes");

    let mut registry = Registry::new();
    registry.export::<Frame>("Frame");
    fixtures.insert(
        directory.join("bindings.dart"),
        generate(&registry.finish(), Language::Dart)?,
    )?;
    let frames = [
        Frame {
            enabled: true,
            choice: Choice::Numbers(Numbers {
                signed_byte: -127,
                unsigned_byte: 255,
                signed_short: -32768,
                unsigned_short: 65535,
                signed_word: i32::MIN,
                unsigned_word: u32::MAX,
                signed_long: -4_294_967_297,
                unsigned_long: 4_294_967_297,
                single: -0.0,
                double: 1.0 / 3.0,
            }),
            optional: Some(Box::new([1.25, -3.5])),
            label: "한글 🦀".into(),
            values: vec![1.0 / 3.0, -2.5, 7.25],
            arrays: NumericArrays {
                i8_values: vec![i8::MIN, -1, i8::MAX],
                u8_values: vec![0, 127, u8::MAX],
                i16_values: vec![i16::MIN, -1, i16::MAX],
                u16_values: vec![0, 32768, u16::MAX],
                i32_values: vec![i32::MIN, -1, i32::MAX],
                u32_values: vec![0, 2_147_483_648, u32::MAX],
                i64_values: vec![-4_294_967_297, -1, 4_294_967_297],
                u64_values: vec![0, 4_294_967_295, 4_294_967_297],
                f32_values: vec![-0.0, 1.0 / 3.0, f32::INFINITY],
                f64_values: vec![-0.0, 1.0 / 3.0, f64::NEG_INFINITY],
                flags: vec![false, true, false],
                nested: vec![vec![1.25], vec![], vec![-2.5, 3.75]],
            },
        },
        Frame {
            enabled: false,
            choice: Choice::Empty,
            optional: None,
            label: String::new(),
            values: vec![],
            arrays: NumericArrays::default(),
        },
    ];
    let bytes = frames
        .iter()
        .map(|frame| Ok(FrameView::create(frame)?.as_bytes().to_vec()))
        .collect::<Result<Vec<_>, intrlay::EncodeError>>()?;
    fixtures.insert(
        directory.join("expected.dart"),
        format!(
            "// Generated Rust reference bytes for native and JavaScript Dart tests.\nconst expectedFrames = {};\n",
            serde_json::to_string(&bytes)?,
        ),
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    private_fields_use_compatible_public_constructor_parameters(fixtures)?;
    tagged_scalar_setters_support_the_minimum_dart_language(fixtures)?;
    emit_mixed_writer_cases(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn private_fields_use_compatible_public_constructor_parameters()
    -> Result<(), Box<dyn std::error::Error>> {
        super::private_fields_use_compatible_public_constructor_parameters(
            &mut super::Fixtures::default(),
        )
    }
    #[test]
    fn tagged_scalar_setters_support_the_minimum_dart_language()
    -> Result<(), Box<dyn std::error::Error>> {
        super::tagged_scalar_setters_support_the_minimum_dart_language(
            &mut super::Fixtures::default(),
        )
    }
}

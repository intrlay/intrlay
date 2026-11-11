//! Real compiler coverage for the C++20 source frontend.

mod common;

use std::path::PathBuf;

use intrlay_frontend_cpp::{Options, extract};
use intrlay_schema::{DefinitionKind, Exposure, Language, Layouts, Scalar, Type};

#[derive(intrlay::Intrlay)]
enum Ordinary {
    Value { value: u32 },
}

#[test]
fn custom_character_traits_preserve_the_string_schema() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let options = Options {
        arguments: vec![format!(
            "-I{}",
            directory.join("../../packages/cpp/include").display()
        )],
        ..Options::default()
    };
    let schema = extract(directory.join("tests/fixtures/codec_cases.hpp"), &options)?;
    let DefinitionKind::Record(fields) = &common::definition(&schema, "Text").kind else {
        return Err("text record expected".into());
    };
    assert_eq!(fields[0].ty, Type::String);
    Ok(())
}

#[test]
fn forward_declarations_preserve_complete_definitions() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut options = Options {
        arguments: vec![format!(
            "-I{}",
            directory.join("../../packages/cpp/include").display()
        )],
        ..Options::default()
    };
    let path = directory.join("tests/fixtures/forward.hpp");
    let forward = extract(&path, &options)?;
    options.arguments.push("-DINTRLAY_DEFINE_FIRST".into());
    let defined_first = extract(&path, &options)?;
    assert_eq!(forward, defined_first);
    let child = common::definition(&forward, "Child");
    assert!(child.transparent);
    assert_eq!(child.alignment, 16);
    assert_eq!(child.docs, "An explicitly aligned child.");
    let mode = common::definition(&forward, "OperatingMode");
    assert_eq!(mode.name, "OperatingMode");
    let DefinitionKind::Enum(variants) = &mode.kind else {
        return Err("mode enum expected".into());
    };
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].tag, 7);
    assert_eq!(variants[1].tag, 9);
    assert_eq!(variants[0].docs, "Device is enabled.");
    assert_eq!(
        variants[0].targets[&Language::Cpp].name.as_deref(),
        Some("Enabled")
    );
    Ok(())
}

#[test]
fn forward_declarations_cannot_hide_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut options = Options {
        arguments: vec![
            format!(
                "-I{}",
                directory.join("../../packages/cpp/include").display()
            ),
            "-DINTRLAY_INHERITANCE".into(),
        ],
        ..Options::default()
    };
    for defined_first in [false, true] {
        if defined_first {
            options.arguments.push("-DINTRLAY_DEFINE_FIRST".into());
        }
        let Err(error) = extract(directory.join("tests/fixtures/forward.hpp"), &options) else {
            return Err("inherited fields must not be silently discarded".into());
        };
        assert!(
            error
                .to_string()
                .contains("inheritance requires an explicit mirror")
        );
    }
    Ok(())
}

#[test]
fn untagged_unions_require_an_explicit_representation() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for mode in [
        "NESTED_UNION",
        "INTRLAY_DIRECT_UNION",
        "INTRLAY_UNION_ALIAS",
    ] {
        let options = Options {
            arguments: vec![
                format!(
                    "-I{}",
                    directory.join("../../packages/cpp/include").display()
                ),
                format!("-D{mode}"),
            ],
            ..Options::default()
        };
        let Err(error) = extract(directory.join("tests/fixtures/unions.hpp"), &options) else {
            return Err("untagged union must not be extracted as a record".into());
        };
        assert!(
            error.to_string().contains("untagged union"),
            "{mode}: {error}"
        );
    }
    Ok(())
}

#[test]
fn concrete_cpp_types_use_the_shared_layout() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let options = Options {
        arguments: vec![format!(
            "-I{}",
            directory.join("../../packages/cpp/include").display()
        )],
        ..Options::default()
    };
    let schema = extract(directory.join("tests/fixtures/schema.hpp"), &options)?;
    assert_eq!(schema.exports.len(), 4);
    let point = common::definition(&schema, "Point");
    assert_eq!(
        point.docs,
        "A point in world space.\n\n    Preserve example indentation."
    );
    let DefinitionKind::Record(point_fields) = &point.kind else {
        return Err("point record expected".into());
    };
    assert_eq!(point_fields[0].docs, "Position along X.");
    assert_eq!(
        common::definition(&schema, "Frame").docs,
        "Drawing frame.\n\n    Keep this indentation too."
    );
    let DefinitionKind::Enum(variants) = &common::definition(&schema, "Mode").kind else {
        return Err("mode enum expected".into());
    };
    assert_eq!(variants[1].docs, "Device is enabled.");
    assert_eq!(point.alignment, 16);
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        assert_eq!(
            point.targets[&language].exposure,
            Some(Exposure::TypedArray(Scalar::F32))
        );
    }
    for language in [Language::Rust, Language::Cpp, Language::CSharp] {
        assert!(!point.targets.contains_key(&language));
    }
    assert_eq!(
        Layouts::new(&schema)
            .of(&common::named(&schema, "Point"))?
            .size(),
        16
    );
    let Type::Named(identity) = &schema.exports["Concrete"] else {
        panic!("concrete record expected");
    };
    let DefinitionKind::Record(fields) = &schema.definitions[identity].kind else {
        panic!("record expected");
    };
    assert_eq!(fields[0].ty, Type::Scalar(Scalar::U64));
    assert_eq!(
        fields[1].ty,
        Type::Optional(Box::new(Type::Scalar(Scalar::U64)))
    );
    let _ = common::definition(&schema, "Node");
    Ok(())
}

#[test]
fn annotations_preserve_source_identity() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let options = Options {
        arguments: vec![format!(
            "-I{}",
            directory.join("../../packages/cpp/include").display()
        )],
        ..Options::default()
    };
    let schema = extract(directory.join("tests/fixtures/annotations.hpp"), &options)?;
    let id = common::definition(&schema, "CppId");
    assert!(id.transparent);
    assert_eq!(id.alignment, 16);
    let DefinitionKind::Record(fields) = &common::definition(&schema, "RenamedField").kind else {
        panic!("record expected")
    };
    assert_eq!(fields[0].name, "native_value");
    assert_eq!(
        fields[0].targets[&Language::Cpp].name.as_deref(),
        Some("exported_value")
    );
    assert_eq!(
        fields[0].targets[&Language::TypeScript].name.as_deref(),
        Some("literalValue")
    );
    let DefinitionKind::Enum(variants) = &common::definition(&schema, "CppChoice").kind else {
        panic!("enum expected")
    };
    assert_eq!(variants[0].name, "Original");
    assert_eq!(
        variants[0].targets[&Language::TypeScript].name.as_deref(),
        Some("Specific")
    );
    Ok(())
}

#[test]
fn top_level_qualifiers_share_the_underlying_named_type() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let options = Options {
        arguments: vec![format!(
            "-I{}",
            directory.join("../../packages/cpp/include").display()
        )],
        ..Options::default()
    };
    let schema = extract(directory.join("tests/fixtures/qualifiers.hpp"), &options)?;
    assert_eq!(schema.definitions.len(), 7);
    let DefinitionKind::Record(fields) = &common::definition(&schema, "Qualified").kind else {
        panic!("record expected");
    };
    let child = common::named(&schema, "Child");
    assert!(fields[..4].iter().all(|field| field.ty == child));
    let mode = common::named(&schema, "Mode");
    assert!(fields[4..8].iter().all(|field| field.ty == mode));
    assert_eq!(fields[8].ty, fields[9].ty);
    assert_eq!(fields[10].ty, Type::Boxed(Box::new(child.clone())));
    assert_eq!(
        fields[11].ty,
        Type::Array {
            element: Box::new(child),
            length: 2,
        }
    );
    assert_eq!(fields[12].ty, fields[13].ty);
    assert_eq!(fields[12].ty, schema.exports["PlainHolder"]);
    assert_eq!(fields[14].ty, schema.exports["ConstHolder"]);
    assert_ne!(schema.exports["PlainHolder"], schema.exports["ConstHolder"]);
    let DefinitionKind::Record(fields) = &common::definition(&schema, "Node").kind else {
        panic!("recursive record expected");
    };
    assert_eq!(
        fields[0].ty,
        Type::Boxed(Box::new(common::named(&schema, "Node")))
    );
    Ok(())
}

#[test]
fn only_native_variant_alternatives_have_synthetic_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let options = Options {
        arguments: vec![format!(
            "-I{}",
            directory.join("../../packages/cpp/include").display()
        )],
        ..Options::default()
    };
    let schema = extract(directory.join("tests/fixtures/qualifiers.hpp"), &options)?;
    let DefinitionKind::Enum(variants) = &common::definition(&schema, "Mode").kind else {
        panic!("enum expected");
    };
    assert!(variants.iter().all(|variant| !variant.synthetic_payload));
    let DefinitionKind::Record(fields) = &common::definition(&schema, "Qualified").kind else {
        panic!("record expected");
    };
    let Type::Named(choice) = &fields[8].ty else {
        panic!("named variant expected");
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[choice].kind else {
        panic!("variant expected");
    };
    assert_eq!(variants.len(), 2);
    assert!(
        variants
            .iter()
            .all(|variant| variant.synthetic_payload && variant.fields.len() == 1)
    );

    let mut registry = intrlay_schema::Registry::new();
    registry.export::<Ordinary>("Ordinary");
    let schema = registry.finish();
    let Type::Named(identity) = &schema.exports["Ordinary"] else {
        panic!("named enum expected");
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[identity].kind else {
        panic!("ordinary enum expected");
    };
    assert!(!variants[0].synthetic_payload);
    assert_eq!(variants[0].fields[0].name, "value");
    Ok(())
}

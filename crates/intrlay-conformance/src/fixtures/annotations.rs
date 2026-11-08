//! Cross-target annotation contracts, including declaration-local accessor emission.
#![allow(clippy::unwrap_used, reason = "test fixture shape is fixed")]

use super::Fixtures;

#[cfg(test)]
use intrlay::schema::{DefinitionKind, Exposure, Scalar, TargetOptions};
#[cfg(test)]
use intrlay_codegen::generate;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};
use std::error::Error;

#[derive(Intrlay)]
struct ContractChild {
    value: u32,
}
#[derive(Intrlay)]
#[intrlay(generate_setters = false)]
struct ConstantChild {
    value: u32,
}
#[derive(Intrlay)]
#[intrlay(generate_setters = false)]
struct ReadIndexed {
    read_score: u32,
    read_vector: [f32; 3],
}
#[derive(Intrlay)]
#[intrlay(generate_getters = false)]
struct WriteIndexed {
    write_score: u32,
    write_vector: [f32; 3],
}
#[derive(Intrlay)]
#[intrlay(generate_getters = false, generate_setters = false)]
struct HiddenIndexed {
    hidden_score: u32,
    hidden_vector: [f32; 3],
}
#[derive(Intrlay)]
struct IndexedHints {
    readable: Vec<ReadIndexed>,
    writable: Vec<WriteIndexed>,
    hidden: [HiddenIndexed; 1],
}
#[derive(Intrlay)]
#[intrlay(transparent)]
#[repr(C, align(16))]
struct Identifier(u32);
#[derive(Intrlay)]
#[intrlay(transparent)]
struct MaybeValue(Option<u32>);
#[derive(Intrlay)]
#[intrlay(transparent)]
struct Label(String);

#[derive(Intrlay)]
struct NumericLeaf {
    #[intrlay(generate_setters = false)]
    value: f32,
}
#[derive(Intrlay)]
struct NumericBranch {
    leaf: NumericLeaf,
}
#[derive(Intrlay)]
#[intrlay(project = "f32")]
struct NumericAccess {
    branch: NumericBranch,
}
#[derive(Intrlay)]
struct ProjectedAccess {
    value: NumericAccess,
    #[intrlay(project = "fields")]
    structured: NumericAccess,
    #[intrlay(project = "indexed")]
    indexed: Vec<ContractChild>,
}

#[derive(Intrlay)]
#[intrlay(transparent)]
struct RecursiveAlias(Option<Box<RecursiveAlias>>);
#[derive(Intrlay)]
struct Contracts {
    editable: ContractChild,
    #[intrlay(generate_setters = false)]
    read_only: ContractChild,
    #[intrlay(generate_getters = false)]
    write_only: ContractChild,
    #[intrlay(generate_setters = false)]
    read_only_array: [u32; 3],
    #[intrlay(generate_setters = false)]
    #[allow(
        clippy::box_collection,
        reason = "exercise child accessor independence across Option, Box, and Vec"
    )]
    read_only_nested: Option<Box<Vec<ContractChild>>>,
    #[intrlay(generate_setters = true)]
    fixed: ConstantChild,
    identifier: Identifier,
    identifiers: Vec<Identifier>,
    maybe: Option<MaybeValue>,
    label: Label,
}

#[derive(Intrlay)]
enum RenamedVariants {
    #[intrlay(name = "Common", typescript(name = "Specific"))]
    Source { value: u32 },
    #[intrlay(generate_getters = false)]
    Secret { item: ContractChild },
}

fn fixture() -> Contracts {
    Contracts {
        editable: ContractChild { value: 1 },
        read_only: ContractChild { value: 2 },
        write_only: ContractChild { value: 3 },
        read_only_array: [4, 5, 6],
        read_only_nested: Some(Box::new(vec![ContractChild { value: 7 }])),
        fixed: ConstantChild { value: 8 },
        identifier: Identifier(9),
        identifiers: vec![Identifier(10), Identifier(11)],
        maybe: Some(MaybeValue(None)),
        label: Label("hello".into()),
    }
}

fn emit_annotation_contracts(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Contracts>("Contracts");
    registry.export::<RenamedVariants>("RenamedVariants");
    registry.export::<ProjectedAccess>("ProjectedAccess");
    registry.export::<IndexedHints>("IndexedHints");
    let schema = registry.finish();
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(
        directory.join("contracts.bin"),
        intrlay::create::<Contracts>(&fixture())?.as_ref(),
    )?;
    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::Cpp, "hpp"),
        (Language::CSharp, "cs"),
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("contracts.{extension}")),
            generate_with_options(
                &schema,
                language,
                GenerateOptions {
                    module_name: Some("Contracts".into()),
                    ..Default::default()
                },
            )?,
        )?;
    }
    Ok(())
}

#[test]
fn indexed_helpers_respect_element_declaration_hints() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<IndexedHints>("IndexedHints");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        for (stem, getters, setters) in [
            ("read", true, false),
            ("write", false, true),
            ("hidden", false, false),
        ] {
            for (prefix, expected) in [("get", getters), ("set", setters)] {
                for suffix in ["score", "vector_component"] {
                    let raw = format!("{prefix}_{stem}_{suffix}");
                    let name = if language == Language::Python {
                        raw
                    } else {
                        intrlay::schema::NameCase::CamelCase.apply(&raw)
                    };
                    assert_eq!(
                        source.contains(&format!("{name}(")),
                        expected,
                        "{language:?}: {name}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn native_backends_ignore_accessor_generation_metadata() -> Result<(), Box<dyn Error>> {
    for language in [Language::Rust, Language::Cpp, Language::CSharp] {
        let mut registry = Registry::new();
        registry.export::<ContractChild>("ContractChild");
        let mut schema = registry.finish();
        let complete = generate(&schema, language)?;
        let definition = schema.definitions.values_mut().next().unwrap();
        let options = TargetOptions {
            generate_getters: Some(false),
            generate_setters: Some(false),
            ..Default::default()
        };
        definition.targets.insert(language, options.clone());
        let intrlay::schema::DefinitionKind::Record(fields) = &mut definition.kind else {
            unreachable!()
        };
        fields[0].targets.insert(language, options);
        assert_eq!(generate(&schema, language)?, complete, "{language:?}");
    }
    Ok(())
}

#[test]
fn common_projections_apply_only_without_value_types() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<ProjectedAccess>("ProjectedAccess");
    let schema = registry.finish();
    let numeric = schema
        .definitions
        .values()
        .find(|value| value.name == "NumericAccess")
        .unwrap();
    let parent = schema
        .definitions
        .values()
        .find(|value| value.name == "ProjectedAccess")
        .unwrap();
    let DefinitionKind::Record(fields) = &parent.kind else {
        unreachable!()
    };
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        assert_eq!(
            numeric.targets[&language].exposure,
            Some(Exposure::TypedArray(Scalar::F32))
        );
        assert_eq!(
            fields[1].targets[&language].exposure,
            Some(Exposure::Fields)
        );
        assert_eq!(
            fields[2].targets[&language].exposure,
            Some(Exposure::Indexed)
        );
        generate(&schema, language)?;
    }
    let mut unprojected = schema.clone();
    for definition in unprojected.definitions.values_mut() {
        for options in definition.targets.values_mut() {
            options.exposure = None;
        }
        if let DefinitionKind::Record(fields) = &mut definition.kind {
            for field in fields {
                for options in field.targets.values_mut() {
                    options.exposure = None;
                }
            }
        }
    }
    for language in [Language::Rust, Language::Cpp, Language::CSharp] {
        assert!(!numeric.targets.contains_key(&language));
        assert!(!fields[1].targets.contains_key(&language));
        assert!(!fields[2].targets.contains_key(&language));
        assert_eq!(
            generate(&schema, language)?,
            generate(&unprojected, language)?,
            "{language:?}"
        );
    }
    Ok(())
}

#[test]
fn rejects_recursive_aliases_only_where_wrappers_are_removed() {
    let mut registry = Registry::new();
    registry.export::<RecursiveAlias>("RecursiveAlias");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        assert!(
            generate(&schema, language)
                .unwrap_err()
                .to_string()
                .contains("recursive transparent alias")
        );
    }
    for language in [Language::Rust, Language::Cpp, Language::CSharp] {
        assert!(generate(&schema, language).is_ok(), "{language:?}");
    }
}

#[test]
fn rejects_generated_name_collisions_and_invalid_metadata() {
    for language in [
        Language::Rust,
        Language::Cpp,
        Language::CSharp,
        Language::TypeScript,
        Language::Python,
        Language::Dart,
    ] {
        let mut registry = Registry::new();
        registry.export::<ContractChild>("ContractChild");
        let mut schema = registry.finish();
        let definition = schema.definitions.values_mut().next().unwrap();
        let intrlay::schema::DefinitionKind::Record(fields) = &mut definition.kind else {
            unreachable!()
        };
        fields[0].targets.insert(
            language,
            TargetOptions {
                name: Some(
                    match language {
                        Language::Rust => "as_bytes",
                        Language::CSharp => "Bytes",
                        Language::Python => "buffer",
                        _ => "bytes",
                    }
                    .into(),
                ),
                ..Default::default()
            },
        );
        assert!(generate(&schema, language).is_err(), "{language:?}");
    }
    let mut registry = Registry::new();
    registry.export::<ContractChild>("ContractChild");
    let mut schema = registry.finish();
    let definition = schema.definitions.values_mut().next().unwrap();
    definition.targets.insert(
        Language::Python,
        TargetOptions {
            name: Some("class".into()),
            ..Default::default()
        },
    );
    assert!(generate(&schema, Language::Python).is_err());

    let intrlay::schema::DefinitionKind::Record(fields) =
        &mut schema.definitions.values_mut().next().unwrap().kind
    else {
        unreachable!()
    };
    fields[0].targets.insert(
        Language::CSharp,
        TargetOptions {
            name: Some("class".into()),
            ..Default::default()
        },
    );
    assert!(generate(&schema, Language::CSharp).is_err());
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    emit_annotation_contracts(fixtures)?;
    Ok(())
}

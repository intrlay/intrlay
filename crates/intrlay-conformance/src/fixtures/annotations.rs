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
enum IndexedChoice {
    Empty,
    Number {
        value: u32,
    },
    Optional {
        value: Option<u32>,
    },
    Data {
        label: String,
        #[intrlay(generate_component_accessors = true)]
        vector: [f32; 3],
        child: ContractChild,
    },
    Unit {
        value: (),
    },
}

#[derive(Intrlay)]
struct IndexedEnums {
    #[intrlay(project = "indexed_only")]
    items: Vec<IndexedChoice>,
    #[intrlay(project = "indexed")]
    fixed: [IndexedChoice; 2],
}

#[derive(Intrlay)]
#[intrlay(project = "u32")]
enum NumericTag {
    First = 1,
    Second = 2,
}

#[derive(Intrlay)]
struct NumericEnumFields {
    #[intrlay(project = "fields")]
    plain: NumericTag,
    numeric: NumericTag,
}

#[derive(Intrlay)]
struct NumericInputs {
    #[intrlay(project = "f32")]
    matrix: [[f32; 2]; 2],
    #[intrlay(project = "f32")]
    points: Vec<[f32; 3]>,
    #[intrlay(project = "u32")]
    tags: Vec<NumericTag>,
    #[intrlay(project = "f32")]
    empty: [f32; 0],
}

#[derive(Intrlay)]
struct EmptyProjected {}

#[derive(Intrlay)]
struct ZeroSizeSequences {
    #[intrlay(project = "f64")]
    numeric: Vec<EmptyProjected>,
    ordinary: Vec<EmptyProjected>,
}

#[derive(Intrlay)]
struct EmptyNumericTail {
    prefix: [u8; 9],
    #[intrlay(project = "f64")]
    trailer: EmptyProjected,
}

#[derive(Intrlay)]
enum FlattenedEmptyChoice {
    Absent,
    Empty {
        #[intrlay(project = "flatten")]
        payload: EmptyProjected,
    },
}

#[derive(Intrlay)]
enum CachedEmptyChoice {
    Absent,
    Empty {
        #[intrlay(project = "f32")]
        payload: [f32; 0],
    },
}

#[derive(Intrlay)]
struct ProjectionEdges {
    flattened: FlattenedEmptyChoice,
    #[intrlay(project = "indexed_only")]
    items: Vec<FlattenedEmptyChoice>,
    cached: CachedEmptyChoice,
    #[intrlay(project = "f32")]
    floats: EmptyProjected,
    #[intrlay(project = "u32")]
    integers: EmptyProjected,
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
    #[intrlay(generate_component_accessors = true)]
    read_vector: [f32; 3],
}
#[derive(Intrlay)]
#[intrlay(generate_getters = false)]
struct WriteIndexed {
    write_score: u32,
    #[intrlay(generate_component_accessors = true)]
    write_vector: [f32; 3],
}
#[derive(Intrlay)]
#[intrlay(generate_getters = false, generate_setters = false)]
struct HiddenIndexed {
    hidden_score: u32,
    #[intrlay(generate_component_accessors = true)]
    hidden_vector: [f32; 3],
}
#[derive(Intrlay)]
struct IndexedHints {
    #[intrlay(project = "indexed")]
    readable: Vec<ReadIndexed>,
    #[intrlay(project = "indexed")]
    writable: Vec<WriteIndexed>,
    #[intrlay(project = "indexed")]
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
    ordinary: Vec<ContractChild>,
    #[intrlay(project = "indexed_only")]
    direct: Vec<ContractChild>,
}

#[derive(Intrlay)]
#[repr(C, align(16))]
struct FlatPoint {
    /// Horizontal coordinate.
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Intrlay)]
struct IndexedEntry {
    id: u32,
    label: String,
    plain: [f32; 3],
    #[intrlay(generate_component_accessors = true)]
    lanes: [f32; 3],
    #[intrlay(project = "f32")]
    projected: FlatPoint,
    #[intrlay(project = "f32", generate_component_accessors = true)]
    direct: FlatPoint,
    child: ContractChild,
    #[intrlay(project = "flatten")]
    flat: [u16; 2],
}

#[derive(Intrlay)]
struct IndexedOnly {
    #[intrlay(project = "indexed_only")]
    items: Vec<IndexedEntry>,
}

#[derive(Intrlay)]
struct IndexedWriteEntry {
    id: u32,
    values: Vec<u32>,
}

#[derive(Intrlay)]
struct IndexedWriteOnly {
    #[intrlay(project = "indexed_only", generate_getters = false)]
    items: Vec<IndexedWriteEntry>,
    #[intrlay(project = "indexed_only", generate_getters = false)]
    fixed: [IndexedWriteEntry; 1],
}

fn indexed_only_fixture() -> IndexedOnly {
    IndexedOnly {
        items: vec![IndexedEntry {
            id: 7,
            label: "entry".into(),
            plain: [1.0, 2.0, 3.0],
            lanes: [4.0, 5.0, 6.0],
            projected: FlatPoint {
                x: 7.0,
                y: 8.0,
                z: 9.0,
            },
            direct: FlatPoint {
                x: 10.0,
                y: 11.0,
                z: 12.0,
            },
            child: ContractChild { value: 13 },
            flat: [14, 15],
        }],
    }
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct IndexedValue {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct IndexedMutableEntry {
    label: String,
    #[intrlay(generate_getters = false)]
    child: IndexedValue,
    lanes: [f32; 2],
    #[intrlay(generate_component_accessors = false)]
    plain: [f32; 2],
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct IndexedFixed {
    #[intrlay(project = "indexed_only", generate_component_accessors = true)]
    with_components: [IndexedMutableEntry; 1],
    #[intrlay(project = "indexed_only")]
    without_components: [IndexedMutableEntry; 1],
}

fn indexed_fixed_fixture() -> IndexedFixed {
    let entry = || IndexedMutableEntry {
        label: "fixed".into(),
        child: IndexedValue { value: 13 },
        lanes: [1.0, 2.0],
        plain: [3.0, 4.0],
    };
    IndexedFixed {
        with_components: [entry()],
        without_components: [entry()],
    }
}

#[derive(Intrlay)]
struct FlatDetails {
    id: u32,
    enabled: bool,
    weight: f64,
    #[intrlay(generate_setters = false)]
    fixed: u16,
}

#[derive(Intrlay)]
#[intrlay(project = "flatten")]
struct FlattenedAccess {
    min: [f32; 3],
    max: FlatPoint,
    details: FlatDetails,
    grid: [[u16; 2]; 2],
    label: String,
    maybe: Option<u32>,
    #[intrlay(project = "fields")]
    structured: NumericBranch,
    #[intrlay(project = "f32")]
    lanes: [f32; 3],
}

#[derive(Intrlay)]
struct SelectiveFlatten {
    #[intrlay(project = "flatten", name = "lower")]
    min: FlatPoint,
    max: FlatPoint,
}

fn flattened_fixture() -> FlattenedAccess {
    FlattenedAccess {
        min: [1.0, 2.0, 3.0],
        max: FlatPoint {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        },
        details: FlatDetails {
            id: 7,
            enabled: true,
            weight: 8.5,
            fixed: 9,
        },
        grid: [[10, 11], [12, 13]],
        label: "mixed fields".into(),
        maybe: Some(14),
        structured: NumericBranch {
            leaf: NumericLeaf { value: 15.0 },
        },
        lanes: [16.0, 17.0, 18.0],
    }
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
    registry.export::<IndexedEnums>("IndexedEnums");
    registry.export::<NumericInputs>("NumericInputs");
    registry.export::<ProjectionEdges>("ProjectionEdges");
    registry.export::<ZeroSizeSequences>("ZeroSizeSequences");
    registry.export::<EmptyNumericTail>("EmptyNumericTail");
    registry.export::<NumericEnumFields>("NumericEnumFields");
    registry.export::<NumericTag>("NumericTag");
    registry.export::<NumericTag>("ZNumericTag");
    // These consumers also inspect the transparent wrapper's standalone layout.
    registry.export::<Identifier>("Identifier");
    registry.export::<RenamedVariants>("RenamedVariants");
    registry.export::<ProjectedAccess>("ProjectedAccess");
    registry.export::<IndexedHints>("IndexedHints");
    registry.export::<FlattenedAccess>("FlattenedAccess");
    registry.export::<SelectiveFlatten>("SelectiveFlatten");
    registry.export::<IndexedOnly>("IndexedOnly");
    registry.export::<IndexedFixed>("IndexedFixed");
    registry.export::<IndexedWriteOnly>("IndexedWriteOnly");
    let schema = registry.finish();
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(
        directory.join("contracts.bin"),
        intrlay::create::<Contracts>(&fixture())?.as_ref(),
    )?;
    fixtures.insert(
        directory.join("flattened.bin"),
        intrlay::create::<FlattenedAccess>(&flattened_fixture())?.as_ref(),
    )?;
    fixtures.insert(
        directory.join("indexed-only.bin"),
        intrlay::create::<IndexedOnly>(&indexed_only_fixture())?.as_ref(),
    )?;
    emit_projection_payloads(fixtures)?;
    let mut indexed_fixed = indexed_fixed_fixture();
    fixtures.insert(
        directory.join("indexed-fixed.bin"),
        intrlay::create::<IndexedFixed>(&indexed_fixed)?.as_ref(),
    )?;
    indexed_fixed.with_components[0].child.value = 31;
    indexed_fixed.with_components[0].lanes[1] = 20.0;
    indexed_fixed.without_components[0].child.value = 32;
    fixtures.insert(
        directory.join("indexed-fixed-changed.bin"),
        intrlay::create::<IndexedFixed>(&indexed_fixed)?.as_ref(),
    )?;
    let indexed_write_only = IndexedWriteOnly {
        items: vec![IndexedWriteEntry {
            id: 17,
            values: vec![1, 20],
        }],
        fixed: [IndexedWriteEntry {
            id: 7,
            values: vec![30, 2],
        }],
    };
    fixtures.insert(
        directory.join("indexed-write-only-changed.bin"),
        intrlay::create::<IndexedWriteOnly>(&indexed_write_only)?.as_ref(),
    )?;
    let mut changed = flattened_fixture();
    changed.min[1] = 20.0;
    changed.max.z = 60.0;
    changed.details.enabled = false;
    changed.details.weight = 80.5;
    changed.grid[1][0] = 120;
    fixtures.insert(
        directory.join("flattened-changed.bin"),
        intrlay::create::<FlattenedAccess>(&changed)?.as_ref(),
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
fn collection_views_combine_reachable_access_requests() -> Result<(), Box<dyn Error>> {
    #[derive(Intrlay)]
    struct Requests<T: Intrlay + 'static> {
        first: T,
        second: T,
        // Encoding this field does not request its default element-view API.
        #[intrlay(generate_getters = false, generate_setters = false)]
        hidden: T,
    }

    fn schema<T: Intrlay<Value = T> + 'static>() -> intrlay::schema::Schema {
        let mut registry = Registry::new();
        registry.export::<Requests<T>>("Requests");
        registry.finish()
    }

    for (original, collection) in [
        (schema::<Vec<ContractChild>>(), "SequenceOfContractChild"),
        (schema::<[ContractChild; 2]>(), "Array2OfContractChild"),
    ] {
        for first in [Exposure::Fields, Exposure::Indexed, Exposure::IndexedOnly] {
            for second in [Exposure::Fields, Exposure::Indexed, Exposure::IndexedOnly] {
                let mut schema = original.clone();
                let root = schema
                    .definitions
                    .values_mut()
                    .find(|node| node.name == "Requests")
                    .unwrap();
                let DefinitionKind::Record(fields) = &mut root.kind else {
                    unreachable!()
                };
                for (field, exposure) in fields.iter_mut().zip([first, second]) {
                    for language in [Language::TypeScript, Language::Python, Language::Dart] {
                        field.targets.insert(
                            language,
                            TargetOptions {
                                exposure: Some(exposure),
                                ..Default::default()
                            },
                        );
                    }
                }
                for language in [Language::TypeScript, Language::Python, Language::Dart] {
                    let source = generate(&schema, language)?;
                    let name = format!("class Intrlay{collection}View");
                    assert_eq!(
                        source.matches(&name).count(),
                        1,
                        "{language:?}: {first:?}, {second:?}"
                    );
                    assert!(!source.contains(&format!("{name}Indexed")));
                    assert_eq!(
                        source.contains("class ContractChildView"),
                        first != Exposure::IndexedOnly || second != Exposure::IndexedOnly
                    );
                    let getter = if language == Language::Python {
                        "get_value("
                    } else {
                        "getValue("
                    };
                    assert_eq!(
                        source.matches(getter).count(),
                        usize::from(first != Exposure::Fields || second != Exposure::Fields)
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn indexed_only_emits_only_reachable_views() -> Result<(), Box<dyn Error>> {
    #[derive(Intrlay)]
    struct SharedEntry {
        entry: IndexedEntry,
    }

    let mut registry = Registry::new();
    registry.export::<IndexedOnly>("IndexedOnly");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        assert!(!source.contains("class IndexedEntryView"));
        assert!(source.contains("IndexedEntryInput"));
        assert!(source.contains("class ContractChildView"));
        let case = if language == Language::Python {
            intrlay::schema::NameCase::SnakeCase
        } else {
            intrlay::schema::NameCase::CamelCase
        };
        for (field, present) in [
            ("plain", false),
            ("lanes", true),
            ("projected", false),
            ("direct", true),
        ] {
            for prefix in ["get", "set"] {
                let name = case.apply(&format!("{prefix}_{field}_component"));
                assert_eq!(
                    source.contains(&format!("{name}(")),
                    present,
                    "{language:?}: {name}"
                );
            }
        }
    }
    // Either another ordinary accessor or an explicit export keeps the element view.
    for standalone in [false, true] {
        let mut registry = Registry::new();
        registry.export::<IndexedOnly>("IndexedOnly");
        if standalone {
            registry.export::<IndexedEntry>("IndexedEntry");
        } else {
            registry.export::<SharedEntry>("SharedEntry");
        }
        let schema = registry.finish();
        for language in [Language::TypeScript, Language::Python, Language::Dart] {
            assert!(generate(&schema, language)?.contains("class IndexedEntryView"));
        }
    }
    Ok(())
}

#[test]
fn indexed_only_preserves_field_writers_without_element_views() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<IndexedFixed>("IndexedFixed");
    registry.export::<IndexedWriteOnly>("IndexedWriteOnly");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        assert!(!source.contains("class IndexedMutableEntryView"));
        assert!(source.contains("class IndexedValueView"));
        assert!(!source.contains("class IndexedWriteEntryView"));
        assert!(!source.contains("IndexedFixedInput"));
        assert!(!source.contains("IndexedMutableEntryInput"));
        assert!(source.contains("IndexedValueInput"));
    }
    Ok(())
}

#[test]
fn indexed_projections_require_non_transparent_record_elements() {
    #[derive(Intrlay)]
    struct InvalidIndexedOnly<T: Intrlay + 'static> {
        #[intrlay(project = "indexed_only")]
        values: T,
    }
    fn schema<T: Intrlay<Value = T> + 'static>() -> intrlay::schema::Schema {
        let mut registry = Registry::new();
        registry.export::<InvalidIndexedOnly<T>>("InvalidIndexedOnly");
        registry.finish()
    }

    for mut schema in [
        schema::<u32>(),
        schema::<Vec<u32>>(),
        schema::<Vec<Option<u32>>>(),
        schema::<[Option<u32>; 2]>(),
        schema::<Vec<Identifier>>(),
    ] {
        for (exposure, name) in [
            (Exposure::IndexedOnly, "indexed_only"),
            (Exposure::Indexed, "indexed"),
        ] {
            for definition in schema.definitions.values_mut() {
                if let DefinitionKind::Record(fields) = &mut definition.kind {
                    for field in fields {
                        for options in field.targets.values_mut() {
                            if options.exposure.is_some() {
                                options.exposure = Some(exposure);
                            }
                        }
                    }
                }
            }
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                assert!(
                    generate(&schema, language)
                        .unwrap_err()
                        .to_string()
                        .contains(&format!("{name} projection requires"))
                );
            }
        }
    }
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
                        Language::Cpp => "bytes",
                        Language::TypeScript | Language::Python | Language::Dart => "storage",
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

#[test]
fn flattened_paths_report_collisions_and_leave_native_apis_unchanged() {
    #[derive(Intrlay)]
    #[intrlay(project = "flatten")]
    struct Collision {
        min: FlatPoint,
        min_x: f32,
    }
    let mut registry = Registry::new();
    registry.export::<Collision>("Collision");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let error = generate(&schema, language).unwrap_err();
        assert!(error.to_string().contains("min"), "{language:?}: {error}");
    }
    for language in [Language::Rust, Language::Cpp, Language::CSharp] {
        assert!(generate(&schema, language).is_ok(), "{language:?}");
    }
}

fn emit_projection_payloads(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let indexed = intrlay::create::<IndexedEnums>(&IndexedEnums {
        items: vec![
            IndexedChoice::Data {
                label: "payload".into(),
                vector: [1.0, 2.0, 3.0],
                child: ContractChild { value: 7 },
            },
            IndexedChoice::Empty,
            IndexedChoice::Optional { value: None },
            IndexedChoice::Unit { value: () },
        ],
        fixed: [
            IndexedChoice::Number { value: 9 },
            IndexedChoice::Optional { value: Some(10) },
        ],
    })?;
    let numeric = intrlay::create::<NumericInputs>(&NumericInputs {
        matrix: [[1.0, 2.0], [3.0, 4.0]],
        points: vec![[5.0, 6.0, 7.0], [8.0, 9.0, 10.0]],
        tags: vec![NumericTag::Second, NumericTag::First],
        empty: [],
    })?;
    let zero_size = intrlay::create::<ZeroSizeSequences>(&ZeroSizeSequences {
        numeric: vec![EmptyProjected {}, EmptyProjected {}],
        ordinary: vec![EmptyProjected {}, EmptyProjected {}, EmptyProjected {}],
    })?;
    fixtures.insert("regressions/indexed-enums.bin", indexed.as_ref())?;
    fixtures.insert("regressions/numeric-inputs.bin", numeric.as_ref())?;
    fixtures.insert("regressions/projection-data.dart", format!(
        "// Rust reference encodings for native and JavaScript consumers.\nconst indexedEnumBytes = {:?};\nconst numericInputBytes = {:?};\nconst zeroSizeSequenceBytes = {:?};\n",
        indexed.as_ref(), numeric.as_ref(), zero_size.as_ref(),
    ))?;
    Ok(())
}

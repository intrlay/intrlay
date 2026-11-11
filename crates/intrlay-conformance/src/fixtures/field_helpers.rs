//! Field helper policies preserve canonical wire ranges without materialized values.

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

use super::Fixtures;

#[derive(Intrlay)]
#[repr(C, align(16))]
struct PaddedVector {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Intrlay)]
struct HelperChild {
    code: u32,
    name: String,
}

#[derive(Intrlay)]
struct HelperFlat {
    first: u32,
    rest: [f32; 2],
}

#[derive(Intrlay)]
#[intrlay(
    generate_value_accessors = false,
    generate_byte_range_accessors = true,
    generate_component_accessors = true
)]
struct FieldRecord {
    matrix: [[f32; 4]; 4],
    #[intrlay(project = "fields")]
    padded: PaddedVector,
    label: String,
    #[intrlay(project = "f32")]
    values: Vec<[f32; 2]>,
    maybe: Option<[f32; 2]>,
    boxed: Box<[f32; 2]>,
    empty: [f32; 0],
    child: HelperChild,
    #[intrlay(project = "flatten")]
    flat: HelperFlat,
}

#[derive(Intrlay)]
#[intrlay(
    generate_value_accessors = false,
    generate_byte_range_accessors = true,
    generate_component_accessors = true
)]
struct VisibleRecord {
    #[intrlay(generate_value_accessors = true, generate_component_accessors = false)]
    lanes: [f32; 2],
}

#[derive(Intrlay)]
struct FieldRows {
    #[intrlay(
        project = "indexed_only",
        generate_value_accessors = false,
        generate_byte_range_accessors = true,
        generate_component_accessors = true
    )]
    rows: Vec<FieldRecord>,
    #[intrlay(project = "indexed")]
    visible: Vec<VisibleRecord>,
}

#[derive(Intrlay)]
#[intrlay(
    generate_value_accessors = false,
    generate_byte_range_accessors = true,
    generate_component_accessors = true
)]
enum RawChoice {
    Empty,
    Data {
        value: [f32; 2],
        label: String,
        maybe: Option<[f32; 2]>,
        boxed: Box<[f32; 2]>,
    },
}

#[derive(Intrlay)]
struct RawChoices {
    #[intrlay(
        project = "indexed_only",
        generate_value_accessors = false,
        generate_byte_range_accessors = true,
        generate_component_accessors = true
    )]
    items: Vec<RawChoice>,
}

#[derive(Intrlay)]
#[intrlay(
    generate_value_accessors = false,
    generate_byte_range_accessors = true,
    generate_component_accessors = true,
    generate_setters = false
)]
struct ReadHelper {
    lanes: [f32; 2],
}

#[derive(Intrlay)]
#[intrlay(
    generate_value_accessors = false,
    generate_byte_range_accessors = true,
    generate_component_accessors = true,
    generate_getters = false
)]
struct WriteHelper {
    lanes: [f32; 2],
}

#[derive(Intrlay)]
#[intrlay(
    generate_value_accessors = false,
    generate_byte_range_accessors = true,
    generate_component_accessors = true,
    generate_getters = false,
    generate_setters = false
)]
struct CreateHelper {
    lanes: [f32; 2],
}

#[derive(Intrlay)]
struct LanguageHelper {
    #[intrlay(
        generate_value_accessors = false,
        generate_byte_range_accessors = true,
        generate_component_accessors = true,
        typescript(generate_value_accessors = true, generate_byte_range_accessors = false),
        python(generate_component_accessors = false),
        dart(generate_value_accessors = true)
    )]
    filtered: [f32; 2],
}

#[derive(Intrlay)]
#[intrlay(transparent)]
struct FoldedArray(Vec<u32>);

#[derive(Intrlay)]
struct IndexedSample {
    lanes: [f32; 2],
}

#[derive(Intrlay)]
#[intrlay(transparent, create = false)]
struct IndexedSamples(
    #[intrlay(
        project = "indexed_only",
        generate_value_accessors = false,
        generate_component_accessors = true
    )]
    Vec<IndexedSample>,
);

#[derive(Intrlay)]
#[intrlay(transparent, create = false)]
struct NestedSamples(IndexedSamples);

#[derive(Intrlay)]
#[intrlay(generate_value_accessors = false)]
struct FoldedHelpers {
    samples: NestedSamples,
}

#[derive(Intrlay)]
#[intrlay(generate_value_accessors = false, generate_byte_range_accessors = true)]
struct ArrayRanges {
    records: Vec<HelperChild>,
    fixed: [HelperChild; 2],
    padded: Vec<PaddedVector>,
    nested: Vec<Vec<u32>>,
    empty: Vec<u32>,
    units: Vec<()>,
    folded: FoldedArray,
}

#[derive(Intrlay)]
#[intrlay(generate_value_accessors = false, generate_byte_range_accessors = true)]
enum ArrayChoice {
    Empty,
    Data {
        records: Vec<HelperChild>,
        units: Vec<()>,
    },
}

#[derive(Intrlay)]
struct ArrayChoices {
    #[intrlay(project = "indexed_only")]
    items: Vec<ArrayChoice>,
}

fn record() -> FieldRecord {
    FieldRecord {
        matrix: [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ],
        padded: PaddedVector {
            x: 17.0,
            y: 18.0,
            z: 19.0,
        },
        label: "field bytes".into(),
        values: vec![[21.0, 22.0], [23.0, 24.0]],
        maybe: Some([25.0, 26.0]),
        boxed: Box::new([27.0, 28.0]),
        empty: [],
        child: HelperChild {
            code: 29,
            name: "nested".into(),
        },
        flat: HelperFlat {
            first: 30,
            rest: [31.0, 32.0],
        },
    }
}

fn change(value: &mut FieldRecord) {
    value.matrix[3][3] = 99.0;
    value.padded.y = 61.0;
    value.values[1][0] = 51.0;
    value.maybe = Some([25.0, 41.0]);
    value.boxed[0] = 71.0;
}

fn choice() -> RawChoice {
    RawChoice::Data {
        value: [1.0, 2.0],
        label: "choice".into(),
        maybe: None,
        boxed: Box::new([3.0, 4.0]),
    }
}

fn schema() -> intrlay::schema::Schema {
    let mut registry = Registry::new();
    registry.export::<FieldRecord>("FieldRecord");
    registry.export::<FieldRows>("FieldRows");
    registry.export::<RawChoice>("RawChoice");
    registry.export::<RawChoices>("RawChoices");
    registry.export::<ReadHelper>("ReadHelper");
    registry.export::<WriteHelper>("WriteHelper");
    registry.export::<CreateHelper>("CreateHelper");
    registry.export::<LanguageHelper>("LanguageHelper");
    registry.export::<ArrayRanges>("ArrayRanges");
    registry.export::<ArrayChoice>("ArrayChoice");
    registry.export::<ArrayChoices>("ArrayChoices");
    registry.export::<FoldedHelpers>("FoldedHelpers");
    registry.finish()
}

fn insert_bytes<T: Intrlay>(
    fixtures: &mut Fixtures,
    dart: &mut String,
    name: &str,
    value: &T::Value,
) -> Result<(), Box<dyn Error>> {
    use std::fmt::Write;

    let bytes = intrlay::create::<T>(value)?;
    fixtures.insert(format!("field-helpers/{name}.bin"), bytes.as_ref())?;
    writeln!(dart, "const {name}Bytes = {:?};", bytes.as_ref())?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    for (language, extension) in [
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            format!("field-helpers/bindings.{extension}"),
            generate(&schema(), language)?,
        )?;
    }
    let mut dart =
        String::from("// Rust reference encodings, also used by Dart JavaScript tests.\n");
    let mut value = record();
    insert_bytes::<FieldRecord>(fixtures, &mut dart, "record", &value)?;
    change(&mut value);
    insert_bytes::<FieldRecord>(fixtures, &mut dart, "changed", &value)?;
    value = record();
    value.maybe = None;
    insert_bytes::<FieldRecord>(fixtures, &mut dart, "absent", &value)?;
    let mut rows = FieldRows {
        rows: vec![record(), record()],
        visible: vec![VisibleRecord { lanes: [7.0, 8.0] }],
    };
    insert_bytes::<FieldRows>(fixtures, &mut dart, "rows", &rows)?;
    change(&mut rows.rows[1]);
    rows.visible[0].lanes[0] = 70.0;
    insert_bytes::<FieldRows>(fixtures, &mut dart, "rowsChanged", &rows)?;
    insert_bytes::<RawChoice>(fixtures, &mut dart, "inactive", &RawChoice::Empty)?;
    insert_bytes::<RawChoice>(fixtures, &mut dart, "active", &choice())?;
    let mut choices = RawChoices {
        items: vec![RawChoice::Empty, choice()],
    };
    insert_bytes::<RawChoices>(fixtures, &mut dart, "choices", &choices)?;
    if let RawChoice::Data { value, boxed, .. } = &mut choices.items[1] {
        value[1] = 90.0;
        boxed[0] = 80.0;
    }
    insert_bytes::<RawChoices>(fixtures, &mut dart, "choicesChanged", &choices)?;
    let child = |code| HelperChild {
        code,
        name: format!("child {code}"),
    };
    insert_bytes::<ArrayRanges>(
        fixtures,
        &mut dart,
        "arrays",
        &ArrayRanges {
            records: vec![child(101), child(202)],
            fixed: [child(303), child(404)],
            padded: vec![
                PaddedVector {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                PaddedVector {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                },
            ],
            nested: vec![vec![11, 12], vec![21]],
            empty: vec![],
            units: vec![(), (), ()],
            folded: FoldedArray(vec![51, 52]),
        },
    )?;
    insert_bytes::<ArrayChoices>(
        fixtures,
        &mut dart,
        "arrayChoices",
        &ArrayChoices {
            items: vec![
                ArrayChoice::Empty,
                ArrayChoice::Data {
                    records: vec![child(505)],
                    units: vec![(), ()],
                },
            ],
        },
    )?;
    let mut folded = FoldedHelpers {
        samples: NestedSamples(IndexedSamples(vec![IndexedSample { lanes: [3.0, 7.0] }])),
    };
    insert_bytes::<FoldedHelpers>(fixtures, &mut dart, "foldedHelpers", &folded)?;
    folded.samples.0.0[0].lanes[1] = 11.0;
    insert_bytes::<FoldedHelpers>(fixtures, &mut dart, "foldedHelpersChanged", &folded)?;
    fixtures.insert("field-helpers/data.dart", dart)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "fixture declarations have fixed names")]

    use intrlay::schema::{DefinitionKind, Exposure, NameCase, TargetOptions};

    use super::*;

    #[test]
    fn folded_indexed_helpers_retain_navigation_without_value_accessors()
    -> Result<(), Box<dyn Error>> {
        for language in [Language::TypeScript, Language::Python, Language::Dart] {
            let source = generate(&schema(), language)?;
            let root = class_source(&source, "FoldedHelpersView");
            let getter = if language == Language::Python {
                "def samples("
            } else {
                "get samples"
            };
            assert!(
                root.contains(getter),
                "{language:?}: missing indexed navigation"
            );
            assert!(!source.contains("class IndexedSampleView"));
            assert!(!source.contains("class IndexedSamplesView"));
            assert!(!source.contains("class NestedSamplesView"));
        }
        Ok(())
    }

    fn class_source(source: &str, class: &str) -> String {
        let start = source
            .find(&format!("class {class}"))
            .expect("fixture view");
        let rest = &source[start..];
        let end = rest[1..]
            .find("\nclass ")
            .or_else(|| rest[1..].find("\nexport class "))
            .map_or(rest.len(), |offset| offset + 1);
        rest[..end].to_owned()
    }

    #[test]
    fn helper_emission_does_not_restore_value_accessors_or_caches() -> Result<(), Box<dyn Error>> {
        for language in [Language::TypeScript, Language::Python, Language::Dart] {
            let source = generate(&schema(), language)?;
            let record = class_source(&source, "FieldRecordView");
            assert!(!source.contains("class HelperChildView"));
            assert!(!source.contains("class PaddedVectorView"));
            let snake = language == Language::Python;
            for method in if snake {
                [
                    "matrix_byte_offset",
                    "matrix_byte_length",
                    "get_matrix_component",
                    "set_matrix_component",
                    "label_byte_length",
                    "flat_first_byte_offset",
                ]
            } else {
                [
                    "matrixByteOffset",
                    "matrixByteLength",
                    "getMatrixComponent",
                    "setMatrixComponent",
                    "labelByteLength",
                    "flatFirstByteOffset",
                ]
            } {
                assert!(record.contains(method), "{language:?}: missing {method}");
            }
            for absent in if snake {
                [
                    "def matrix(",
                    "def set_matrix(",
                    "_cached_matrix",
                    "def flat(",
                    "flat_byte_offset",
                ]
            } else if language == Language::Dart {
                [
                    "get matrix =>",
                    "setMatrix(",
                    "_cachedMatrix",
                    "get flat =>",
                    "flatByteOffset",
                ]
            } else {
                [
                    "get matrix():",
                    "setMatrix(",
                    "_cachedMatrix",
                    "get flat():",
                    "flatByteOffset",
                ]
            } {
                assert!(
                    !record.contains(absent),
                    "{language:?}: unexpected {absent}"
                );
            }
            let read = class_source(&source, "ReadHelperView");
            let write = class_source(&source, "WriteHelperView");
            let create = class_source(&source, "CreateHelperView");
            let (get, set, range) = if snake {
                (
                    "get_lanes_component(",
                    "set_lanes_component(",
                    "lanes_byte_offset",
                )
            } else {
                (
                    "getLanesComponent(",
                    "setLanesComponent(",
                    "lanesByteOffset",
                )
            };
            assert!(read.contains(get) && read.contains(range) && !read.contains(set));
            assert!(write.contains(set) && !write.contains(get) && !write.contains(range));
            assert!(!create.contains(get) && !create.contains(set) && !create.contains(range));
        }
        Ok(())
    }

    #[test]
    fn language_overrides_and_native_apis_are_independent() -> Result<(), Box<dyn Error>> {
        let mut registry = Registry::new();
        registry.export::<LanguageHelper>("LanguageHelper");
        let schema = registry.finish();
        let ts = generate(&schema, Language::TypeScript)?;
        assert!(ts.contains("get filtered") && ts.contains("getFilteredComponent("));
        assert!(!ts.contains("filteredByteOffset"));
        let py = generate(&schema, Language::Python)?;
        assert!(py.contains("filtered_byte_offset"));
        assert!(!py.contains("def filtered(") && !py.contains("get_filtered_component("));
        let dart = generate(&schema, Language::Dart)?;
        assert!(dart.contains("get filtered") && dart.contains("filteredByteOffset"));
        let mut native_defaults = schema.clone();
        for definition in native_defaults.definitions.values_mut() {
            if let DefinitionKind::Record(fields) = &mut definition.kind {
                for field in fields {
                    for options in field.targets.values_mut() {
                        options.generate_value_accessors = None;
                        options.generate_byte_range_accessors = None;
                        options.generate_component_accessors = None;
                    }
                }
            }
        }
        for language in [Language::Rust, Language::Cpp, Language::CSharp] {
            assert_eq!(
                generate(&schema, language)?,
                generate(&native_defaults, language)?,
                "managed helper hints changed the native {language:?} API"
            );
        }
        Ok(())
    }

    #[derive(Intrlay)]
    #[intrlay(
        generate_byte_range_accessors = true,
        generate_component_accessors = true
    )]
    enum VariantOverrides {
        #[intrlay(generate_value_accessors = false)]
        Hidden {
            #[intrlay(generate_value_accessors = true)]
            restored: [f32; 2],
            omitted: [f32; 2],
        },
        #[intrlay(generate_value_accessors = true)]
        Shown {
            #[intrlay(generate_value_accessors = false)]
            suppressed: [f32; 2],
            retained: [f32; 2],
        },
    }

    #[derive(Intrlay)]
    struct IndexedVariantOverrides {
        #[intrlay(project = "indexed_only")]
        items: Vec<VariantOverrides>,
    }

    #[test]
    fn explicit_field_policy_overrides_variant_defaults() -> Result<(), Box<dyn Error>> {
        for indexed in [false, true] {
            let mut registry = Registry::new();
            if indexed {
                registry.export::<IndexedVariantOverrides>("IndexedVariantOverrides");
            } else {
                registry.export::<VariantOverrides>("VariantOverrides");
            }
            let schema = registry.finish();
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                let source = generate(&schema, language)?;
                for (snake, camel, emitted) in [
                    ("hidden_restored", "HiddenRestored", true),
                    ("hidden_omitted", "HiddenOmitted", false),
                    ("shown_suppressed", "ShownSuppressed", false),
                    ("shown_retained", "ShownRetained", true),
                ] {
                    let getter = if language == Language::Python {
                        format!("def {}{snake}(", if indexed { "get_" } else { "" })
                    } else if indexed {
                        format!("get{camel}(")
                    } else {
                        // Ordinary numeric getters cache their typed view, while
                        // indexed access deliberately has no per-element cache.
                        format!("_cached{camel}")
                    };
                    assert_eq!(
                        source.contains(&getter),
                        emitted,
                        "{language:?}, indexed={indexed}: {getter}"
                    );
                }
                let helper = if language == Language::Python {
                    "get_hidden_omitted_component("
                } else {
                    "getHiddenOmittedComponent("
                };
                assert!(source.contains(helper), "{language:?}: {helper}");
            }
        }
        Ok(())
    }

    #[derive(Intrlay)]
    struct SyntheticLeafRecord {
        first: u32,
        second: u32,
    }

    #[derive(Intrlay)]
    #[intrlay(generate_value_accessors = false, generate_byte_range_accessors = true)]
    enum SyntheticPayloadChoice {
        Payload {
            #[intrlay(project = "flatten")]
            value: SyntheticLeafRecord,
        },
        Whole {
            value: SyntheticLeafRecord,
        },
    }

    #[derive(Intrlay)]
    struct SyntheticPayloadRows {
        #[intrlay(project = "indexed_only")]
        rows: Vec<SyntheticPayloadChoice>,
    }

    #[test]
    fn flattened_synthetic_payloads_keep_distinct_leaf_range_names() -> Result<(), Box<dyn Error>> {
        for indexed in [false, true] {
            let mut registry = Registry::new();
            if indexed {
                registry.export::<SyntheticPayloadRows>("SyntheticPayloadRows");
            } else {
                registry.export::<SyntheticPayloadChoice>("SyntheticPayloadChoice");
            }
            let mut schema = registry.finish();
            let definition = schema
                .definitions
                .values_mut()
                .find(|node| node.name == "SyntheticPayloadChoice")
                .expect("synthetic enum");
            let DefinitionKind::Enum(variants) = &mut definition.kind else {
                unreachable!()
            };
            // Synthetic payload is frontend metadata; ordinary Rust derives
            // provide the same one-field shape for this source-schema regression.
            for variant in variants {
                variant.synthetic_payload = true;
            }
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                let source = generate(&schema, language)?;
                let names = if language == Language::Python {
                    NameCase::SnakeCase
                } else {
                    NameCase::CamelCase
                };
                let declaration = |name: &str| {
                    if indexed {
                        format!("{}(", names.apply(&format!("get_{name}")))
                    } else if language == Language::Python {
                        format!("def {name}(")
                    } else if language == Language::Dart {
                        format!("get {} =>", names.apply(name))
                    } else {
                        format!("get {}()", names.apply(name))
                    }
                };
                for name in [
                    "payload_value_first_byte_offset",
                    "payload_value_second_byte_offset",
                    "payload_value_first_byte_length",
                    "payload_value_second_byte_length",
                    "whole_byte_offset",
                    "whole_byte_length",
                ] {
                    let member = declaration(name);
                    assert_eq!(
                        source.matches(&member).count(),
                        1,
                        "{language:?}, indexed={indexed}: {member}"
                    );
                }
                for omitted in [
                    "payload_byte_offset",
                    "payload_value_byte_offset",
                    "whole_value_byte_offset",
                ] {
                    let member = declaration(omitted);
                    assert!(
                        !source.contains(&member),
                        "{language:?}, indexed={indexed}: {member}"
                    );
                }
            }
        }
        Ok(())
    }

    #[derive(Intrlay)]
    struct InheritedElement {
        lanes: [f32; 2],
    }

    #[derive(Intrlay)]
    #[intrlay(
        generate_value_accessors = false,
        generate_component_accessors = true,
        generate_byte_range_accessors = true
    )]
    struct InheritedHelpers {
        #[intrlay(project = "indexed")]
        rows: Vec<InheritedElement>,
    }

    #[test]
    fn indexed_collections_inherit_containing_helper_defaults() -> Result<(), Box<dyn Error>> {
        for exposure in [Exposure::Indexed, Exposure::IndexedOnly] {
            for overrides in [false, true] {
                let mut registry = Registry::new();
                registry.export::<InheritedHelpers>("InheritedHelpers");
                let mut schema = registry.finish();
                let definition = schema
                    .definitions
                    .values_mut()
                    .find(|node| node.name == "InheritedHelpers")
                    .expect("containing record");
                let DefinitionKind::Record(fields) = &mut definition.kind else {
                    unreachable!()
                };
                for language in [Language::TypeScript, Language::Python, Language::Dart] {
                    let options = fields[0].targets.entry(language).or_default();
                    options.exposure = Some(exposure);
                    if overrides {
                        options.generate_value_accessors = Some(true);
                        options.generate_component_accessors = Some(false);
                        options.generate_byte_range_accessors = Some(false);
                    }
                }
                for language in [Language::TypeScript, Language::Python, Language::Dart] {
                    let source = generate(&schema, language)?;
                    let owner = class_source(&source, "InheritedHelpersView");
                    let collection = class_source(&source, "IntrlaySequenceOfInheritedElementView");
                    let (navigation, element_getter, names) = match language {
                        Language::Python => ("def rows(", "def __getitem__(", NameCase::SnakeCase),
                        Language::Dart => ("get rows =>", "operator [](", NameCase::CamelCase),
                        _ => ("get rows()", "public get(", NameCase::CamelCase),
                    };
                    let context = format!("{language:?}, {exposure:?}, overrides={overrides}");
                    assert!(
                        owner.contains(navigation),
                        "{context}: missing collection navigation"
                    );
                    for (name, emitted) in [
                        ("get_lanes", overrides),
                        ("set_lanes", overrides),
                        ("get_lanes_component", !overrides),
                        ("set_lanes_component", !overrides),
                        ("get_lanes_byte_offset", !overrides),
                        ("get_lanes_byte_length", !overrides),
                    ] {
                        let accessor = format!("{}(", names.apply(name));
                        assert_eq!(
                            collection.contains(&accessor),
                            emitted,
                            "{context}: {accessor}"
                        );
                    }
                    let elements = overrides && exposure == Exposure::Indexed;
                    assert_eq!(
                        collection.contains(element_getter),
                        elements,
                        "{context}: element getter"
                    );
                    assert_eq!(
                        source.contains("class InheritedElementView"),
                        elements,
                        "{context}: element view reachability"
                    );
                    assert!(
                        !collection.contains("_cachedLanes")
                            && !collection.contains("_cached_lanes")
                    );
                }
            }
        }
        Ok(())
    }

    #[derive(Intrlay)]
    struct NestedVectorRow {
        #[intrlay(
            project = "indexed_only",
            generate_value_accessors = false,
            generate_component_accessors = true,
            generate_byte_range_accessors = true
        )]
        children: Vec<InheritedElement>,
    }

    #[derive(Intrlay)]
    struct NestedArrayRow {
        #[intrlay(
            project = "indexed_only",
            generate_value_accessors = false,
            generate_component_accessors = true,
            generate_byte_range_accessors = true
        )]
        children: [InheritedElement; 2],
    }

    #[derive(Intrlay)]
    struct NestedVectorRows {
        #[intrlay(project = "indexed_only")]
        rows: Vec<NestedVectorRow>,
    }

    #[derive(Intrlay)]
    struct NestedArrayRows {
        #[intrlay(project = "indexed_only")]
        rows: Vec<NestedArrayRow>,
    }

    #[test]
    fn nested_indexed_helpers_keep_navigation_without_whole_value_writers()
    -> Result<(), Box<dyn Error>> {
        for fixed in [false, true] {
            let mut registry = Registry::new();
            let (outer_name, inner_name) = if fixed {
                registry.export::<NestedArrayRows>("NestedRows");
                (
                    "IntrlaySequenceOfNestedArrayRowView",
                    "IntrlayArray2OfInheritedElementView",
                )
            } else {
                registry.export::<NestedVectorRows>("NestedRows");
                (
                    "IntrlaySequenceOfNestedVectorRowView",
                    "IntrlaySequenceOfInheritedElementView",
                )
            };
            let schema = registry.finish();
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                let source = generate(&schema, language)?;
                let outer = class_source(&source, outer_name);
                let inner = class_source(&source, inner_name);
                let (names, element_getter) = match language {
                    Language::Python => (NameCase::SnakeCase, "def __getitem__("),
                    Language::Dart => (NameCase::CamelCase, "operator []("),
                    _ => (NameCase::CamelCase, "public get("),
                };
                let context = format!("{language:?}, fixed={fixed}");
                for (name, emitted) in [
                    ("get_children", true),
                    ("set_children", false),
                    ("get_children_byte_offset", true),
                    ("get_children_byte_length", true),
                ] {
                    let accessor = format!("{}(", names.apply(name));
                    assert_eq!(
                        outer.contains(&accessor),
                        emitted,
                        "{context}: outer {accessor}"
                    );
                }
                for (name, emitted) in [
                    ("get_lanes_component", true),
                    ("set_lanes_component", true),
                    ("get_lanes_byte_offset", true),
                    ("get_lanes_byte_length", true),
                    ("get_lanes", false),
                    ("set_lanes", false),
                ] {
                    let accessor = format!("{}(", names.apply(name));
                    assert_eq!(
                        inner.contains(&accessor),
                        emitted,
                        "{context}: inner {accessor}"
                    );
                }
                assert!(
                    !outer.contains(element_getter),
                    "{context}: outer element getter"
                );
                assert!(
                    !inner.contains(element_getter),
                    "{context}: inner element getter"
                );
                assert!(
                    !source.contains("class InheritedElementView"),
                    "{context}: unused element view"
                );
                assert!(!inner.contains("_cachedLanes") && !inner.contains("_cached_lanes"));
            }
        }
        Ok(())
    }

    #[derive(Intrlay)]
    struct RangeCollision {
        #[intrlay(generate_byte_range_accessors = true)]
        values: [f32; 2],
        values_byte_offset: u32,
    }

    #[derive(Intrlay)]
    struct IndexedCollision {
        #[intrlay(project = "indexed_only")]
        rows: Vec<RangeCollision>,
    }

    #[derive(Intrlay)]
    struct ComponentCollision {
        #[intrlay(generate_component_accessors = true)]
        values: [f32; 2],
        get_values_component: u32,
    }

    #[test]
    fn helper_names_collide_only_with_emitted_members() -> Result<(), Box<dyn Error>> {
        for indexed in [false, true] {
            let mut registry = Registry::new();
            if indexed {
                registry.export::<IndexedCollision>("IndexedCollision");
            } else {
                registry.export::<RangeCollision>("RangeCollision");
            }
            let mut schema = registry.finish();
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                assert!(
                    generate(&schema, language).is_err(),
                    "{language:?}, indexed={indexed}"
                );
            }
            let definition = schema
                .definitions
                .values_mut()
                .find(|value| value.name == "RangeCollision")
                .expect("collision record");
            let DefinitionKind::Record(fields) = &mut definition.kind else {
                unreachable!()
            };
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                fields[1].targets.insert(
                    language,
                    TargetOptions {
                        generate_value_accessors: Some(false),
                        ..Default::default()
                    },
                );
            }
            for language in [Language::TypeScript, Language::Python, Language::Dart] {
                generate(&schema, language)?;
            }
        }
        let mut registry = Registry::new();
        registry.export::<ComponentCollision>("ComponentCollision");
        let schema = registry.finish();
        for language in [Language::TypeScript, Language::Python, Language::Dart] {
            assert!(generate(&schema, language).is_err(), "{language:?}");
        }
        Ok(())
    }
}

//! Generated public APIs should contain only schema types and reachable views.

use super::Fixtures;

#[cfg(test)]
use intrlay::schema::DefinitionKind;
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

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false, python(name = "__ReadOnly"))]
struct ReadOnly {
    label: String,
    event: ReadOnlyEvent,
    state: ReadOnlyState,
    next: Option<Box<Self>>,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
enum ReadOnlyEvent {
    #[intrlay(tag = 3)]
    Idle,
    #[intrlay(tag = 17)]
    Data { kind: u32 },
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
enum ReadOnlyState {
    #[intrlay(tag = 2)]
    Off,
    #[intrlay(tag = 9)]
    On,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
struct WriteLeaf {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
enum WriteEvent {
    Empty,
    Data { leaf: WriteLeaf },
}

#[derive(Intrlay)]
struct WriteParent {
    leaf: Option<Box<WriteLeaf>>,
    leaves: Vec<WriteLeaf>,
    event: WriteEvent,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct ReplacementOnly {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct FieldOnly {
    label: String,
    leaf: ReplacementOnly,
}

#[derive(Intrlay)]
#[intrlay(generate_getters = false, generate_setters = false)]
struct CreateOnly {
    count: u32,
    label: String,
    fixed: [u8; 2],
    optional: Option<u32>,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_getters = false, generate_setters = false)]
struct InternalRecord {
    value: u32,
    label: String,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_getters = false, generate_setters = false)]
enum InternalChoice {
    Empty,
    Record { payload: InternalRecord },
}

#[derive(Intrlay)]
#[intrlay(generate_getters = false, generate_setters = false)]
struct OutboundPacket {
    records: Vec<InternalRecord>,
    optional: Option<Box<InternalRecord>>,
    choice: Box<InternalChoice>,
}

fn outbound_schema() -> intrlay::schema::Schema {
    let mut registry = Registry::new();
    registry.export::<OutboundPacket>("OutboundPacket");
    // An explicit alias still needs its Input, but no disabled View or View alias.
    registry.export::<InternalRecord>("InternalRecord");
    registry.export::<InternalRecord>("ZInternalRecord");
    registry.export::<InternalChoice>("InternalChoice");
    registry.export::<InternalChoice>("ZInternalChoice");
    registry.finish()
}

#[test]
fn encoding_dependencies_do_not_require_views() -> Result<(), Box<dyn std::error::Error>> {
    let schema = outbound_schema();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        for name in [
            "InternalRecordView",
            "InternalChoiceView",
            "ZInternalRecordView",
            "ZInternalChoiceView",
        ] {
            assert!(!has_identifier(&source, name), "{language:?}: {name}");
        }
        for name in [
            "OutboundPacketView",
            "InternalRecordInput",
            "InternalChoiceInput",
            "ZInternalRecordInput",
            "ZInternalChoiceInput",
        ] {
            assert!(has_identifier(&source, name), "{language:?}: {name}");
        }
        // Returning a disabled child through a parent accessor still needs its
        // view type and constructor, including through arrays and wrappers.
        let mut readable = schema.clone();
        let packet = readable
            .definitions
            .values_mut()
            .find(|definition| definition.name == "OutboundPacket")
            .ok_or("missing outbound packet")?;
        packet.targets.entry(language).or_default().generate_getters = Some(true);
        let source = generate(&readable, language)?;
        for name in [
            "InternalRecordView",
            "InternalChoiceView",
            "ZInternalRecordView",
            "ZInternalChoiceView",
        ] {
            assert!(
                has_identifier(&source, name),
                "{language:?}: borrowed {name}"
            );
        }
    }
    Ok(())
}

#[test]
fn input_names_are_validated_without_views() -> Result<(), Box<dyn std::error::Error>> {
    for (language, definition, field, invalid) in [
        (Language::TypeScript, "InternalChoice", "payload", "kind"),
        (Language::Python, "InternalChoice", "payload", "kind"),
        (Language::Python, "InternalRecord", "value", "class"),
    ] {
        let mut schema = outbound_schema();
        let definition = schema
            .definitions
            .values_mut()
            .find(|node| node.name == definition)
            .ok_or("missing input definition")?;
        let fields = match &mut definition.kind {
            DefinitionKind::Record(fields) => fields,
            DefinitionKind::Enum(variants) => {
                &mut variants
                    .iter_mut()
                    .find(|variant| variant.name == "Record")
                    .ok_or("missing payload variant")?
                    .fields
            }
        };
        fields
            .iter_mut()
            .find(|candidate| candidate.name == field)
            .ok_or("missing input field")?
            .targets
            .entry(language)
            .or_default()
            .name = Some(invalid.into());
        let error = generate(&schema, language)
            .err()
            .ok_or("missing input name error")?;
        assert!(error.to_string().contains(invalid), "{error}");
    }
    Ok(())
}

fn view_emission(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from("regressions");
    let schema = outbound_schema();
    for (language, extension) in [
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("view_emission.{extension}")),
            generate(&schema, language)?,
        )?;
    }
    fixtures.insert(
        directory.join("view_emission.bin"),
        intrlay::create::<OutboundPacket>(&OutboundPacket {
            records: vec![InternalRecord {
                value: 7,
                label: "nested".into(),
            }],
            optional: Some(Box::new(InternalRecord {
                value: 8,
                label: "optional".into(),
            })),
            choice: Box::new(InternalChoice::Record {
                payload: InternalRecord {
                    value: 9,
                    label: "variant".into(),
                },
            }),
        })?
        .as_ref(),
    )?;
    Ok(())
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_getters = false, generate_setters = false)]
struct DisabledNames {
    #[intrlay(name = "_offset")]
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
struct UnitItems {
    values: Vec<()>,
}

#[derive(Intrlay)]
struct UnitField {
    marker: (),
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
struct ZeroPayload {
    label: String,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct ZeroArray {
    values: [ZeroPayload; 0],
}

fn storage_emission(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<CreateOnly>("CreateOnly");
    registry.export::<DisabledNames>("DisabledNames");
    registry.export::<UnitItems>("UnitItems");
    registry.export::<ZeroArray>("ZeroArray");
    let schema = registry.finish();
    let directory = std::path::PathBuf::from("regressions");
    for (language, extension) in [
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        let source = generate(&schema, language)?;
        for helper in [
            "measureZeroPayload",
            "writeZeroPayload",
            "measure_zero_payload",
            "write_zero_payload",
        ] {
            assert!(!source.contains(helper), "{language:?}: unused {helper}");
        }
        fixtures.insert(
            directory.join(format!("storage_emission.{extension}")),
            source,
        )?;
        let mut disabled = Registry::new();
        disabled.export::<DisabledNames>("DisabledNames");
        fixtures.insert(
            directory.join(format!("disabled_emission.{extension}")),
            generate(&disabled.finish(), language)?,
        )?;
    }
    fixtures.insert(
        directory.join("storage_emission_created.bin"),
        intrlay::create::<CreateOnly>(&CreateOnly {
            count: 7,
            label: "created".into(),
            fixed: [2, 5],
            optional: Some(11),
        })?
        .as_ref(),
    )?;
    fixtures.insert(
        directory.join("storage_emission_units.bin"),
        intrlay::create::<UnitItems>(&UnitItems {
            values: vec![(); 3],
        })?
        .as_ref(),
    )?;
    Ok(())
}

fn input_emission_schema() -> intrlay::schema::Schema {
    let mut registry = Registry::new();
    registry.export::<ReadOnly>("ReadOnly");
    registry.export::<ReadOnly>("ZReadOnly");
    registry.export::<ReadOnlyEvent>("ReadOnlyEvent");
    registry.export::<ReadOnlyEvent>("ZReadOnlyEvent");
    registry.export::<ReadOnlyState>("ReadOnlyState");
    registry.export::<ReadOnlyState>("ZReadOnlyState");
    registry.export::<WriteParent>("WriteParent");
    registry.export::<WriteEvent>("WriteEvent");
    registry.export::<WriteEvent>("ZWriteEvent");
    registry.export::<FieldOnly>("FieldOnly");
    registry.finish()
}

#[test]
fn input_declarations_follow_writers_and_preserve_enum_constants()
-> Result<(), Box<dyn std::error::Error>> {
    let schema = input_emission_schema();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        for omitted in [
            "ReadOnlyInput",
            "ZReadOnlyInput",
            "__ReadOnlyInput",
            "ReadOnlyEventInput",
            "ReadOnlyEventIdleInput",
            "ReadOnlyEventDataInput",
            "ZReadOnlyEventInput",
            "FieldOnlyInput",
        ] {
            assert!(!has_identifier(&source, omitted), "{language:?}: {omitted}");
        }
        for retained in [
            "ReadOnlyView",
            "ZReadOnlyView",
            "ReadOnlyEventView",
            "ReadOnlyStateInput",
            "ZReadOnlyStateInput",
            "WriteLeafInput",
            "WriteParentInput",
            "WriteEventInput",
            "ZWriteEventInput",
            "ReplacementOnlyInput",
        ] {
            assert!(
                has_identifier(&source, retained),
                "{language:?}: {retained}"
            );
        }
        if language != Language::Dart {
            assert!(has_identifier(&source, "ReadOnlyEventKind"));
            assert!(has_identifier(&source, "ZReadOnlyEventKind"));
        }
    }
    Ok(())
}

fn input_emission(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from("regressions");
    let schema = input_emission_schema();
    for (language, extension) in [
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("input_emission.{extension}")),
            generate(&schema, language)?,
        )?;
    }
    fixtures.insert(
        directory.join("input_emission_read.bin"),
        intrlay::create::<ReadOnly>(&ReadOnly {
            label: "incoming".into(),
            event: ReadOnlyEvent::Data { kind: 42 },
            state: ReadOnlyState::On,
            next: None,
        })?
        .as_ref(),
    )?;
    fixtures.insert(
        directory.join("input_emission_mutation.bin"),
        intrlay::create::<FieldOnly>(&FieldOnly {
            label: "mutable field".into(),
            leaf: ReplacementOnly { value: 5 },
        })?
        .as_ref(),
    )?;
    Ok(())
}

#[test]
fn omitted_payload_inputs_do_not_reserve_declaration_names()
-> Result<(), Box<dyn std::error::Error>> {
    #[derive(Intrlay)]
    struct ReadOnlyEventData {
        count: u32,
    }

    let mut registry = Registry::new();
    registry.export::<ReadOnlyEvent>("ReadOnlyEvent");
    registry.export::<ReadOnlyEventData>("ReadOnlyEventData");
    let schema = registry.finish();
    for language in [Language::Python, Language::Dart] {
        generate(&schema, language)?;
        let mut writable = schema.clone();
        let event = writable
            .definitions
            .values_mut()
            .find(|definition| definition.name == "ReadOnlyEvent")
            .ok_or("missing event")?;
        // Isolate the declaration collision from the input's `kind` field.
        if let DefinitionKind::Enum(variants) = &mut event.kind {
            for field in variants.iter_mut().flat_map(|variant| &mut variant.fields) {
                field.targets.entry(language).or_default().name = Some("payload".into());
            }
        }
        event.targets.entry(language).or_default().create = Some(true);
        let error = generate(&writable, language)
            .err()
            .ok_or("missing collision")?;
        assert!(error.to_string().contains("ReadOnlyEventDataInput"));
    }
    Ok(())
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
    registry.export::<CreateOnly>("CreateOnly");
    registry.export::<Vec<UnitField>>("UnitFields");
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
    input_emission(fixtures)?;
    storage_emission(fixtures)?;
    view_emission(fixtures)?;
    Ok(())
}

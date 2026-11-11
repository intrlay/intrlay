//! Naming rules must distinguish API roles without renaming library APIs.

use super::Fixtures;

#[cfg(test)]
use intrlay_codegen::Symbol;

use intrlay::{
    Intrlay,
    schema::{Language, NameCase, Registry},
};
use intrlay_codegen::{
    GenerateOptions, Modifier, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector,
    Visibility, generate_with_options,
};

#[derive(Intrlay)]
struct NamingSample {
    sample_rate: u32,
    #[intrlay(csharp(name = "literalName"), typescript(name = "literalName"))]
    untouched: Option<u32>,
}

#[derive(Intrlay)]
struct PayloadRecord {
    request_id: u32,
}

#[derive(Intrlay)]
enum PayloadEvent {
    Waiting,
    TaskDone {
        request_id: u32,
        #[intrlay(generate_setters = false)]
        saved_id: u32,
    },
    #[intrlay(
        typescript(name = "Delivered"),
        python(name = "Delivered"),
        dart(name = "Delivered"),
        cpp(name = "Delivered"),
        csharp(name = "Delivered")
    )]
    Renamed {
        #[intrlay(
            typescript(name = "Ticket"),
            python(name = "Ticket"),
            dart(name = "Ticket"),
            cpp(name = "Ticket"),
            csharp(name = "Ticket")
        )]
        request_id: u32,
    },
    Flattened {
        #[intrlay(project = "flatten")]
        payload: PayloadRecord,
    },
    #[intrlay(generate_getters = false)]
    SetOnly {
        request_id: u32,
    },
}

fn payload_property_names(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<PayloadEvent>("PayloadEvent");
    let schema = registry.finish();
    for (language, extension) in [
        (Language::Cpp, "hpp"),
        (Language::CSharp, "cs"),
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        let mut policy = NamingPolicy::for_language(language);
        for (kind, prefix, readonly) in [
            (SymbolKind::Field, "input_", None),
            (SymbolKind::Variant, "Variant", None),
            (SymbolKind::Property, "read_", None),
            (SymbolKind::Property, "write_", Some(false)),
        ] {
            policy.rules.push(NamingRule {
                selector: SymbolSelector {
                    kind: Some(kind),
                    visibility: Some(Visibility::Public),
                    exclude: if readonly == Some(false) {
                        vec![Modifier::Readonly]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                style: NameStyle {
                    case: if kind == SymbolKind::Variant {
                        NameCase::PascalCase
                    } else {
                        NameCase::SnakeCase
                    },
                    prefix: prefix.into(),
                    suffix: String::new(),
                },
            });
        }
        let source = generate_with_options(
            &schema,
            language,
            GenerateOptions {
                module_name: Some("PayloadNames".into()),
                naming: Some(policy),
                ..Default::default()
            },
        )?;
        let writable_property = matches!(language, Language::TypeScript | Language::Python);
        let prefix = if writable_property { "write" } else { "read" };
        assert!(
            source.contains(&format!("{prefix}_task_done_request_id")),
            "{language:?}"
        );
        assert!(source.contains("read_task_done_saved_id"), "{language:?}");
        assert!(
            source.contains(&format!("{prefix}_delivered_ticket")),
            "{language:?}"
        );
        assert!(
            !source.contains("task_done_input_request_id"),
            "{language:?}"
        );
        assert!(
            !source.contains("task_done_read_request_id"),
            "{language:?}"
        );
        if !language.has_value_types() {
            assert!(
                source.contains(&format!("{prefix}_flattened_payload_request_id")),
                "{language:?}"
            );
        }
        fixtures.insert(format!("naming/enum_properties.{extension}"), source)?;
    }
    Ok(())
}

fn naming(language: Language) -> NamingPolicy {
    let mut policy = NamingPolicy::for_language(language);
    policy.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Method),
                visibility: Some(Visibility::Public),
                require: vec![Modifier::Static],
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::PascalCase,
                prefix: "Static_".into(),
                suffix: String::new(),
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Method),
                visibility: Some(Visibility::Public),
                exclude: vec![Modifier::Static],
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::SnakeCase,
                prefix: String::new(),
                suffix: "_instance".into(),
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                visibility: Some(Visibility::Private),
                require: vec![Modifier::Readonly],
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::PascalCase,
                prefix: "_".into(),
                suffix: String::new(),
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                require: vec![Modifier::Static, Modifier::Const],
                ..Default::default()
            },
            style: NameStyle::new(NameCase::ScreamingSnakeCase),
        },
    ]);
    policy
}

#[test]
fn modifiers_and_visibility_select_independent_styles() {
    let policy = naming(Language::TypeScript);
    let base = Symbol {
        name: "compute_byte_length",
        kind: SymbolKind::Method,
        visibility: Some(Visibility::Public),
        modifiers: &[Modifier::Static],
    };
    assert_eq!(policy.resolve(base), "Static_ComputeByteLength");
    assert_eq!(
        policy.resolve(Symbol {
            modifiers: &[],
            ..base
        }),
        "compute_byte_length_instance"
    );
    assert_eq!(
        policy.resolve(Symbol {
            kind: SymbolKind::Field,
            name: "world_buffer",
            visibility: Some(Visibility::Private),
            modifiers: &[Modifier::Readonly]
        }),
        "_WorldBuffer"
    );
    assert_eq!(
        policy.resolve(Symbol {
            kind: SymbolKind::Field,
            name: "byte_size",
            modifiers: &[Modifier::Static, Modifier::Const],
            ..base
        }),
        "BYTE_SIZE"
    );
    // A local or module constant does not become a type member just by being const.
    assert_eq!(
        policy.resolve(Symbol {
            kind: SymbolKind::Field,
            name: "byte_size",
            modifiers: &[Modifier::Const],
            ..base
        }),
        "byteSize"
    );
}

fn generated_members_keep_explicit_names_and_standard_library_members(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<NamingSample>("NamingSample");
    let schema = registry.finish();
    for language in [Language::CSharp, Language::TypeScript] {
        let source = generate_with_options(
            &schema,
            language,
            GenerateOptions {
                naming: Some(naming(language)),
                ..Default::default()
            },
        )?;
        assert!(!source.contains("$["));
        assert!(source.contains("Static_CreateInPlace"));
        assert!(source.contains("BYTE_SIZE"));
        assert!(source.contains("_Offset"));
        assert!(!source.contains("reserve_instance"));
        assert!(source.contains("literalName"));
        if language == Language::CSharp {
            assert!(source.contains("global::Intrlay.GenerateBindings"));
            assert!(source.contains("global::Intrlay.WireKind.Optional"));
            assert!(!source.contains("BinaryPrimitives.ReadUInt32LittleEndian"));
        }
        let directory = std::path::PathBuf::from("naming");

        fixtures.insert(
            directory.join(if language == Language::CSharp {
                "bindings.cs"
            } else {
                "bindings.ts"
            }),
            source,
        )?;
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generated_members_keep_explicit_names_and_standard_library_members(fixtures)?;
    payload_property_names(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn payload_property_names() -> Result<(), Box<dyn std::error::Error>> {
        super::payload_property_names(&mut super::Fixtures::default())
    }

    #[test]
    fn generated_members_keep_explicit_names_and_standard_library_members()
    -> Result<(), Box<dyn std::error::Error>> {
        super::generated_members_keep_explicit_names_and_standard_library_members(
            &mut super::Fixtures::default(),
        )
    }
}

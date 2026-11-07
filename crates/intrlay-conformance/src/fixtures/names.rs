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
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn generated_members_keep_explicit_names_and_standard_library_members()
    -> Result<(), Box<dyn std::error::Error>> {
        super::generated_members_keep_explicit_names_and_standard_library_members(
            &mut super::Fixtures::default(),
        )
    }
}

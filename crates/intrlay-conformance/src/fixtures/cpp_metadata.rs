//! Native C++ mappings retain field identity and distinguish whole alternatives.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{DefinitionKind, Language, NameCase, Registry, Type},
};
use intrlay_codegen::{
    GenerateOptions, Modifier, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector,
    Visibility, generate_with_options,
};

#[derive(Intrlay)]
struct Child {
    value: u32,
    marker: u32,
}

#[derive(Intrlay)]
struct Peer {
    value: u32,
    marker: u32,
}

#[derive(Intrlay)]
#[intrlay(cpp(name = "V"))]
struct SourceV {
    #[intrlay(cpp(name = "renamed"))]
    native_value: u32,
}

#[derive(Intrlay)]
struct Keyword {
    #[intrlay(cpp(name = "kind"))]
    r#class: u32,
}

#[derive(Intrlay)]
struct Text {
    value: String,
}

#[derive(Intrlay)]
struct Names {
    #[intrlay(cpp(name = "Child"))]
    first: Child,
    second: Child,
    #[intrlay(cpp(name = "Factory"))]
    factory: u32,
    #[intrlay(cpp(name = "ElementAccess"))]
    element_access: u32,
    source: SourceV,
}

#[derive(Intrlay)]
enum OrdinaryChoice {
    Value {
        value: Option<u32>,
        #[intrlay(cpp(name = "renamed_extra"))]
        native_extra: u32,
    },
    Empty,
}

#[derive(Intrlay)]
enum Mode {
    #[intrlay(tag = 7)]
    On,
    #[intrlay(tag = 9)]
    Off,
}

#[derive(Intrlay)]
enum WholeChoice {
    Empty { payload: () },
    Value { payload: Child },
    Nested { payload: [Option<u32>; 2] },
}

fn generate_cpp_metadata_regressions(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("cpp-metadata");

    let mut registry = Registry::new();
    registry.export::<Names>("Names");
    registry.export::<Keyword>("Keyword");
    registry.export::<OrdinaryChoice>("OrdinaryChoice");
    registry.export::<WholeChoice>("WholeChoice");
    registry.export::<Mode>("Mode");
    registry.export::<Text>("Text");
    let mut schema = registry.finish();
    let Type::Named(identity) = &schema.exports["WholeChoice"] else {
        return Err("named variant expected".into());
    };
    let DefinitionKind::Enum(variants) = &mut schema
        .definitions
        .get_mut(identity)
        .ok_or("variant definition missing")?
        .kind
    else {
        return Err("variant definition expected".into());
    };
    // The C++ frontend marks these wrappers explicitly. A different field name
    // ensures native construction cannot accidentally rely on the name `value`.
    for variant in variants {
        variant.synthetic_payload = true;
    }
    fixtures.insert(
        directory.join("bindings.hpp"),
        generate_with_options(
            &schema,
            Language::Cpp,
            GenerateOptions {
                cpp_namespace: Some("cpp_metadata".into()),
                ..GenerateOptions::default()
            },
        )?,
    )?;
    // Exercise whole-alternative views in every target, without the unrelated
    // C++ naming overrides from the native-metadata fixtures above.
    let mut registry = Registry::new();
    registry.export::<WholeChoice>("WholeChoice");
    let mut alternatives = registry.finish();
    for definition in alternatives.definitions.values_mut() {
        if let DefinitionKind::Enum(variants) = &mut definition.kind {
            for variant in variants {
                variant.synthetic_payload = true;
            }
        }
    }
    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::TypeScript, "ts"),
        (Language::CSharp, "cs"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("alternatives.{extension}")),
            generate_with_options(&alternatives, language, GenerateOptions::default())?,
        )?;
    }
    fixtures.insert(
        directory.join("names.bin"),
        NamesView::create(&Names {
            first: Child {
                value: 3,
                marker: 5,
            },
            second: Child {
                value: 7,
                marker: 11,
            },
            factory: 13,
            element_access: 17,
            source: SourceV { native_value: 19 },
        })?
        .as_bytes(),
    )?;
    fixtures.insert(
        directory.join("ordinary.bin"),
        OrdinaryChoiceView::create(&OrdinaryChoice::Value {
            value: Some(23),
            native_extra: 29,
        })?
        .as_bytes(),
    )?;
    fixtures.insert(
        directory.join("whole.bin"),
        WholeChoiceView::create(&WholeChoice::Value {
            payload: Child {
                value: 31,
                marker: 37,
            },
        })?
        .as_bytes(),
    )?;
    fixtures.insert(
        directory.join("nested.bin"),
        WholeChoiceView::create(&WholeChoice::Nested {
            payload: [Some(41), None],
        })?
        .as_bytes(),
    )?;
    Ok(())
}

fn generate_cpp_metadata_peer(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    // Independent headers in one namespace may reuse native field spellings.
    let mut registry = Registry::new();
    registry.export::<Peer>("Peer");
    fixtures.insert(
        "cpp-metadata/peer.hpp",
        generate_with_options(
            &registry.finish(),
            Language::Cpp,
            GenerateOptions {
                cpp_namespace: Some("cpp_metadata".into()),
                ..GenerateOptions::default()
            },
        )?,
    )?;
    Ok(())
}

fn generate_cpp_custom_metadata_names(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut naming = NamingPolicy::for_language(Language::Cpp);
    for (kind, visibility, prefix, case, require) in [
        (
            SymbolKind::Type,
            Visibility::Public,
            "Api",
            NameCase::PascalCase,
            vec![],
        ),
        (
            SymbolKind::Field,
            Visibility::Private,
            "cell_",
            NameCase::PascalCase,
            vec![],
        ),
        (
            SymbolKind::Field,
            Visibility::Public,
            "native_",
            NameCase::SnakeCase,
            vec![],
        ),
        (
            SymbolKind::Property,
            Visibility::Public,
            "Read",
            NameCase::PascalCase,
            vec![],
        ),
        (
            SymbolKind::Method,
            Visibility::Public,
            "Build",
            NameCase::PascalCase,
            vec![Modifier::Static],
        ),
    ] {
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(kind),
                visibility: Some(visibility),
                require,
                ..SymbolSelector::default()
            },
            style: NameStyle {
                case,
                prefix: prefix.into(),
                suffix: String::new(),
            },
        });
    }
    let mut registry = Registry::new();
    registry.export::<Child>("Child");
    let directory = std::path::PathBuf::from("cpp-metadata");

    fixtures.insert(
        directory.join("custom.hpp"),
        generate_with_options(
            &registry.finish(),
            Language::Cpp,
            GenerateOptions {
                cpp_namespace: Some("cpp_metadata::custom".into()),
                naming: Some(naming),
                ..GenerateOptions::default()
            },
        )?,
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_cpp_metadata_regressions(fixtures)?;
    generate_cpp_metadata_peer(fixtures)?;
    generate_cpp_custom_metadata_names(fixtures)?;
    Ok(())
}

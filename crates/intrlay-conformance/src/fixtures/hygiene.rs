//! Generated code must distinguish user type names from implementation imports.

use super::Fixtures;

#[cfg(test)]
use intrlay::schema::TargetOptions;
#[cfg(test)]
use intrlay_codegen::generate;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

#[derive(Intrlay)]
#[intrlay(rust(name = "Result"), csharp(name = "BinaryPrimitives"))]
struct ShadowResult {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "Some"), csharp(name = "Encoding"))]
struct ShadowSome {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "None"), csharp(name = "ValueTuple"))]
struct ShadowNone {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "Ok"), csharp(name = "ArgumentException"))]
struct ShadowOk {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "Err"), csharp(name = "System"))]
struct ShadowErr {
    value: u32,
}

#[derive(Intrlay)]
struct Hygiene {
    result: ShadowResult,
    some: ShadowSome,
    none: ShadowNone,
    ok: ShadowOk,
    err: ShadowErr,
    unit: (),
    text: String,
    fixed: [u32; 2],
    selected: Option<ShadowResult>,
}

fn generate_hygiene_cases(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("edge-cases");

    let mut registry = Registry::new();
    registry.export::<Hygiene>("Hygiene");
    let schema = registry.finish();
    for (language, extension) in [(Language::Rust, "rs"), (Language::CSharp, "cs")] {
        fixtures.insert(
            directory.join(format!("hygiene.{extension}")),
            generate_with_options(
                &schema,
                language,
                GenerateOptions {
                    csharp: intrlay_codegen::CSharpOptions {
                        namespace: Some("EdgeCases.Hygiene".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )?,
        )?;
    }
    fixtures.insert(
        directory.join("hygiene.bin"),
        HygieneView::create(&Hygiene {
            result: ShadowResult { value: 1 },
            some: ShadowSome { value: 2 },
            none: ShadowNone { value: 3 },
            ok: ShadowOk { value: 4 },
            err: ShadowErr { value: 5 },
            unit: (),
            text: "standard names".into(),
            fixed: [6, 7],
            selected: Some(ShadowResult { value: 8 }),
        })?
        .as_bytes(),
    )?;
    Ok(())
}

#[test]
fn imported_implementation_names_are_rejected() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<ShadowResult>("ShadowResult");
    let schema = registry.finish();
    for (language, names) in [
        (Language::Cpp, &["detail"][..]),
        (Language::TypeScript, &["intrlay", "Some", "globalThis"][..]),
        (
            Language::Python,
            &["_intrlay", "Some", "_abc", "_typing", "_builtins", "super"][..],
        ),
        (
            Language::Dart,
            &["intrlay", "Some", "core", "typed", "convert"][..],
        ),
    ] {
        for name in names {
            let mut schema = schema.clone();
            for definition in schema.definitions.values_mut() {
                definition.targets.insert(
                    language,
                    TargetOptions {
                        name: Some((*name).into()),
                        ..Default::default()
                    },
                );
            }
            let error = generate(&schema, language)
                .err()
                .ok_or("expected an implementation name collision")?;
            assert!(
                error
                    .to_string()
                    .contains("generated module infrastructure"),
                "{language:?}: {error}"
            );
        }
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_hygiene_cases(fixtures)?;
    Ok(())
}

//! Explicit generation units keep names stable and isolate native declarations.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

#[derive(Intrlay)]
struct Shared {
    value: u32,
}

#[derive(Intrlay)]
struct SharedAlpha {
    shared: Shared,
}

#[derive(Intrlay)]
struct SharedBeta {
    shared: Shared,
}

#[derive(Intrlay)]
struct Frame {
    fixed: [f32; 3],
    values: Vec<Shared>,
}

#[test]
fn module_and_helper_names_do_not_depend_on_exports() -> Result<(), Box<dyn Error>> {
    for extra_export in [false, true] {
        let mut registry = Registry::new();
        registry.export::<Frame>("Frame");
        if extra_export {
            registry.export::<SharedBeta>("AnotherExport");
        }
        let schema = registry.finish();
        let options = GenerateOptions {
            module_name: Some("SensorProtocol".into()),
            ..Default::default()
        };
        let csharp = generate_with_options(&schema, Language::CSharp, options.clone())?;
        assert!(csharp.contains("namespace IntrlayGenerated.SensorProtocol;"));
        assert!(csharp.contains("ArrayName = \"FixedArray3\""));
        assert!(csharp.contains("\"SequenceOfSharedView\""));
        assert!(csharp.contains("internal static partial class Bindings"));
        assert!(!csharp.contains("FrameAnotherExport"));
        let cpp = generate_with_options(&schema, Language::Cpp, options.clone())?;
        assert!(cpp.contains("namespace intrlay_generated::sensor_protocol {"));
        for (language, namespace, marker) in [
            (Language::Cpp, "app::sensors", "namespace app::sensors {"),
            (Language::CSharp, "App.Sensors", "namespace App.Sensors;"),
        ] {
            let mut options = options.clone();
            if language == Language::Cpp {
                options.cpp_namespace = Some(namespace.into());
            } else {
                options.csharp_namespace = Some(namespace.into());
            }
            assert!(generate_with_options(&schema, language, options)?.contains(marker));
        }
    }
    Ok(())
}

fn cpp_modules_keep_shared_names_in_separate_scopes(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("regressions");

    let mut alpha = Registry::new();
    alpha.export::<SharedAlpha>("SharedAlpha");
    let mut beta = Registry::new();
    beta.export::<SharedBeta>("SharedBeta");
    for (file, registry) in [("shared-alpha", alpha), ("shared-beta", beta)] {
        let source = generate_with_options(
            &registry.finish(),
            Language::Cpp,
            GenerateOptions {
                module_name: Some(file.replace('-', "_")),
                ..Default::default()
            },
        )?;
        assert!(!source.contains("using schema_"));
        fixtures.insert(directory.join(format!("{file}.hpp")), source)?;
    }

    let mut registry = Registry::new();
    registry.export::<Shared>("Shared");
    let schema = registry.finish();
    for (file, namespace) in [
        ("custom-alpha", "fixture::left"),
        ("custom-beta", "fixture::right"),
    ] {
        let source = generate_with_options(
            &schema,
            Language::Cpp,
            GenerateOptions {
                cpp_namespace: Some(namespace.into()),
                ..Default::default()
            },
        )?;
        assert!(source.contains(&format!("namespace {namespace} {{")));
        fixtures.insert(directory.join(format!("{file}.hpp")), source)?;
    }
    Ok(())
}

#[test]
fn cpp_namespace_must_be_a_qualified_identifier() {
    let mut registry = Registry::new();
    registry.export::<Shared>("Shared");
    let schema = registry.finish();
    for namespace in [
        "",
        "::",
        "a::::b",
        "a::class",
        "a::1",
        "a;b",
        "a::INTRLAY_RESTRICT",
    ] {
        let result = generate_with_options(
            &schema,
            Language::Cpp,
            GenerateOptions {
                cpp_namespace: Some(namespace.into()),
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(error) if error.0.contains("invalid C++ namespace")));
    }
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    cpp_modules_keep_shared_names_in_separate_scopes(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn cpp_modules_keep_shared_names_in_separate_scopes() -> Result<(), Box<dyn std::error::Error>>
    {
        super::cpp_modules_keep_shared_names_in_separate_scopes(&mut super::Fixtures::default())
    }
}

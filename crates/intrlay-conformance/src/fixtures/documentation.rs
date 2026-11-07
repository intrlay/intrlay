//! Source descriptions survive layout planning and target-language formatting.

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{DefinitionKind, Language, Registry},
};
use intrlay_codegen::generate;

/// A documented record.
///
///     Preserve this indentation.
///
/// Unicode: 한글. Special text: <T> & """ and */ and `$[storage]`.
#[derive(Intrlay)]
struct Documented {
    /// Position along the X axis, in metres.
    x: f32,
    /// A write-only value.
    #[intrlay(generate_getters = false)]
    output: u32,
}

/// A documented event.
#[derive(Intrlay)]
enum Event {
    /// No measurement is available.
    Empty,
    /// A new measurement is available.
    Value {
        /// Strength of the measurement.
        strength: f32,
    },
}

/** A block-documented container.
 *
 *     Preserve the example indentation.
 */
#[derive(Intrlay)]
struct Container {
    /// Indexed records.
    records: Vec<Documented>,
    /// Current event.
    event: Event,
}

fn documentation_reaches_native_input_types_and_views(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<Container>("Container");
    let schema = registry.finish();
    let record = schema
        .definitions
        .values()
        .find(|definition| definition.name == "Documented")
        .ok_or("record")?;
    assert!(
        record
            .docs
            .contains("\n\n    Preserve this indentation.\n\n")
    );
    let container = schema
        .definitions
        .values()
        .find(|definition| definition.name == "Container")
        .ok_or("container")?;
    assert!(
        container
            .docs
            .contains("\n\n    Preserve the example indentation.")
    );
    let event = schema
        .definitions
        .values()
        .find(|definition| definition.name == "Event")
        .ok_or("event")?;
    let DefinitionKind::Enum(variants) = &event.kind else {
        return Err("enum expected".into());
    };
    assert_eq!(variants[1].docs, "A new measurement is available.");
    assert_eq!(variants[1].fields[0].docs, "Strength of the measurement.");

    let directory = std::path::PathBuf::from("documentation");

    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::Cpp, "hpp"),
        (Language::TypeScript, "ts"),
        (Language::CSharp, "cs"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        let source = generate(&schema, language)?;
        assert!(
            source.contains("including nested views, must not overlap the bytes being written")
        );
        assert!(source.contains("A documented record."));
        assert!(source.contains("A new measurement is available."));
        assert!(source.contains("No measurement is available."));
        assert!(source.contains("Strength of the measurement."));
        assert!(
            source.contains("$[storage]"),
            "source docs are not template symbols"
        );
        if !matches!(language, Language::Rust | Language::CSharp) {
            assert!(
                source
                    .matches("Position along the X axis, in metres.")
                    .count()
                    >= 2
            );
            assert!(source.matches("A write-only value.").count() >= 2);
        }
        match language {
            Language::TypeScript => {
                assert!(source.contains(
                    "/**\n * A documented record.\n *\n *     Preserve this indentation."
                ));
            }
            Language::CSharp => assert!(source.contains("&lt;T&gt; &amp;")),
            Language::Python => {
                assert!(source.contains("\"\"\"Position along the X axis, in metres.\"\"\""));
            }
            _ => assert!(source.contains("/// Position along the X axis, in metres.")),
        }
        fixtures.insert(directory.join(format!("bindings.{extension}")), source)?;
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    documentation_reaches_native_input_types_and_views(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn documentation_reaches_native_input_types_and_views() -> Result<(), Box<dyn std::error::Error>>
    {
        super::documentation_reaches_native_input_types_and_views(&mut super::Fixtures::default())
    }
}

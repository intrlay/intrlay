//! Source descriptions survive layout planning and target-language formatting.

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{DefinitionKind, Language, Registry, TargetOptions},
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
    /// An output value.
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

/// Coordinates in world space, measured in metres.
#[derive(Intrlay)]
#[repr(C, align(16))]
struct Coordinates {
    /// Horizontal coordinate.
    x: f32,
    /// Vertical coordinate.
    y: f32,
    /// Depth coordinate.
    z: f32,
}

/// The meaningful region of the scene.
#[derive(Intrlay)]
struct Region {
    /// Minimum corner in world space.
    #[intrlay(name = "lower")]
    min: Coordinates,
    /// Maximum corner in world space.
    max: [f32; 3],
}

/// A sampled object with one position.
#[derive(Intrlay)]
struct Sample {
    /// Measured position of this object.
    #[intrlay(project = "f32", generate_component_accessors = true)]
    position: Coordinates,
}

/// Availability of a measurement.
#[derive(Intrlay)]
enum SampleStatus {
    /// The measurement is unavailable.
    Absent,
    /// A confidence value accompanies the measurement.
    Available {
        /// Confidence from zero to one.
        confidence: f32,
    },
}

/// Descriptions survive changes to the exposed representation.
#[derive(Intrlay)]
struct ProjectedDocumentation {
    /// Region used to frame the scene.
    #[intrlay(project = "flatten", name = "bounds")]
    region: Region,
    /// Positions packed with their alignment lanes.
    #[intrlay(project = "f32")]
    packed: Vec<Coordinates>,
    /// Samples read without constructing element views.
    #[intrlay(project = "indexed_only")]
    samples: Vec<Sample>,
    /// Statuses read without constructing variant views.
    #[intrlay(project = "indexed_only")]
    statuses: Vec<SampleStatus>,
}

fn projected_documentation_is_independent_of_input_emission(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    registry.export::<ProjectedDocumentation>("Packet");
    let original = registry.finish();
    for (language, extension) in [
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        for readers_only in [false, true] {
            let mut schema = original.clone();
            if readers_only {
                for definition in schema.definitions.values_mut() {
                    let options = definition
                        .targets
                        .entry(language)
                        .or_insert_with(TargetOptions::default);
                    options.create = Some(false);
                    options.generate_setters = Some(false);
                }
            }
            let source = generate(&schema, language)?;
            for text in [
                "The meaningful region of the scene.",
                "Region used to frame the scene.",
                "Minimum corner in world space.",
                "Maximum corner in world space.",
                "Coordinates in world space, measured in metres.",
                "Horizontal coordinate.",
                "Vertical coordinate.",
                "Depth coordinate.",
                "A sampled object with one position.",
                "Measured position of this object.",
                "Availability of a measurement.",
                "A confidence value accompanies the measurement.",
                "The measurement is unavailable.",
                "Confidence from zero to one.",
                "Source: ProjectedDocumentation.region.min.x.",
                "Source: ProjectedDocumentation.region.max[0].",
                "Source: Sample.position.",
                "Source: SampleStatus.Available.confidence.",
                "element stride 4 lanes (16 bytes)",
                "Padding: lanes [3, 4).",
            ] {
                assert!(
                    source.contains(text),
                    "{language:?}, readers_only={readers_only}: missing {text}"
                );
            }
            assert!(
                !source.contains("Source: ProjectedDocumentation.bounds.lower"),
                "API renames must not change provenance"
            );
            if readers_only {
                assert!(!source.contains("class SampleView"));
                assert!(!source.contains("class SampleInput"));
                assert!(!source.contains("interface SampleInput"));
                assert_eq!(
                    source.matches("Maximum corner in world space.").count(),
                    1,
                    "array context belongs to the group, not to each accessor"
                );
            }
            let mode = if readers_only { "read-only" } else { "inputs" };
            fixtures.insert(
                format!("documentation/projections-{mode}.{extension}"),
                source,
            )?;
        }
    }
    Ok(())
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
            assert!(source.matches("An output value.").count() >= 2);
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
    projected_documentation_is_independent_of_input_emission(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn projection_preserves_context_without_inputs() -> Result<(), Box<dyn std::error::Error>> {
        super::projected_documentation_is_independent_of_input_emission(
            &mut super::Fixtures::default(),
        )
    }

    #[test]
    fn numeric_documentation_describes_repetition_without_expansion()
    -> Result<(), Box<dyn std::error::Error>> {
        #[derive(intrlay::Intrlay)]
        struct Repeated {
            #[intrlay(project = "f32")]
            values: [[super::Coordinates; 1024]; 1024],
        }
        let mut registry = intrlay::schema::Registry::new();
        registry.export::<Repeated>("Repeated");
        let schema = registry.finish();
        for language in [
            super::Language::TypeScript,
            super::Language::Python,
            super::Language::Dart,
        ] {
            let source = super::generate(&schema, language)?;
            assert!(source.contains("1024 elements; element stride"));
            assert!(source.contains("Horizontal coordinate."));
            assert!(
                source.len() < 30_000,
                "documentation must scale with types, not element count"
            );
        }
        Ok(())
    }

    #[test]
    fn documentation_reaches_native_input_types_and_views() -> Result<(), Box<dyn std::error::Error>>
    {
        super::documentation_reaches_native_input_types_and_views(&mut super::Fixtures::default())
    }
}

//! C# construction consumes the common proof rather than repeating layout analysis.

use super::Fixtures;

use std::{error::Error, path::Path};

use intrlay::{Intrlay, create, schema::Registry};
use intrlay_codegen::{GenerateOptions, generate_with_options};
use intrlay_schema::Language;

#[derive(Intrlay)]
#[repr(C)]
struct Packed {
    id: u32,
    value: f32,
}

#[derive(Intrlay)]
struct Frame {
    sequence: u64,
    values: Vec<Packed>,
}

#[derive(Intrlay)]
struct Padded {
    small: u8,
    wide: u64,
}

#[derive(Intrlay)]
struct Gapped {
    data: Vec<u8>,
    words: Vec<u64>,
}

#[derive(Intrlay)]
struct OptionalValue {
    value: Option<u32>,
}

#[derive(Intrlay)]
enum Variant {
    Empty,
    Word(u64),
}

#[derive(Intrlay)]
struct Dynamic {
    values: Vec<Vec<u32>>,
}

#[derive(Intrlay)]
struct Tree {
    children: Vec<Self>,
}

fn fixture<C: Intrlay>(
    fixtures: &mut Fixtures,
    directory: &Path,
    name: &str,
    value: &C::Value,
) -> Result<(), Box<dyn Error>> {
    fixtures.insert(
        directory.join(format!("{name}.bin")),
        create::<C>(value)?.as_ref(),
    )?;
    Ok(())
}

fn generate_initialization_cases(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Frame>("Frame");
    registry.export::<Padded>("Padded");
    registry.export::<Gapped>("Gapped");
    registry.export::<OptionalValue>("OptionalValue");
    registry.export::<Variant>("Variant");
    registry.export::<Dynamic>("Dynamic");
    registry.export::<Tree>("Tree");
    let code = generate_with_options(
        &registry.finish(),
        Language::CSharp,
        GenerateOptions {
            csharp_namespace: Some("Initialization.Generated".into()),
            ..Default::default()
        },
    )?;
    for (name, expected) in [
        ("Frame", true),
        ("Padded", false),
        ("Gapped", false),
        ("OptionalValue", false),
        ("Variant", false),
        ("Dynamic", true),
        ("Tree", true),
    ] {
        let declaration = code
            .split("[global::Intrlay.WireType(")
            .skip(1)
            .filter_map(|attribute| attribute.split_once(")]").map(|(value, _)| value))
            .find(|attribute| attribute.contains(&format!("\"{name}\"")))
            .ok_or_else(|| format!("missing metadata for {name}"))?;
        assert_eq!(
            declaration.contains("FullyWritten = true"),
            expected,
            "{name}"
        );
    }
    let directory = std::path::PathBuf::from("initialization");

    fixtures.insert(directory.join("bindings.cs"), code)?;
    reference_bytes(fixtures, &directory)
}

fn reference_bytes(fixtures: &mut Fixtures, directory: &Path) -> Result<(), Box<dyn Error>> {
    fixture::<Frame>(
        fixtures,
        directory,
        "frame",
        &Frame {
            sequence: 17,
            values: vec![Packed { id: 3, value: -0.0 }, Packed { id: 5, value: 1.25 }],
        },
    )?;
    fixture::<Frame>(
        fixtures,
        directory,
        "empty",
        &Frame {
            sequence: 19,
            values: vec![],
        },
    )?;
    fixture::<Padded>(
        fixtures,
        directory,
        "padded",
        &Padded {
            small: 7,
            wide: u64::MAX,
        },
    )?;
    fixture::<OptionalValue>(
        fixtures,
        directory,
        "absent",
        &OptionalValue { value: None },
    )?;
    fixture::<OptionalValue>(
        fixtures,
        directory,
        "present",
        &OptionalValue { value: Some(23) },
    )?;
    fixture::<Variant>(fixtures, directory, "variant-empty", &Variant::Empty)?;
    fixture::<Variant>(
        fixtures,
        directory,
        "variant-word",
        &Variant::Word(u64::MAX),
    )?;
    fixture::<Dynamic>(
        fixtures,
        directory,
        "dynamic",
        &Dynamic {
            values: vec![vec![1, 2], vec![], vec![3]],
        },
    )?;
    fixture::<Tree>(
        fixtures,
        directory,
        "tree",
        &Tree {
            children: vec![Tree { children: vec![] }],
        },
    )?;
    for length in 0..17 {
        fixture::<Gapped>(
            fixtures,
            directory,
            &format!("gap-{length}"),
            &Gapped {
                data: vec![0x37; length],
                words: vec![u64::MAX, 29],
            },
        )?;
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_initialization_cases(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn generate_initialization_cases() -> Result<(), Box<dyn std::error::Error>> {
        super::generate_initialization_cases(&mut super::Fixtures::default())
    }
}

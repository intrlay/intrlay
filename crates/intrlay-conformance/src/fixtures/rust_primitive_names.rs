//! Explicit Rust declaration names cannot change the meaning of primitive fields.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

#[derive(Intrlay)]
#[intrlay(rust(name = "u32"))]
struct Word {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "usize"))]
struct Offset {
    value: u64,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "bool"))]
enum Flag {
    Value { value: bool },
    Empty,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "u8"))]
struct Byte {
    value: u8,
}

#[derive(Intrlay)]
struct Bundle {
    words: Vec<Word>,
    location: Offset,
    flags: [Flag; 2],
    byte: Option<Byte>,
}

fn generate_primitive_named_types_and_export_aliases(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Bundle>("Bundle");
    registry.export::<Word>("Word");
    registry.export::<Offset>("Offset");
    registry.export::<Flag>("Flag");
    registry.export::<Byte>("Byte");
    let output = generate(&registry.finish(), Language::Rust)?;
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(directory.join("primitive-names.rs"), output)?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_primitive_named_types_and_export_aliases(fixtures)?;
    Ok(())
}

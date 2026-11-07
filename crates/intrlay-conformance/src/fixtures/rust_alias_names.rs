//! Container view parameters must not capture generated source declarations.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

#[derive(Intrlay)]
struct B {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(rust(name = "B_"))]
struct Second {
    value: u32,
}

fn generate_aliases_with_fresh_buffer_parameters(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Vec<B>>("Batch");
    registry.export::<Option<B>>("Maybe");
    registry.export::<Box<B>>("Owned");
    registry.export::<[B; 2]>("Pair");
    registry.export::<Vec<Option<Box<B>>>>("Nested");
    registry.export::<Second>("Second");
    let output = generate(&registry.finish(), Language::Rust)?;
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(directory.join("alias-names.rs"), output)?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_aliases_with_fresh_buffer_parameters(fixtures)?;
    Ok(())
}

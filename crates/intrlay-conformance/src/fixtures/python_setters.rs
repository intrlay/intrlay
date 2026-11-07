//! Transparent scalar optimizations honor nested accessor emission hints.

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;
use std::error::Error;

#[derive(Intrlay)]
#[intrlay(transparent)]
struct Identifier(#[intrlay(generate_setters = false)] u32);

#[derive(Intrlay)]
#[intrlay(transparent, generate_setters = false)]
struct Constant(u32);

#[derive(Intrlay)]
#[intrlay(transparent)]
struct Editable(u32);

#[derive(Intrlay)]
struct Wrapper {
    identifier: Identifier,
    constant: Constant,
    editable: Editable,
}

fn scalar_descriptors_follow_the_nested_mutation_plan(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Wrapper>("Wrapper");
    let source = generate(&registry.finish(), Language::Python)?;
    assert!(source.contains("editable = _intrlay.u32_field("));
    assert!(!source.contains("identifier = _intrlay.u32_field("));
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(directory.join("scalar_setters.py"), source)?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    scalar_descriptors_follow_the_nested_mutation_plan(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn scalar_descriptors_follow_the_nested_mutation_plan() -> Result<(), Box<dyn std::error::Error>>
    {
        super::scalar_descriptors_follow_the_nested_mutation_plan(&mut super::Fixtures::default())
    }
}

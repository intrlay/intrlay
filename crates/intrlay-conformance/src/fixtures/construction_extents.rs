//! Dynamic construction checks the written extent without changing fixed factories.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

#[derive(Intrlay)]
#[repr(align(16))]
struct Empty {}

#[derive(Intrlay)]
struct Extent {
    value: Option<Box<Empty>>,
}

#[derive(Intrlay)]
struct Counted {
    values: Vec<u32>,
}

fn dynamic_factories_check_the_final_extent(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("construction-extents");

    let mut registry = Registry::new();
    registry.export::<Extent>("Extent");
    registry.export::<Counted>("Counted");
    let schema = registry.finish();
    for (language, extension, check) in [
        (Language::Python, "py", "check_extent=True"),
        (
            Language::Dart,
            "dart",
            "cursor.finish(storage.bytes.length)",
        ),
    ] {
        let source = generate(&schema, language)?;
        assert!(source.contains(check));
        fixtures.insert(directory.join(format!("bindings.{extension}")), source)?;
        let mut fixed = Registry::new();
        fixed.export::<Empty>("Empty");
        assert!(!generate(&fixed.finish(), language)?.contains(check));
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    dynamic_factories_check_the_final_extent(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn dynamic_factories_check_the_final_extent() -> Result<(), Box<dyn std::error::Error>> {
        super::dynamic_factories_check_the_final_extent(&mut super::Fixtures::default())
    }
}

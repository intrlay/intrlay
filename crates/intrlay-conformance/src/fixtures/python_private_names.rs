//! Explicit Python private names retain their meaning inside generated classes.

use super::Fixtures;

use intrlay::schema::NameCase;
use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{
    GenerateOptions, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility,
    generate, generate_with_options,
};
use std::error::Error;

#[derive(Intrlay)]
#[intrlay(python(name = "__Child"))]
struct Child {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(python(name = "__Choice"))]
enum Choice {
    Empty,
    Present {
        child: Child,
    },
    Private {
        #[intrlay(python(name = "__number"))]
        number: u32,
    },
}

#[derive(Intrlay)]
#[intrlay(python(name = "__Status"))]
enum Status {
    Idle = 1,
    Ready = 3,
}

#[derive(Intrlay)]
#[intrlay(python(name = "__Root"))]
struct Root {
    child: Child,
    children: Vec<Child>,
    optional: Option<Child>,
    boxed: Box<Child>,
    choice: Choice,
    text: String,
    status: Status,
    #[intrlay(project = "u32")]
    numeric_status: Status,
    #[intrlay(project = "u32")]
    statuses: Vec<Status>,
}

#[derive(Intrlay)]
struct PrivateField {
    #[intrlay(python(name = "__value"))]
    value: u32,
}

fn private_names_work_in_factories_and_child_accessors(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Root>("Root");
    registry.export::<PrivateField>("PrivateField");
    let schema = registry.finish();
    let source = generate(&schema, Language::Python)?;
    assert!(source.contains("_intrlay__ChildView = __ChildView"));
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(directory.join("private_names.py"), source)?;
    let mut naming = NamingPolicy::for_language(Language::Python);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Function),
            visibility: Some(Visibility::Internal),
            ..Default::default()
        },
        style: NameStyle {
            case: NameCase::SnakeCase,
            prefix: "__".into(),
            suffix: String::new(),
        },
    });
    let source = generate_with_options(
        &schema,
        Language::Python,
        GenerateOptions {
            naming: Some(naming),
            ..Default::default()
        },
    )?;
    assert!(source.contains("_intrlay__measure_root = __measure_root"));
    fixtures.insert(directory.join("private_helpers.py"), source)?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    private_names_work_in_factories_and_child_accessors(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn private_names_work_in_factories_and_child_accessors()
    -> Result<(), Box<dyn std::error::Error>> {
        super::private_names_work_in_factories_and_child_accessors(&mut super::Fixtures::default())
    }

    #[test]
    fn private_enum_aliases_still_reject_real_name_collisions()
    -> Result<(), Box<dyn std::error::Error>> {
        #[derive(intrlay::Intrlay)]
        #[intrlay(python(name = "_intrlay__Status"))]
        enum Collision {
            Value,
        }

        let mut registry = super::Registry::new();
        registry.export::<super::Root>("Root");
        registry.export::<Collision>("Collision");
        let error = super::generate(&registry.finish(), super::Language::Python)
            .err()
            .ok_or("private reference alias shadowed a distinct enum")?;
        assert!(
            error.to_string().contains("_intrlay__Status conflicts"),
            "{error}"
        );
        Ok(())
    }
}

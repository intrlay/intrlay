//! Fixed-value replacement preserves neighboring data and shared view semantics.
#![allow(
    clippy::option_option,
    reason = "All three nested optional states are part of the wire API"
)]

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};
use std::error::Error;

#[derive(Debug, PartialEq, Intrlay)]
struct Leaf {
    number: u32,
    scalar: Option<u32>,
}

#[derive(Debug, PartialEq, Intrlay)]
struct EmptyPayloads {
    empty: [String; 0],
    nested: [[Vec<u64>; 0]; 2],
    number: u32,
}

#[derive(Debug, PartialEq, Intrlay)]
enum Choice {
    Empty,
    Child { leaf: Leaf },
    Numbers { values: [u32; 3] },
    EmptyArrays { data: EmptyPayloads },
}

#[derive(Debug, PartialEq, Intrlay)]
struct Fixed {
    maybe: Option<Leaf>,
    nested: Option<Option<Leaf>>,
    choice: Choice,
}

#[derive(Debug, PartialEq, Intrlay)]
struct Message {
    before: u32,
    fixed: Fixed,
    items: Vec<Option<Leaf>>,
    text: String,
    after: u32,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct NoFactory {
    value: Option<u32>,
}

#[test]
fn replace_values_containing_empty_payload_arrays() -> Result<(), Box<dyn Error>> {
    let initial = EmptyPayloads {
        empty: [],
        nested: [[], []],
        number: 1,
    };
    let mut view = EmptyPayloadsView::create(&initial)?;
    assert_eq!(view.as_bytes(), 1_u32.to_le_bytes());
    let replacement = EmptyPayloads {
        number: 2,
        ..initial
    };
    view.replace(&replacement)?;
    assert_eq!(view.as_bytes(), 2_u32.to_le_bytes());
    assert_eq!(EmptyPayloads::from(view), replacement);

    let mut choice = ChoiceView::create(&Choice::Empty)?;
    choice.replace(&Choice::EmptyArrays { data: replacement })?;
    assert!(matches!(
        choice.variant(),
        ChoiceVariantView::EmptyArrays { data } if data.number() == 2
    ));
    Ok(())
}

#[test]
fn replace_fixed_presence_and_variants() -> Result<(), Box<dyn Error>> {
    let source = Message {
        before: 123,
        fixed: Fixed {
            maybe: None,
            nested: Some(None),
            choice: Choice::Empty,
        },
        items: vec![None],
        text: "preserved".into(),
        after: 456,
    };
    let mut view = MessageView::create(&source)?;
    view.fixed_mut().replace_maybe(&Some(Leaf {
        number: 7,
        scalar: Some(8),
    }))?;
    view.fixed_mut().replace_nested(&Some(Some(Leaf {
        number: 9,
        scalar: None,
    })))?;
    view.fixed_mut().choice_mut().replace(&Choice::Child {
        leaf: Leaf {
            number: 10,
            scalar: Some(11),
        },
    })?;
    assert_eq!(view.fixed().maybe().ok_or("missing leaf")?.number(), 7);
    assert_eq!(
        view.fixed()
            .nested()
            .flatten()
            .ok_or("missing nested leaf")?
            .number(),
        9
    );
    view.fixed_mut().choice_mut().replace(&Choice::Numbers {
        values: [12, 13, 14],
    })?;
    assert!(!view.fixed_mut().choice_mut().replace_child_leaf(&Leaf {
        number: 0,
        scalar: None,
    })?);
    assert!(
        view.fixed_mut()
            .choice_mut()
            .replace_numbers_values(&[21, 22, 23])?
    );
    view.fixed_mut().replace_maybe(&None)?;
    view.items_mut().set(
        0,
        &Some(Leaf {
            number: 15,
            scalar: None,
        }),
    )?;
    assert_eq!(
        view.items()
            .get(0)
            .flatten()
            .ok_or("missing item")?
            .number(),
        15
    );
    view.items_mut().set(0, &None)?;
    let decoded = Message::from(view);
    assert_eq!(decoded.before, 123);
    assert_eq!(decoded.after, 456);
    assert_eq!(decoded.text, source.text);
    assert_eq!(decoded.items, source.items);
    assert_eq!(decoded.fixed.maybe, None);
    assert_eq!(
        decoded.fixed.choice,
        Choice::Numbers {
            values: [21, 22, 23]
        }
    );
    Ok(())
}

fn generate_replacement_consumers(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Message>("Message");
    registry.export::<EmptyPayloads>("EmptyPayloads");
    registry.export::<Option<u32>>("MaybeScalar");
    registry.export::<Option<Option<Leaf>>>("Nested");
    registry.export::<Box<Option<Leaf>>>("BoxedLeaf");
    registry.export::<NoFactory>("NoFactory");
    let schema = registry.finish();
    let path = std::path::PathBuf::from("replacement");
    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::Cpp, "hpp"),
        (Language::CSharp, "cs"),
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        let source = generate_with_options(
            &schema,
            language,
            GenerateOptions {
                module_name: Some("Replacement".into()),
                ..Default::default()
            },
        )?;
        fixtures.insert(path.join(format!("bindings.{extension}")), source)?;
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_replacement_consumers(fixtures)?;
    Ok(())
}

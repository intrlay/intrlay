#[path = "../../../target/cpp-metadata/alternatives.rs"]
mod alternatives;
#[path = "../../../target/regressions/cpp-source.rs"]
mod cpp_source;
#[path = "../../../target/regressions/bindings.rs"]
mod generated;

#[path = "../../../target/regressions/contracts.rs"]
mod contracts;

#[path = "../../../target/regressions/primitive-names.rs"]
mod primitive_names;

#[path = "../../../target/regressions/rust-derive-names.rs"]
mod derive_names;

#[path = "../../../target/regressions/rust-enum-names.rs"]
mod enum_names;

#[path = "../../../target/replacement/bindings.rs"]
mod replacement;

use intrlay::Intrlay;

mod alias_names;

fn main() {
    let mut whole = alternatives::WholeChoiceView::create(&alternatives::WholeChoice::Value(
        alternatives::Child {
            value: 31,
            marker: 37,
        },
    ))
    .unwrap();
    assert_eq!(
        whole.as_bytes(),
        std::fs::read("target/cpp-metadata/whole.bin").unwrap()
    );
    match whole.variant() {
        alternatives::WholeChoiceVariantView::Value(child) => assert_eq!(child.value(), 31),
        _ => panic!("expected complete alternative payload"),
    }
    whole
        .replace_value_field_0(&alternatives::Child {
            value: 41,
            marker: 43,
        })
        .unwrap();
    match whole.variant() {
        alternatives::WholeChoiceVariantView::Value(child) => assert_eq!(child.marker(), 43),
        _ => panic!("replacement must retain the variant"),
    }
    whole
        .replace(&alternatives::WholeChoice::Nested([Some(41), None]))
        .unwrap();
    assert_eq!(
        whole.as_bytes(),
        std::fs::read("target/cpp-metadata/nested.bin").unwrap()
    );
    #[cfg(feature = "forbidden-replace")]
    {
        // This enum's header is fixed, but switching to Text would need payload storage.
        #[derive(Intrlay)]
        enum Dynamic {
            Empty,
            Text(String),
        }
        let mut view = DynamicView::create(&Dynamic::Empty).unwrap();
        view.replace(&Dynamic::Text("cannot append storage".into()))
            .unwrap();
    }
    let mut fixed = replacement::FixedView::create(&replacement::Fixed {
        maybe: None,
        nested: Some(None),
        choice: replacement::Choice::Empty {},
    })
    .unwrap();
    fixed
        .replace_maybe(&Some(replacement::Leaf {
            number: 7,
            scalar: Some(8),
        }))
        .unwrap();
    assert_eq!(fixed.maybe().unwrap().number(), 7);
    fixed
        .choice_mut()
        .replace(&replacement::Choice::Numbers { values: [1, 2, 3] })
        .unwrap();
    fixed.replace_maybe(&None).unwrap();
    assert!(fixed.maybe().is_none());
    let mut empty = replacement::EmptyPayloadsView::create(&replacement::EmptyPayloads {
        empty: [],
        nested: [[], []],
        number: 1,
    })
    .unwrap();
    let value = replacement::EmptyPayloads {
        empty: [],
        nested: [[], []],
        number: 2,
    };
    empty.replace(&value).unwrap();
    assert_eq!(empty.as_bytes(), 2_u32.to_le_bytes());
    assert_eq!(empty.empty().len(), 0);
    assert_eq!(empty.nested().len(), 2);
    fixed
        .choice_mut()
        .replace(&replacement::Choice::EmptyArrays { data: value })
        .unwrap();
    assert!(
        matches!(fixed.choice().variant(), replacement::ChoiceVariantView::EmptyArrays { data } if data.number() == 2)
    );
    let renamed =
        derive_names::RootDataView::create(&derive_names::RootData { create: 23 }).unwrap();
    assert_eq!(renamed.get_create(), 23);
    let mut choice = enum_names::ChoiceView::create(&enum_names::Choice::A { b_get_c: 7 }).unwrap();
    assert!(choice.replace_a_get_b_get_c(&11).unwrap());
    choice.a_get_b_get_c_mut().unwrap().set(13);
    assert_eq!(
        enum_names::Choice::from(choice),
        enum_names::Choice::A { b_get_c: 13 }
    );
    alias_names::run();
    {
        use primitive_names::{Bundle, BundleView, Byte, Flag, Offset, Word};
        let source = Bundle {
            words: vec![Word { value: 11 }],
            location: Offset { value: 13 },
            flags: [Flag::Value { value: true }, Flag::Empty {}],
            byte: Some(Byte { value: 17 }),
        };
        let mut view = BundleView::create(&source).unwrap();
        assert_eq!(view.words().get(0).unwrap().value(), 11);
        assert!(matches!(
            view.flags().get(0).unwrap().variant(),
            primitive_names::boolVariantView::Value { value: true }
        ));
        assert_eq!(view.byte().unwrap().value(), 17);
        view.location_mut().value_mut().set(19);
        let decoded = Bundle::from(view);
        assert_eq!(decoded.location.value, 19);
        assert_eq!(decoded.words, source.words);
        assert_eq!(decoded.flags, source.flags);
    }
    let bytes = include_bytes!("../../../target/regressions/contracts.bin");
    let view = unsafe { contracts::ContractsView::from_bytes_unchecked(bytes.as_slice()) };
    assert_eq!(view.identifier().field_0(), 9);
    assert_eq!(view.identifiers().get(1).unwrap().field_0(), 11);
    assert_eq!(contracts::Identifier::LAYOUT.size(), 16);
    assert_eq!(generated::Aligned::LAYOUT.size(), 16);
    assert_eq!(generated::Aligned::LAYOUT.alignment(), 16);
    let value = generated::Aligned::Value { payload: 7 };
    let encoded = generated::AlignedView::create(&value).unwrap();
    assert_eq!(generated::Aligned::from(encoded), value);
    assert_eq!(cpp_source::Point::LAYOUT.size(), 16);
    assert_eq!(cpp_source::Point::LAYOUT.alignment(), 16);
    let mut scalar = generated::ScalarView::create(&1.5).unwrap();
    assert_eq!(scalar.value(), 1.5);
    scalar.value_mut().set(2.5);
    assert_eq!(scalar.value(), 2.5);
    let value = generated::Restricted::Value {
        visible: 3,
        hidden: 4,
    };
    let mut restricted = generated::RestrictedView::create(&value).unwrap();
    assert!(matches!(
        restricted.variant(),
        generated::RestrictedVariantView::Value {
            visible: 3,
            hidden: 4
        }
    ));
    restricted.value_hidden_mut().unwrap().set(9);
    assert_eq!(
        generated::Restricted::from(restricted),
        generated::Restricted::Value {
            visible: 3,
            hidden: 9
        }
    );

    #[cfg(feature = "forbidden-create")]
    let _ = generated::ReadOnlyView::create(&generated::ReadOnly { value: 7 });

    {
        let view = generated::RestrictedView::create(&generated::Restricted::Empty {}).unwrap();
        let _ = view.variant();
    }

    {
        let mut bytes = [0u8; 4];
        let mut view = unsafe { <generated::ReadOnly as Intrlay>::view_mut(&mut bytes, 0) };
        view.value_mut().set(7);
    }
    println!("Rust regenerated layouts, accessor generation, and scalar entry points passed");
}

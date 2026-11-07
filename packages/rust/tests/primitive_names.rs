//! Generic source names must not shadow primitive types used by derived codecs.
#![allow(
    non_camel_case_types,
    clippy::builtin_type_shadow,
    reason = "exercise legal primitive-like type parameter names"
)]

macro_rules! primitive_parameter {
    ($name:ident) => {
        mod $name {
            use intrlay::Intrlay;

            #[derive(Debug, PartialEq, Intrlay)]
            struct Record<$name> {
                value: $name,
                bytes: [::core::primitive::u8; 2],
            }

            #[derive(Debug, PartialEq, Intrlay)]
            enum Choice<$name> {
                #[intrlay(tag = 7)]
                Value {
                    value: $name,
                },
                Empty,
            }

            #[test]
            fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
                let source = Record {
                    value: 11_u64,
                    bytes: [13, 17],
                };
                let mut view = RecordView::create(&source)?;
                view.value_mut().set(19);
                let view = view.as_view();
                assert_eq!(view.bytes().get(1), Some(17));
                assert_eq!(
                    Record::from(view),
                    Record {
                        value: 19,
                        bytes: [13, 17]
                    }
                );
                let mut choice = ChoiceView::create(&Choice::Value { value: 23_u64 })?;
                choice
                    .value_value_mut()
                    .ok_or("active variant expected")?
                    .set(29);
                assert!(matches!(
                    choice.variant(),
                    ChoiceVariantView::Value { value: 29 }
                ));
                assert_eq!(Choice::from(choice), Choice::Value { value: 29 });
                let empty = ChoiceView::create(&Choice::<::core::primitive::u64>::Empty)?;
                assert_eq!(Choice::from(empty), Choice::Empty);
                Ok(())
            }
        }
    };
}

primitive_parameter!(bool);
primitive_parameter!(u8);
primitive_parameter!(u16);
primitive_parameter!(u32);
primitive_parameter!(u64);
primitive_parameter!(usize);
primitive_parameter!(i8);
primitive_parameter!(i16);
primitive_parameter!(i32);
primitive_parameter!(i64);
primitive_parameter!(isize);
primitive_parameter!(f32);
primitive_parameter!(f64);

//! Legal generic names must not be interpreted as generated local bindings.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    reason = "exercise legal const parameter names"
)]

macro_rules! collision_case {
    ($name:ident) => {
        mod $name {
            use intrlay::{Intrlay, schema::Registry};

            #[derive(Debug, PartialEq, Intrlay)]
            struct Record<const $name: usize> {
                value: [u32; $name],
                cursor: Vec<u8>,
            }

            #[derive(Debug, PartialEq, Intrlay)]
            #[repr(C)]
            struct Tuple<const $name: usize>([u32; $name]);

            #[derive(Debug, PartialEq, Intrlay)]
            enum Choice<const $name: usize> {
                #[intrlay(tag = if $name == 0 { 0 } else { 1 })]
                Value { value: [u32; $name] },
                Empty,
            }

            #[test]
            fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
                let source = Record::<2> { value: [3, 5], cursor: vec![7, 11] };
                let layout = RecordView::compute_layout(&source)?;
                let mut storage = intrlay::OwnedBuffer::new(layout)?;
                let mut view = RecordView::create_in_place(&source, storage.as_mut())?;
                view.value_mut().get_mut(0).ok_or("first value")?.set(13);
                assert_eq!(view.as_view().value().get(0), Some(13));
                assert_eq!(view.cursor().get(1), Some(11));
                let decoded: Record<2> = view.into();
                assert_eq!(decoded.value, [13, 5]);

                let tuple = Tuple::<2>([17, 19]);
                assert!(<Tuple<2> as Intrlay>::NATIVE_LAYOUT);
                let tuple_view = TupleView::create(&tuple)?;
                assert_eq!(Tuple::from(tuple_view), tuple);

                let mut choice = ChoiceView::create(&Choice::<2>::Value { value: [23, 29] })?;
                choice.value_value_mut().ok_or("active variant")?.get_mut(1).ok_or("second value")?.set(31);
                assert!(matches!(choice.variant(), ChoiceVariantView::Value { value } if value.get(1) == Some(31)));
                assert_eq!(Choice::from(choice), Choice::Value { value: [23, 31] });
                let empty = ChoiceView::create(&Choice::<2>::Empty)?;
                assert_eq!(Choice::from(empty), Choice::Empty);

                let mut registry = Registry::new();
                registry.export::<Record<2>>("Record");
                registry.export::<Choice<2>>("Choice");
                assert_eq!(registry.finish().exports.len(), 2);
                Ok(())
            }
        }
    };
}

collision_case!(value);
collision_case!(value_);
collision_case!(cursor);
collision_case!(writer);
collision_case!(offset);
collision_case!(bytes);
collision_case!(registry);
collision_case!(buffer);
collision_case!(view);
collision_case!(fields);
collision_case!(tag);
collision_case!(tags);
collision_case!(i);
collision_case!(j);
collision_case!(__tag);
collision_case!(__field_0);
collision_case!(__IntrlayBuffer);

mod raw_names {
    collision_case!(r#value);
    collision_case!(r#cursor);
    collision_case!(r#__field_0);
    collision_case!(r#__IntrlayBuffer);
}

#[derive(intrlay::Intrlay)]
struct BufferType<__IntrlayBuffer, const value: usize, const value_: usize> {
    item: __IntrlayBuffer,
    first: [u8; value],
    second: [u8; value_],
}

#[test]
fn fresh_names_skip_all_source_parameters() -> Result<(), intrlay::EncodeError> {
    let source = BufferType {
        item: 37u32,
        first: [1; 2],
        second: [2; 3],
    };
    let view = BufferTypeView::create(&source)?;
    assert_eq!(view.item(), 37);
    let decoded = BufferType::from(view);
    assert_eq!(decoded.first, [1; 2]);
    assert_eq!(decoded.second, [2; 3]);
    Ok(())
}

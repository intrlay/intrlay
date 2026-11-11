//! Borrowed construction preserves owned encodings, including nested payloads.

#![deny(type_alias_bounds)]

use std::{cell::Cell, rc::Rc, sync::Arc};

use intrlay::{
    BoxInput, EncodeError, EncodeSource, Intrlay, SequenceInput, Zeroed, ZeroedSequence,
};

#[test]
fn zeroed_inputs_fill_nested_arrays_and_reused_storage() -> Result<(), EncodeError> {
    let items = [ItemInput {
        id: 7_u32,
        matrices: ZeroedSequence::new(3),
    }];
    let input = BatchInput {
        items: items.as_slice(),
        label: "zeroed",
    };
    let expected = intrlay::create::<Batch>(&Batch {
        items: vec![Item {
            id: 7,
            matrices: vec![[0.0; 16]; 3],
        }],
        label: "zeroed".into(),
    })?;
    assert_eq!(
        intrlay::create_from::<Batch>(&input)?.as_ref(),
        expected.as_ref()
    );
    let capacity = intrlay::SizeCursor::new(intrlay::layout::Layout::new(
        (expected.as_ref().len() + 16).next_multiple_of(expected.layout().alignment()),
        expected.layout().alignment(),
    ))?
    .finish();
    let mut reused = intrlay::OwnedBuffer::new(capacity)?;
    reused.as_mut().fill(0xa5);
    intrlay::create_in_place_from::<Batch>(&input, reused.as_mut())?;
    assert_eq!(
        &reused.as_ref()[..expected.as_ref().len()],
        expected.as_ref()
    );
    assert!(
        reused.as_ref()[expected.as_ref().len()..]
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    assert_eq!(
        intrlay::create_from::<[[f32; 4]; 4]>(&Zeroed)?.as_ref(),
        &[0; 64]
    );
    assert!(
        intrlay::create_from::<[String; 0]>(&Zeroed)?
            .as_ref()
            .is_empty()
    );
    Ok(())
}

#[derive(Intrlay)]
struct Batch {
    items: Vec<Item>,
    label: String,
}

#[derive(Intrlay)]
struct Item {
    id: u32,
    matrices: Vec<[f32; 16]>,
}

#[test]
fn fixed_headers_keep_padding_and_surrounding_payload_cursors()
-> Result<(), Box<dyn std::error::Error>> {
    #[derive(Clone, Copy, Debug, PartialEq, Intrlay)]
    struct Row {
        enabled: bool,
        id: u64,
        direction: [f32; 3],
    }
    #[derive(Debug, PartialEq, Intrlay)]
    struct Envelope {
        prefix: String,
        rows: Vec<Row>,
        #[expect(
            clippy::box_collection,
            reason = "exercise nested payload reservations after fixed headers"
        )]
        suffix: Box<String>,
    }
    let rows = [
        Row {
            enabled: true,
            id: 17,
            direction: [1.0, 2.0, 3.0],
        },
        Row {
            enabled: false,
            id: u64::MAX,
            direction: [4.0, 5.0, 6.0],
        },
    ];
    let expected = Envelope {
        prefix: "prefix".into(),
        rows: rows.to_vec(),
        suffix: Box::new("suffix".into()),
    };
    let input = EnvelopeInput {
        prefix: "prefix",
        rows: SequenceInput::new(rows.iter().map(|row| RowInput {
            enabled: &row.enabled,
            id: &row.id,
            direction: &row.direction,
        })),
        suffix: BoxInput("suffix"),
    };
    let owned = intrlay::create::<Envelope>(&expected)?;
    let borrowed = intrlay::create_from::<Envelope>(&input)?;
    assert_eq!(owned.as_ref(), borrowed.as_ref());
    assert_eq!(Envelope::decode(borrowed.as_ref(), 0)?, expected);
    let mut reused = intrlay::OwnedBuffer::new(intrlay::compute_layout::<Vec<u64>>(&vec![
        0;
        owned
            .as_ref()
            .len(
            )
    ])?)?;
    reused.as_mut().fill(0xa5);
    let written = intrlay::write_from::<Envelope>(&input, reused.as_mut())?;
    assert_eq!(&reused.as_ref()[..written.byte_length()], owned.as_ref());
    assert!(
        reused.as_ref()[written.byte_length()..]
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    Ok(())
}

#[derive(Intrlay)]
struct Nested<T> {
    payload: Option<Box<T>>,
    values: [u16; 3],
}

#[test]
fn borrowed_generic_inputs_do_not_require_owned_schema_copy() {
    #[derive(Intrlay)]
    struct Single<T> {
        value: T,
    }
    fn requires_copy<T: Copy>(_: T) {}
    // String is not Copy; its borrowed encoding source is.
    let input = SingleInput::<String, _>::new("borrowed");
    requires_copy(input);
    assert!(format!("{input:?}").contains("borrowed"));
}

#[derive(Intrlay)]
enum Message {
    Empty,
    Data { label: String, items: Vec<Item> },
}

trait CodeMarker {}

#[derive(Intrlay)]
enum GenericCode<const TAG: u32 = 7>
where
    Self: CodeMarker,
{
    #[intrlay(tag = TAG)]
    Start,
    #[intrlay(tag = 99)]
    End,
}

impl<const TAG: u32> CodeMarker for GenericCode<TAG> {}

#[derive(Intrlay)]
enum NamedCode
where
    NamedCode: CodeMarker,
{
    Ready = 37,
}

impl CodeMarker for NamedCode {}

#[test]
fn payload_free_input_alias_preserves_defaults_and_resolves_schema_bounds()
-> Result<(), EncodeError> {
    let first: GenericCodeInput = GenericCode::Start;
    let second: GenericCodeInput<11> = GenericCode::Start;
    assert_eq!(
        intrlay::create_from::<GenericCode<7>>(&first)?.as_ref(),
        7_u32.to_le_bytes()
    );
    assert_eq!(
        intrlay::create_from::<GenericCode<11>>(&second)?.as_ref(),
        11_u32.to_le_bytes()
    );
    let named: NamedCodeInput = NamedCode::Ready;
    assert_eq!(
        intrlay::create_from::<NamedCode>(&named)?.as_ref(),
        37_u32.to_le_bytes()
    );
    Ok(())
}

#[test]
#[deny(warnings)]
fn local_input_impls_do_not_shadow_schema_generics() -> Result<(), EncodeError> {
    // The second parameter also occupies the preferred internal alias name.
    #[derive(Intrlay)]
    struct Packet<r#PacketInput, __IntrlayInput> {
        value: PacketInput,
        marker: __IntrlayInput,
    }

    #[derive(Intrlay)]
    enum Message<MessageInput> {
        Data { value: MessageInput },
        Empty,
    }

    let numbers = [3_u32, 5, 7];
    let owned = Packet {
        value: numbers.to_vec(),
        marker: 11_u8,
    };
    let input = PacketInput::new(numbers.as_slice(), 11_u8);
    assert_eq!(
        PacketView::<_, Vec<u32>, u8>::create_from(&input)?.as_bytes(),
        PacketView::<_, Vec<u32>, u8>::create(&owned)?.as_bytes(),
    );

    let input = MessageInput::Data {
        value: numbers.as_slice(),
    };
    let owned = Message::Data {
        value: numbers.to_vec(),
    };
    assert_eq!(
        MessageView::<_, Vec<u32>>::create_from(&input)?.as_bytes(),
        MessageView::<_, Vec<u32>>::create(&owned)?.as_bytes(),
    );
    assert_eq!(
        MessageView::<_, Vec<u32>>::create_from(&MessageInput::<Vec<u32>, &[u32]>::Empty)?
            .as_bytes(),
        MessageView::<_, Vec<u32>>::create(&Message::Empty)?.as_bytes(),
    );
    Ok(())
}

#[test]
#[deny(warnings)]
fn renamed_inputs_keep_schema_generic_and_const_arguments() -> Result<(), EncodeError> {
    #[derive(Intrlay)]
    #[intrlay(rust(name = "Wire"))]
    struct Packet<WireInput, const N: usize>([WireInput; N]);

    let numbers = [13_u32, 17, 19];
    let input = WireInput::new(&numbers);
    assert_eq!(
        WireView::<_, u32, 3>::create_from(&input)?.as_bytes(),
        WireView::<_, u32, 3>::create(&Packet(numbers))?.as_bytes(),
    );
    Ok(())
}

#[test]
#[deny(warnings)]
fn input_bindings_preserve_traits_named_only_in_attribute_bounds() -> Result<(), EncodeError> {
    #[allow(
        non_camel_case_types,
        reason = "Exercises generated type-parameter hygiene"
    )]
    trait __IntrlayField0: Intrlay<Value = Self> + Sized {}
    impl __IntrlayField0 for Vec<u32> {}

    #[allow(
        non_camel_case_types,
        reason = "Exercises generated type-alias hygiene"
    )]
    trait __IntrlayInput: Intrlay<Value = Self> + Sized {}
    impl __IntrlayInput for Vec<u32> {}

    #[derive(Intrlay)]
    #[intrlay(bound = "T: __IntrlayField0")]
    struct Packet<T> {
        value: T,
    }

    #[derive(Intrlay)]
    #[intrlay(bound = "T: __IntrlayField0")]
    enum Message<T> {
        Data { value: T },
    }

    #[derive(Intrlay)]
    #[intrlay(bound = "AliasInput: __IntrlayInput")]
    struct Alias<AliasInput> {
        value: AliasInput,
    }

    let numbers = [13_u32, 17, 19];
    assert_eq!(
        PacketView::<_, Vec<u32>>::create_from(&PacketInput::new(numbers.as_slice()))?.as_bytes(),
        PacketView::create(&Packet {
            value: numbers.to_vec()
        })?
        .as_bytes(),
    );
    assert_eq!(
        MessageView::<_, Vec<u32>>::create_from(&MessageInput::Data {
            value: numbers.as_slice(),
        })?
        .as_bytes(),
        MessageView::create(&Message::Data {
            value: numbers.to_vec()
        })?
        .as_bytes(),
    );
    assert_eq!(
        AliasView::<_, Vec<u32>>::create_from(&AliasInput::new(numbers.as_slice()))?.as_bytes(),
        AliasView::create(&Alias {
            value: numbers.to_vec()
        })?
        .as_bytes(),
    );
    Ok(())
}

#[test]
fn nested_iterator_and_arc_slices_match_owned_encoding() -> Result<(), EncodeError> {
    let matrices: Arc<[[f32; 16]]> = Arc::from([[1.0; 16], [2.0; 16]]);
    let label = String::from("borrowed 🦀");
    let ids = [3, 7];
    let input = BatchInput {
        items: SequenceInput::new(ids.iter().map(|id| ItemInput {
            id: *id,
            matrices: matrices.as_ref(),
        })),
        label: label.as_str(),
    };
    let owned = Batch {
        items: ids
            .iter()
            .map(|id| Item {
                id: *id,
                matrices: matrices.to_vec(),
            })
            .collect(),
        label: label.clone(),
    };
    let actual = BatchView::create_from(&input)?;
    assert_eq!(actual.as_bytes(), BatchView::create(&owned)?.as_bytes());
    assert_eq!(actual.items().get(1).map(|item| item.id()), Some(7));
    assert_eq!(actual.label(), label);
    assert_eq!(Arc::strong_count(&matrices), 1);

    let layout = BatchView::compute_layout_from(&input)?;
    let mut storage = intrlay::OwnedBuffer::new(layout)?;
    let written = BatchView::create_in_place_from(&input, storage.as_mut())?;
    assert_eq!(written.as_bytes(), actual.as_bytes());
    Ok(())
}

#[test]
fn enum_option_and_box_inputs_borrow_recursively() -> Result<(), EncodeError> {
    let matrices = [[5.0; 16]];
    let items = [ItemInput {
        id: 5,
        matrices: matrices.as_slice(),
    }];
    let input = NestedInput::new(
        Some(BoxInput(MessageInput::Data {
            label: "data",
            items: items.as_slice(),
        })),
        [1, 2, 3],
    );
    let owned = Nested {
        payload: Some(Box::new(Message::Data {
            label: "data".into(),
            items: vec![Item {
                id: 5,
                matrices: matrices.to_vec(),
            }],
        })),
        values: [1, 2, 3],
    };
    assert_eq!(
        intrlay::create_from::<Nested<Message>>(&input)?.as_ref(),
        intrlay::create::<Nested<Message>>(&owned)?.as_ref()
    );
    Ok(())
}

#[test]
fn explicit_numeric_conversion_keeps_borrowed_sequence_inputs() -> Result<(), EncodeError> {
    let values = [1_u16, 256, u16::MAX];
    let input = SequenceInput::new(values.iter().map(|value| u32::from(*value)));
    let owned = vec![1_u32, 256, u32::from(u16::MAX)];
    assert_eq!(
        intrlay::create_from::<Vec<u32>>(&input)?.as_ref(),
        intrlay::create::<Vec<u32>>(&owned)?.as_ref(),
    );

    let value = 1.25_f32;
    assert_eq!(
        intrlay::create_from::<f32>(&&value)?.as_ref(),
        value.to_le_bytes(),
    );
    Ok(())
}

#[test]
fn fixed_array_sources_keep_native_bulk_copy_proofs() -> Result<(), EncodeError> {
    const {
        assert!(<[f32; 16] as EncodeSource<[f32; 16]>>::NATIVE_LAYOUT);
    }
    const {
        assert!(<[f32; 16] as EncodeSource<[f32; 16]>>::WRITES_FIXED_LAYOUT);
    }
    const {
        assert!(!<&[f32; 16] as EncodeSource<[f32; 16]>>::NATIVE_LAYOUT);
    }
    let matrices = [[f32::from_bits(0x7fc0_0042); 16], [-0.0; 16]];
    let borrowed = intrlay::create_from::<Vec<[f32; 16]>>(matrices.as_slice())?;
    let owned = intrlay::create::<Vec<[f32; 16]>>(&matrices.to_vec())?;
    assert_eq!(borrowed.as_ref(), owned.as_ref());
    Ok(())
}

#[derive(Clone)]
struct ReportedCount {
    remaining: usize,
    reported: usize,
}

impl Iterator for ReportedCount {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some(1)
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.reported, Some(self.reported))
    }
}
impl ExactSizeIterator for ReportedCount {}

#[test]
fn incorrect_safe_iterator_counts_cannot_publish_uninitialized_bytes() {
    for (reported, remaining) in [(2, 1), (1, 2)] {
        let input = SequenceInput::new(ReportedCount {
            remaining,
            reported,
        });
        assert!(matches!(
            intrlay::create_from::<Vec<u32>>(&input),
            Err(EncodeError::SequenceLengthChanged { .. })
        ));
    }
}

#[test]
fn fixed_element_sequences_measure_without_visiting_elements() -> Result<(), EncodeError> {
    let visits = Rc::new(Cell::new(0));
    let input = SequenceInput::new((0..8_u32).inspect(|_| visits.set(visits.get() + 1)));
    let encoded = intrlay::create_from::<Vec<u32>>(&input)?;
    assert_eq!(visits.get(), 8);
    // SAFETY: create_from constructed a complete Vec<u32> encoding.
    let values = unsafe { <Vec<u32> as Intrlay>::decode_unchecked(encoded.as_ref(), 0) };
    assert_eq!(values, (0..8).collect::<Vec<_>>());
    Ok(())
}

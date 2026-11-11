//! Checked decoding and one-time view validation, including hostile encodings.

use std::sync::atomic::{AtomicUsize, Ordering};

use intrlay::{DecodeError, Decoder, Intrlay, Validator, ValueView};

#[test]
fn owned_views_return_the_original_storage() -> Result<(), Box<dyn std::error::Error>> {
    fn recover<B: intrlay::OwnedBytes + intrlay::StableBytes>(buffer: B) -> Result<B, DecodeError> {
        let view = ValueView::<u32, _>::from_bytes(buffer)?;
        assert_eq!(view.value(), 7);
        Ok(view.into_buffer())
    }

    let bytes = 7_u32.to_le_bytes();
    assert_eq!(recover(bytes)?, bytes);
    let vector = bytes.to_vec();
    let address = vector.as_ptr();
    let vector = recover(vector)?;
    assert_eq!(vector.as_ptr(), address);
    let boxed = vector.into_boxed_slice();
    let address = boxed.as_ptr();
    let boxed = recover(boxed)?;
    assert_eq!(boxed.as_ptr(), address);
    let shared = std::sync::Arc::<[u8]>::from(boxed);
    assert!(std::sync::Arc::ptr_eq(&recover(shared.clone())?, &shared));
    let shared = std::rc::Rc::<[u8]>::from(bytes.as_slice());
    assert!(std::rc::Rc::ptr_eq(&recover(shared.clone())?, &shared));

    let buffer = intrlay::create::<u32>(&7)?;
    let address = buffer.as_ref().as_ptr();
    assert_eq!(recover(buffer)?.as_ref().as_ptr(), address);

    let mut bytes = bytes;
    {
        let mut borrowed = ValueView::<u32, _>::from_bytes(bytes.as_mut_slice())?;
        borrowed.value_mut().set(11);
    }
    assert_eq!(u32::from_le_bytes(bytes), 11);
    Ok(())
}

#[test]
fn codec_views_and_header_sequences_have_checked_entry_points()
-> Result<(), Box<dyn std::error::Error>> {
    let text = intrlay::create::<String>(&"checked 🦀".to_owned())?;
    assert_eq!(String::view(text.as_ref(), 0)?, "checked 🦀");
    assert!(String::view(&text.as_ref()[..text.as_ref().len() - 1], 0).is_err());
    assert!(bool::view(&[2], 0).is_err());
    assert!(u32::view(&[0; 4], usize::MAX).is_err());
    let mut invalid = [2_u8];
    assert!(bool::view_mut(&mut invalid, 0).is_err());

    let mut numbers = intrlay::create::<[u32; 3]>(&[1, 2, 3])?;
    {
        let mut values = <[u32; 3]>::view_mut(numbers.as_mut(), 0)?;
        values.get_mut(1).ok_or("missing element")?.set(20);
    }
    let values = intrlay::SequenceView::<u32>::new(numbers.as_ref(), 0, 3)?;
    assert_eq!(values.iter().collect::<Vec<_>>(), [1, 20, 3]);
    assert!(intrlay::SequenceView::<u32>::new(numbers.as_ref(), 0, usize::MAX).is_err());
    assert!(intrlay::SequenceView::<u32>::new(numbers.as_ref(), usize::MAX, 0).is_err());
    intrlay::SequenceMut::<u32>::new(numbers.as_mut(), 0, 3)?.set(2, &30)?;
    assert_eq!(<[u32; 3]>::decode(numbers.as_ref(), 0)?, [1, 20, 30]);

    // Both headers are reserved before visiting any string payload. A forged
    // reference into the second header must not create a mutable alias.
    let mut strings = intrlay::create::<[String; 2]>(&["first".into(), "second".into()])?;
    let readable = intrlay::SequenceView::<String>::new(strings.as_ref(), 0, 2)?;
    assert_eq!(readable.get(1), Some("second"));
    strings.as_mut()[..4].copy_from_slice(&8_u32.to_le_bytes());
    assert!(intrlay::SequenceView::<String>::new(strings.as_ref(), 0, 2).is_err());
    assert!(intrlay::SequenceMut::<String>::new(strings.as_mut(), 0, 2).is_err());
    Ok(())
}

#[derive(Debug, PartialEq, Intrlay)]
struct Packet {
    id: u64,
    matrices: Vec<[f32; 16]>,
    entries: Vec<Entry>,
}

#[derive(Debug, PartialEq, Intrlay)]
enum Entry {
    Empty,
    Text { label: String, enabled: bool },
    Value(Option<Box<u32>>),
}

#[test]
fn checked_views_decode_and_mutation_share_the_encoding() -> Result<(), Box<dyn std::error::Error>>
{
    let source = Packet {
        id: 7,
        matrices: vec![[1.0; 16], [2.0; 16]],
        entries: vec![
            Entry::Empty,
            Entry::Text {
                label: "UTF-8 🦀 한글".into(),
                enabled: true,
            },
            Entry::Value(Some(Box::new(9))),
        ],
    };
    let encoded = intrlay::create::<Packet>(&source)?;
    assert_eq!(Packet::decode(encoded.as_ref(), 0)?, source);
    let checked = PacketView::from_bytes(&encoded)?;
    // SAFETY: The exact same immutable bytes were validated above.
    let unchecked = unsafe { PacketView::from_bytes_unchecked(&encoded) };
    assert_eq!(checked.id(), unchecked.id());
    assert_eq!(
        checked
            .matrices()
            .get(1)
            .ok_or("missing test element")?
            .get(3),
        Some(2.0)
    );

    // Relative references work unchanged at a nonzero, unaligned base offset.
    let mut bytes = vec![0xa5; 3];
    bytes.extend_from_slice(encoded.as_ref());
    let mut view = PacketView::from_parts(bytes.as_mut_slice(), 3)?;
    view.id_mut().set(11);
    view.matrices_mut()
        .get_mut(1)
        .ok_or("missing test element")?
        .get_mut(3)
        .ok_or("missing test element")?
        .set(4.0);
    assert_eq!(view.id(), 11);
    assert_eq!(
        view.matrices().get(1).ok_or("missing test element")?.get(3),
        Some(4.0)
    );
    assert_eq!(&bytes[..3], &[0xa5; 3]);

    for length in 0..encoded.as_ref().len() {
        assert!(
            PacketView::from_bytes(&encoded.as_ref()[..length]).is_err(),
            "prefix {length}"
        );
        assert!(
            Packet::decode(&encoded.as_ref()[..length], 0).is_err(),
            "prefix {length}"
        );
    }
    assert!(PacketView::from_parts(encoded.as_ref(), usize::MAX).is_err());
    assert!(Packet::decode(encoded.as_ref(), usize::MAX).is_err());
    Ok(())
}

#[test]
fn tags_utf8_lengths_and_references_are_validated() -> Result<(), Box<dyn std::error::Error>> {
    for tag in [2, 255] {
        assert!(matches!(
            bool::decode(&[tag], 0),
            Err(DecodeError::InvalidTag { .. })
        ));
        assert!(ValueView::<bool, _>::from_bytes([tag]).is_err());
        assert!(ValueView::<Option<bool>, _>::from_bytes([tag, 0]).is_err());
    }
    let invalid_tag = [255; Entry::LAYOUT.size()];
    assert!(matches!(
        Entry::decode(&invalid_tag, 0),
        Err(DecodeError::InvalidTag { .. })
    ));
    assert!(EntryView::from_bytes(invalid_tag).is_err());

    // Include overlong encodings, surrogates, missing continuation bytes and >U+10FFFF.
    for value in [
        &b"\xc0\x80"[..],
        &b"\xed\xa0\x80"[..],
        &b"\xf4\x90\x80\x80"[..],
        &b"\xe2\x82"[..],
    ] {
        let mut bytes = intrlay::create::<String>(&"x".repeat(value.len()))?
            .as_ref()
            .to_vec();
        bytes[8..].copy_from_slice(value);
        assert!(matches!(
            String::decode(&bytes, 0),
            Err(DecodeError::InvalidUtf8 { .. })
        ));
        assert!(ValueView::<String, _>::from_bytes(bytes).is_err());
    }
    let mut bytes = intrlay::create::<Vec<u64>>(&vec![1, 2])?.as_ref().to_vec();
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Vec::<u64>::decode(&bytes, 0).is_err());
    assert!(ValueView::<Vec<u64>, _>::from_bytes(&bytes).is_err());
    bytes[..4].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        Vec::<u64>::validate(&bytes, 0),
        Err(DecodeError::InvalidReference { .. })
    ));
    bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Vec::<u64>::validate(&bytes, 0).is_err());
    Ok(())
}

#[derive(Intrlay)]
struct Aliases {
    first: Vec<u8>,
    second: String,
}

#[test]
fn view_validation_rejects_payload_aliases_but_decode_can_copy_them()
-> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = intrlay::create::<Aliases>(&Aliases {
        first: vec![b'a'],
        second: "b".into(),
    })?
    .as_ref()
    .to_vec();
    // Both references point at the second payload: mutating the byte vector would
    // otherwise invalidate a safely borrowed string's UTF-8.
    let target = 8 + u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
    bytes[..4].copy_from_slice(&u32::try_from(target)?.to_le_bytes());
    assert!(matches!(
        AliasesView::from_bytes(&bytes),
        Err(DecodeError::InvalidReference { .. })
    ));
    let owned = Aliases::decode(&bytes, 0)?;
    assert_eq!(owned.first, b"b");
    assert_eq!(owned.second, "b");

    // A reference cannot point into a later field's inline header either.
    bytes[..4].copy_from_slice(&8_u32.to_le_bytes());
    assert!(AliasesView::from_bytes(bytes).is_err());
    Ok(())
}

#[derive(Intrlay)]
struct Node {
    next: Option<Box<Node>>,
}

#[test]
fn recursion_is_bounded_and_empty_values_are_supported() -> Result<(), Box<dyn std::error::Error>> {
    let mut node = Node { next: None };
    for _ in 0..16 {
        node = Node {
            next: Some(Box::new(node)),
        };
    }
    let bytes = intrlay::create::<Node>(&node)?;
    assert!(NodeView::from_bytes(&bytes).is_ok());
    let mut validator = Validator::with_max_depth(bytes.as_ref(), 4);
    assert_eq!(validator.validate::<Node>(0), Err(DecodeError::DepthLimit));
    assert_eq!(validator.validate::<Node>(0), Err(DecodeError::DepthLimit));
    let mut decoder = Decoder::with_max_depth(bytes.as_ref(), 4);
    assert!(matches!(
        decoder.decode::<Node>(0),
        Err(DecodeError::DepthLimit)
    ));
    assert!(matches!(
        decoder.decode::<Node>(0),
        Err(DecodeError::DepthLimit)
    ));
    assert!(ValueView::<(), _>::from_bytes([]).is_ok());
    assert!(ValueView::<[String; 0], _>::from_bytes([]).is_ok());
    let units = intrlay::create::<Vec<()>>(&vec![(); 3])?;
    assert_eq!(
        ValueView::<Vec<()>, _>::from_bytes(&units)?.value().len(),
        3
    );
    Ok(())
}

static DROPS: AtomicUsize = AtomicUsize::new(0);

#[derive(Intrlay)]
struct Dropped {
    valid: bool,
}

impl Drop for Dropped {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn failed_fixed_array_decode_drops_its_initialized_prefix() {
    DROPS.store(0, Ordering::Relaxed);
    assert!(<[Dropped; 3]>::decode(&[1, 0, 2], 0).is_err());
    assert_eq!(DROPS.load(Ordering::Relaxed), 2);
}

#[derive(Clone, Debug, PartialEq, Intrlay)]
struct FixedRecord {
    byte: u8,
    words: [u32; 3],
}

#[derive(Clone, Debug, PartialEq, Intrlay)]
struct FixedEnvelope {
    records: [FixedRecord; 2],
}

#[test]
fn fixed_conversion_preserves_depth_limits_and_unaligned_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    let value = FixedEnvelope {
        records: [
            FixedRecord {
                byte: 7,
                words: [11, 13, 17],
            },
            FixedRecord {
                byte: 19,
                words: [23, 29, 31],
            },
        ],
    };
    let bytes = intrlay::create::<FixedEnvelope>(&value)?;
    let mut unaligned = vec![0xa5];
    unaligned.extend_from_slice(bytes.as_ref());
    assert_eq!(FixedEnvelope::decode(&unaligned, 1)?, value);
    for length in 0..bytes.as_ref().len() {
        assert!(FixedEnvelope::decode(&unaligned[..=length], 1).is_err());
    }
    assert!(FixedEnvelope::decode(&unaligned, usize::MAX).is_err());

    // Envelope -> fixed array -> record -> native numeric field needs four
    // levels on little-endian targets. The numeric array needs one additional
    // level on big-endian targets, where its elements require conversion.
    let required_depth = if cfg!(target_endian = "little") { 4 } else { 5 };
    for depth in 0..required_depth {
        let mut decoder = Decoder::with_max_depth(&unaligned, depth);
        for _ in 0..2 {
            assert_eq!(
                decoder.decode::<FixedEnvelope>(1),
                Err(DecodeError::DepthLimit)
            );
        }
    }
    assert_eq!(
        Decoder::with_max_depth(&unaligned, required_depth).decode::<FixedEnvelope>(1)?,
        value
    );
    // With a nonzero but insufficient budget, the outer bounds error still
    // precedes the first descendant's depth failure.
    assert!(matches!(
        Decoder::with_max_depth(&[], 1).decode::<FixedEnvelope>(0),
        Err(DecodeError::OutOfBounds { .. })
    ));

    let values = vec![value.clone(), value];
    let bytes = intrlay::create::<Vec<FixedEnvelope>>(&values)?;
    assert_eq!(Vec::<FixedEnvelope>::decode(bytes.as_ref(), 0)?, values);
    assert_eq!(
        Decoder::with_max_depth(bytes.as_ref(), required_depth).decode::<Vec<FixedEnvelope>>(0),
        Err(DecodeError::DepthLimit)
    );
    assert_eq!(
        Decoder::with_max_depth(bytes.as_ref(), required_depth + 1)
            .decode::<Vec<FixedEnvelope>>(0)?,
        values
    );
    for length in 0..bytes.as_ref().len() {
        assert!(Vec::<FixedEnvelope>::decode(&bytes.as_ref()[..length], 0).is_err());
    }
    #[cfg(feature = "smallvec")]
    {
        use intrlay::adapters::smallvec::SmallVec;
        // Exercise both inline and spilled element destinations.
        assert_eq!(
            SmallVec::<FixedEnvelope, 4>::decode(bytes.as_ref(), 0)?.as_slice(),
            values
        );
        assert_eq!(
            SmallVec::<FixedEnvelope, 1>::decode(bytes.as_ref(), 0)?.as_slice(),
            values
        );
        assert!(matches!(
            Decoder::with_max_depth(bytes.as_ref(), required_depth)
                .decode::<SmallVec<FixedEnvelope, 4>>(0),
            Err(DecodeError::DepthLimit)
        ));
    }
    // Empty fixed arrays need no child budget, including arrays of dynamic types.
    assert!(
        Decoder::with_max_depth(&[], 1)
            .decode::<[String; 0]>(0)?
            .is_empty()
    );
    let empty = intrlay::create::<Vec<FixedEnvelope>>(&vec![])?;
    assert!(
        Decoder::with_max_depth(empty.as_ref(), 1)
            .decode::<Vec<FixedEnvelope>>(0)?
            .is_empty()
    );
    Ok(())
}

static CUSTOM_DECODES: AtomicUsize = AtomicUsize::new(0);

struct CustomNumber;

impl intrlay::schema::Describe for CustomNumber {
    fn describe(registry: &mut intrlay::schema::Registry) -> intrlay::schema::Type {
        <u32 as intrlay::schema::Describe>::describe(registry)
    }
}

// SAFETY: Representation, validation, and access delegate to u32. Checked
// reconstruction deliberately has its own child call and no static depth proof.
unsafe impl Intrlay for CustomNumber {
    type Value = u32;
    type View<'a> = u32;
    type ViewMut<'a> = intrlay::ScalarMut<'a, u32>;
    const LAYOUT: intrlay::layout::Layout = u32::LAYOUT;
    const ALL_BITS_VALID: bool = true;
    fn measure(value: &u32, cursor: &mut intrlay::SizeCursor) -> Result<(), intrlay::EncodeError> {
        u32::measure(value, cursor)
    }
    fn write(
        value: &u32,
        writer: &mut intrlay::Writer<'_>,
        offset: usize,
    ) -> Result<(), intrlay::EncodeError> {
        u32::write(value, writer, offset)
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> u32 {
        // SAFETY: The caller supplies the complete valid u32 header.
        unsafe { u32::view_unchecked(bytes, offset) }
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        // SAFETY: The caller exclusively supplies the complete valid u32 header.
        unsafe { u32::view_mut_unchecked(bytes, offset) }
    }
    fn validate_fields(validator: &mut Validator<'_>, offset: usize) -> Result<(), DecodeError> {
        u32::validate_fields(validator, offset)
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<u32, DecodeError> {
        CUSTOM_DECODES.fetch_add(1, Ordering::Relaxed);
        decoder.decode::<u32>(offset)
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> u32 {
        // SAFETY: This codec has exactly the u32 representation.
        unsafe { u32::decode_unchecked(bytes, offset) }
    }
}

#[derive(Debug, PartialEq, Intrlay)]
struct CustomField {
    #[intrlay(with = CustomNumber)]
    number: u32,
}

#[test]
fn all_bits_valid_custom_codecs_keep_their_checked_traversal()
-> Result<(), Box<dyn std::error::Error>> {
    let values = vec![CustomField { number: 3 }, CustomField { number: 7 }];
    let bytes = intrlay::create::<Vec<CustomField>>(&values)?;
    CUSTOM_DECODES.store(0, Ordering::Relaxed);
    assert_eq!(Vec::<CustomField>::decode(bytes.as_ref(), 0)?, values);
    assert_eq!(CUSTOM_DECODES.load(Ordering::Relaxed), 2);
    assert_eq!(
        Decoder::with_max_depth(bytes.as_ref(), 3).decode::<Vec<CustomField>>(0),
        Err(DecodeError::DepthLimit)
    );
    assert_eq!(
        Decoder::with_max_depth(bytes.as_ref(), 4).decode::<Vec<CustomField>>(0)?,
        values
    );
    Ok(())
}

#[cfg(panic = "unwind")]
mod constructor_unwind {
    use super::{AtomicUsize, Intrlay, Ordering};

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct Value {
        id: u32,
    }

    impl Drop for Value {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[derive(Clone, Copy, Intrlay)]
    #[intrlay(remote = Value, from = Codec::construct)]
    struct Codec {
        #[intrlay(getter = Codec::id)]
        id: u32,
    }

    impl Codec {
        fn id(value: &Value) -> u32 {
            value.id
        }
        fn construct(value: Self) -> Value {
            assert_ne!(value.id, 2, "test constructor unwinds on the third value");
            Value { id: value.id }
        }
    }

    fn check<C: Intrlay>(bytes: &[u8]) {
        DROPS.store(0, Ordering::Relaxed);
        assert!(std::panic::catch_unwind(|| C::decode(bytes, 0)).is_err());
        assert_eq!(DROPS.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn converted_prefixes_are_dropped_when_a_constructor_unwinds()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = intrlay::create::<Vec<Codec>>(&vec![
            Value { id: 0 },
            Value { id: 1 },
            Value { id: 2 },
        ])?;
        check::<Vec<Codec>>(bytes.as_ref());
        // The sequence starts at byte 8; its headers also form a fixed array.
        check::<[Codec; 3]>(&bytes.as_ref()[8..]);
        #[cfg(feature = "smallvec")]
        {
            check::<intrlay::adapters::smallvec::SmallVec<Codec, 4>>(bytes.as_ref());
            check::<intrlay::adapters::smallvec::SmallVec<Codec, 1>>(bytes.as_ref());
        }
        Ok(())
    }
}

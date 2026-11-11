//! Initialization elision must preserve every wire byte and allocation lifetime.

use std::{cell::Cell, error::Error, hint::black_box, panic::catch_unwind};

use intrlay::{
    EncodeError, Initialization, Intrlay, OwnedBuffer, SizeCursor, Writer, compute_layout, create,
    layout::Layout,
    schema::{Describe, Registry, Type},
    write_into,
};

#[derive(Intrlay)]
#[repr(C)]
struct Vertex {
    id: u32,
    position: [f32; 3],
    color: [f32; 4],
}

#[derive(Intrlay)]
struct Frame {
    sequence: u64,
    timestamp: u64,
    vertices: Vec<Vertex>,
}

#[derive(Intrlay)]
struct Padded {
    byte: u8,
    word: u32,
}

#[derive(Intrlay)]
#[repr(C, align(16))]
struct Aligned {
    value: [f32; 3],
}

#[derive(Intrlay)]
struct Mixed {
    bytes: Vec<u8>,
    words: Vec<u64>,
}

#[derive(Intrlay)]
struct TextAndWords {
    text: String,
    words: Vec<u64>,
}

#[derive(Intrlay)]
struct AlignedBox {
    id: u32,
    word: Box<u64>,
}

#[derive(Intrlay)]
struct Nested {
    frames: [Frame; 2],
    tail: Vec<u32>,
}

#[derive(Intrlay)]
enum Choice {
    Empty,
    Wide(u64),
    Text(String),
}

#[derive(Intrlay)]
struct Tree {
    children: Vec<Self>,
}

#[derive(Intrlay)]
struct Left {
    children: Vec<Right>,
}

#[derive(Intrlay)]
struct Right {
    parent: Vec<Left>,
}

struct External {
    first: u16,
    second: u16,
}

#[allow(dead_code, reason = "fields describe the external value")]
#[derive(Intrlay)]
#[intrlay(remote = External)]
struct Adapter {
    second: u16,
    first: u16,
}

fn check<C: Intrlay>(value: &C::Value) -> Result<(), Box<dyn Error>> {
    let layout = compute_layout::<C>(value)?;
    // The reference path deliberately starts with initialized zeroes, regardless
    // of the codec's proof, so both optimized paths have an independent baseline.
    let mut reference = OwnedBuffer::new(layout)?;
    assert!(reference.as_ref().iter().all(|byte| *byte == 0));
    C::write(value, &mut Writer::new(reference.as_mut(), C::LAYOUT)?, 0)?;
    let fresh = create::<C>(value)?;
    assert_eq!(fresh.as_ref(), reference.as_ref());

    let padding = layout.alignment().max(16);
    let capacity = layout.byte_length() + padding * 2;
    let mut reused = OwnedBuffer::new(compute_layout::<Vec<u64>>(&vec![0; capacity])?)?;
    reused.as_mut().fill(0xa5);
    let address = reused.as_ref().as_ptr().addr();
    let offset = (address + padding).next_multiple_of(layout.alignment()) - address;
    let actual = write_into::<C>(value, &mut reused.as_mut()[offset..])?;
    assert_eq!(actual, layout);
    let end = offset + layout.byte_length();
    assert_eq!(&reused.as_ref()[offset..end], reference.as_ref());
    assert!(reused.as_ref()[..offset].iter().all(|byte| *byte == 0xa5));
    assert!(reused.as_ref()[end..].iter().all(|byte| *byte == 0xa5));
    Ok(())
}

#[test]
fn complete_headers_and_aligned_payloads_have_const_proofs() -> Result<(), Box<dyn Error>> {
    assert!(black_box(Frame::WRITES_ALL_BYTES));
    assert!(!black_box(Frame::NATIVE_LAYOUT));
    assert!(black_box(Nested::WRITES_ALL_BYTES));
    assert!(black_box(AlignedBox::WRITES_ALL_BYTES));
    assert!(black_box(<Vec<Adapter>>::WRITES_ALL_BYTES));
    assert!(!black_box(Adapter::NATIVE_LAYOUT));
    for vertices in [
        vec![],
        vec![Vertex {
            id: u32::MAX,
            position: [-0.0, f32::from_bits(0x7fc1_2345), f32::INFINITY],
            color: [1.0, -2.0, 3.0, 4.0],
        }],
    ] {
        check::<Frame>(&Frame {
            sequence: 7,
            timestamp: 11,
            vertices,
        })?;
    }
    check::<Nested>(&Nested {
        frames: [
            Frame {
                sequence: 1,
                timestamp: 2,
                vertices: vec![],
            },
            Frame {
                sequence: 3,
                timestamp: 4,
                vertices: vec![],
            },
        ],
        tail: vec![5, 6, 7],
    })?;
    check::<AlignedBox>(&AlignedBox {
        id: 9,
        word: Box::new(u64::MAX),
    })?;
    check::<Vec<Adapter>>(&vec![External {
        first: 1,
        second: 2,
    }])?;
    check::<String>(&"UTF-8: 한글".to_owned())?;
    check::<()>(&())?;
    check::<[u64; 0]>(&[])?;
    check::<Vec<[u64; 0]>>(&vec![[]; 3])?;
    Ok(())
}

#[test]
fn headers_inactive_values_and_payload_gaps_keep_clearing() -> Result<(), Box<dyn Error>> {
    assert!(!black_box(Padded::WRITES_ALL_BYTES));
    assert!(!black_box(Aligned::WRITES_ALL_BYTES));
    assert!(!black_box(<Option<u32>>::WRITES_ALL_BYTES));
    assert!(!black_box(Choice::WRITES_ALL_BYTES));
    assert!(!black_box(Mixed::WRITES_ALL_BYTES));
    assert!(!black_box(TextAndWords::WRITES_ALL_BYTES));
    assert!(!black_box(<Box<u64>>::WRITES_ALL_BYTES));
    check::<Padded>(&Padded { byte: 3, word: 5 })?;
    check::<Aligned>(&Aligned {
        value: [1.0, 2.0, 3.0],
    })?;
    check::<Option<u32>>(&None)?;
    check::<Option<u32>>(&Some(9))?;
    check::<Box<u64>>(&Box::new(17))?;
    for choice in [
        Choice::Empty,
        Choice::Wide(u64::MAX),
        Choice::Text("value".to_owned()),
    ] {
        check::<Choice>(&choice)?;
    }
    // The second reservation may introduce padding even for an empty vector.
    for length in 0..17 {
        for words in [vec![], vec![u64::MAX, 19]] {
            check::<Mixed>(&Mixed {
                bytes: vec![0x37; length],
                words: words.clone(),
            })?;
            check::<TextAndWords>(&TextAndWords {
                text: "x".repeat(length),
                words,
            })?;
        }
    }
    Ok(())
}

#[test]
fn dynamic_and_recursive_elements_have_a_terminating_fallback() -> Result<(), Box<dyn Error>> {
    assert!(!black_box(<Vec<Vec<u32>>>::WRITES_ALL_BYTES));
    assert!(!black_box(Tree::WRITES_ALL_BYTES));
    assert!(!black_box(Left::WRITES_ALL_BYTES));
    assert!(!black_box(Right::WRITES_ALL_BYTES));
    check::<Vec<Vec<u32>>>(&vec![vec![1, 2], vec![], vec![3]])?;
    check::<Tree>(&Tree {
        children: vec![Tree { children: vec![] }],
    })?;
    check::<Left>(&Left {
        children: vec![Right {
            parent: vec![Left { children: vec![] }],
        }],
    })?;
    Ok(())
}

struct Fallible;

impl Describe for Fallible {
    fn describe(registry: &mut Registry) -> Type {
        <[u8; 32]>::describe(registry)
    }
}

// SAFETY: Successful writes initialize all 32 bytes. Failure paths do not return
// initialized views, and the byte-array codec provides the same representation.
unsafe impl Intrlay for Fallible {
    const ALL_BITS_VALID: bool = true;
    fn validate_fields(
        _: &mut intrlay::Validator<'_>,
        _: usize,
    ) -> Result<(), intrlay::DecodeError> {
        Ok(())
    }

    type Value = u8;
    type View<'a> = &'a [u8];
    type ViewMut<'a> = &'a mut [u8];
    const LAYOUT: Layout = Layout::new(32, 1);
    const WRITES_FIXED_LAYOUT: bool = true;
    const INITIALIZATION: Initialization = Initialization::FIXED;

    fn measure(_: &u8, _: &mut SizeCursor) -> Result<(), EncodeError> {
        Ok(())
    }

    fn write(value: &u8, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        writer.write(offset, &[0x39; 4]);
        match value {
            0 => Err(EncodeError::SizeOverflow),
            1 => panic!("intentional codec failure after a partial write"),
            _ => {
                writer.write(offset + 4, &[0x57; 28]);
                Ok(())
            }
        }
    }

    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> &[u8] {
        &bytes[offset..offset + 32]
    }

    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> &mut [u8] {
        &mut bytes[offset..offset + 32]
    }

    fn decode_from(
        decoder: &mut intrlay::Decoder<'_>,
        offset: usize,
    ) -> Result<u8, intrlay::DecodeError> {
        decoder.range(offset, 32)?;
        Ok(2)
    }

    unsafe fn decode_unchecked(_: &[u8], _: usize) -> u8 {
        2
    }
}

#[test]
fn partially_written_allocations_are_discarded_on_error_or_panic() -> Result<(), Box<dyn Error>> {
    #[derive(Intrlay)]
    struct Wrapped {
        prefix: u32,
        #[intrlay(with = Fallible)]
        last: u8,
    }
    assert!(matches!(
        create::<Fallible>(&0),
        Err(EncodeError::SizeOverflow)
    ));
    assert!(catch_unwind(|| create::<Fallible>(&1)).is_err());
    check::<Fallible>(&2)?;
    assert!(matches!(
        create::<Wrapped>(&Wrapped { prefix: 1, last: 0 }),
        Err(EncodeError::SizeOverflow)
    ));
    assert!(catch_unwind(|| create::<Wrapped>(&Wrapped { prefix: 1, last: 1 })).is_err());
    check::<Wrapped>(&Wrapped { prefix: 1, last: 2 })?;
    Ok(())
}

struct OffsetSensitive;

impl Describe for OffsetSensitive {
    fn describe(registry: &mut Registry) -> Type {
        u8::describe(registry)
    }
}

// SAFETY: Every byte is valid. XOR with the absolute offset is reversible, and
// all reads and writes stay within this one-byte fixed header.
unsafe impl Intrlay for OffsetSensitive {
    type Value = u8;
    type View<'a> = u8;
    type ViewMut<'a> = &'a mut [u8];
    const LAYOUT: Layout = Layout::new(1, 1);
    const ALL_BITS_VALID: bool = true;
    const WRITES_FIXED_LAYOUT: bool = true;
    const INITIALIZATION: Initialization = Initialization::FIXED;

    fn measure(_: &u8, _: &mut SizeCursor) -> Result<(), EncodeError> {
        Ok(())
    }

    fn write(value: &u8, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        writer.write(offset, &[value ^ offset.to_le_bytes()[0]]);
        Ok(())
    }

    fn validate_fields(
        _: &mut intrlay::Validator<'_>,
        _: usize,
    ) -> Result<(), intrlay::DecodeError> {
        Ok(())
    }

    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> u8 {
        bytes[offset] ^ offset.to_le_bytes()[0]
    }

    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> &mut [u8] {
        &mut bytes[offset..=offset]
    }

    fn decode_from(
        decoder: &mut intrlay::Decoder<'_>,
        offset: usize,
    ) -> Result<u8, intrlay::DecodeError> {
        Ok(decoder.range(offset, 1)?[0] ^ offset.to_le_bytes()[0])
    }

    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> u8 {
        bytes[offset] ^ offset.to_le_bytes()[0]
    }
}

#[test]
fn fixed_custom_codecs_retain_absolute_offsets_in_owned_and_borrowed_records()
-> Result<(), Box<dyn Error>> {
    #[derive(Debug, PartialEq, Intrlay)]
    struct Located {
        prefix: u64,
        #[intrlay(with = OffsetSensitive)]
        value: u8,
    }
    let records = vec![
        Located {
            prefix: 1,
            value: 3,
        },
        Located {
            prefix: 2,
            value: 7,
        },
    ];
    let owned = create::<Vec<Located>>(&records)?;
    assert_eq!(Vec::<Located>::decode(owned.as_ref(), 0)?, records);
    let input = intrlay::SequenceInput::new(records.iter().map(|record| LocatedInput {
        prefix: &record.prefix,
        value: intrlay::ValueSource::<OffsetSensitive>::new(&record.value),
    }));
    let borrowed = intrlay::create_from::<Vec<Located>>(&input)?;
    assert_eq!(borrowed.as_ref(), owned.as_ref());
    for (view, record) in Vec::<Located>::view(borrowed.as_ref(), 0)?
        .iter()
        .zip(&records)
    {
        assert_eq!(view.value(), record.value);
    }
    check::<Vec<Located>>(&records)?;
    Ok(())
}

#[test]
fn replacement_preserves_live_bytes_on_custom_codec_failure() -> Result<(), Box<dyn Error>> {
    let mut view = intrlay::ValueView::<Fallible, _>::create(&2)?;
    view.value_mut().fill(0x91);
    let original = view.as_bytes().to_vec();
    assert!(matches!(view.replace(&0), Err(EncodeError::SizeOverflow)));
    assert_eq!(view.as_bytes(), original);
    assert!(catch_unwind(std::panic::AssertUnwindSafe(|| view.replace(&1))).is_err());
    assert_eq!(view.as_bytes(), original);
    view.replace(&2)?;
    assert_eq!(&view.value()[..4], &[0x39; 4]);
    Ok(())
}

struct ChangingSource {
    reads: Cell<usize>,
    measured: &'static str,
    written: &'static str,
}

fn changing_text(value: &ChangingSource) -> String {
    let reads = value.reads.get();
    value.reads.set(reads + 1);
    if reads == 0 {
        value.measured
    } else {
        value.written
    }
    .to_owned()
}

fn from_changing_adapter(_: ChangingAdapter) -> ChangingSource {
    ChangingSource {
        reads: Cell::new(0),
        measured: "",
        written: "",
    }
}

#[allow(dead_code, reason = "the field describes a remote getter")]
#[derive(Intrlay)]
#[intrlay(remote = ChangingSource, from = from_changing_adapter)]
struct ChangingAdapter {
    #[intrlay(getter = changing_text)]
    text: String,
}

#[derive(Intrlay)]
struct NestedChangingSource {
    #[intrlay(with = ChangingAdapter)]
    source: ChangingSource,
    suffix: String,
}

#[test]
fn changing_remote_payloads_cannot_publish_uninitialized_bytes() -> Result<(), Box<dyn Error>> {
    assert!(black_box(ChangingAdapter::WRITES_ALL_BYTES));
    assert!(black_box(NestedChangingSource::WRITES_ALL_BYTES));
    for (measured, written) in [("abc", ""), ("", "abc"), ("abc", "xyz")] {
        let source = ChangingSource {
            reads: Cell::new(0),
            measured,
            written,
        };
        let layout = compute_layout::<ChangingAdapter>(&source)?;
        source.reads.set(0);
        let fresh = create::<ChangingAdapter>(&source);
        source.reads.set(0);
        let mut buffer = OwnedBuffer::new(layout)?;
        buffer.as_mut().fill(0xa5);
        let reused = write_into::<ChangingAdapter>(&source, buffer.as_mut());
        match measured.len().cmp(&written.len()) {
            std::cmp::Ordering::Greater => {
                assert!(matches!(fresh, Err(EncodeError::LayoutChanged { .. })));
                assert!(matches!(reused, Err(EncodeError::LayoutChanged { .. })));
            }
            std::cmp::Ordering::Less => {
                assert!(matches!(fresh, Err(EncodeError::BufferTooSmall { .. })));
                assert!(matches!(reused, Err(EncodeError::BufferTooSmall { .. })));
            }
            std::cmp::Ordering::Equal => {
                let fresh = fresh?;
                assert_eq!(reused?, layout);
                assert_eq!(fresh.as_ref(), buffer.as_ref());
                // SAFETY: Both successful constructors established the encoding's validity.
                let view = unsafe { ChangingAdapter::view_unchecked(fresh.as_ref(), 0) };
                assert_eq!(view.text(), written);
            }
        }
    }
    let nested = NestedChangingSource {
        source: ChangingSource {
            reads: Cell::new(0),
            measured: "abc",
            written: "",
        },
        suffix: "tail".to_owned(),
    };
    assert!(matches!(
        create::<NestedChangingSource>(&nested),
        Err(EncodeError::LayoutChanged { .. })
    ));
    nested.source.reads.set(0);
    // A sequence uses the conservative clearing path, but still rejects a changed layout.
    assert!(matches!(
        create::<Vec<ChangingAdapter>>(&vec![nested.source]),
        Err(EncodeError::LayoutChanged { .. })
    ));
    Ok(())
}

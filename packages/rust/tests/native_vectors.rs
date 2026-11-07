//! Contiguous native writes retain wire bytes and codec fallback behavior.

use std::error::Error;

use intrlay::{Intrlay, OwnedBuffer, compute_layout, create, write_into};

#[derive(Intrlay)]
#[repr(C)]
struct Record {
    id: u32,
    values: [f32; 3],
}

#[derive(Intrlay)]
#[repr(C)]
struct Padded {
    small: u8,
    wide: u32,
}

#[derive(Intrlay)]
#[repr(C, align(16))]
struct TrailingPadding {
    values: [f32; 3],
}

#[repr(C)]
struct External {
    first: u16,
    second: u16,
}

#[allow(dead_code, reason = "adapter fields describe the external type")]
#[derive(Intrlay)]
#[intrlay(remote = External)]
struct ReorderedAdapter {
    second: u16,
    first: u16,
}

fn reused<C: Intrlay>(value: &C::Value) -> Result<OwnedBuffer, Box<dyn Error>> {
    let expected = create::<C>(value)?;
    let size = expected.as_ref().len();
    let padding = expected.layout().alignment().max(16);
    let mut owner = OwnedBuffer::new(compute_layout::<Vec<u8>>(&vec![0; size + padding * 2])?)?;
    owner.as_mut().fill(0xa5);
    let address = owner.as_ref().as_ptr().addr();
    let offset = (address + padding).next_multiple_of(expected.layout().alignment()) - address;
    let layout = write_into::<C>(value, &mut owner.as_mut()[offset..])?;
    assert_eq!(layout.byte_length(), size);
    assert_eq!(&owner.as_ref()[offset..offset + size], expected.as_ref());
    assert!(owner.as_ref()[..offset].iter().all(|byte| *byte == 0xa5));
    assert!(
        owner.as_ref()[offset + size..]
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    Ok(expected)
}

#[test]
fn native_vectors_preserve_float_bits_and_nested_arrays() -> Result<(), Box<dyn Error>> {
    let values = vec![
        Record {
            id: 0x1234_5678,
            values: [-0.0, f32::from_bits(0x7fc1_2345), f32::INFINITY],
        },
        Record {
            id: 0xfedc_ba98,
            values: [1.5, -2.25, f32::NEG_INFINITY],
        },
    ];
    assert_eq!(
        std::hint::black_box(Record::NATIVE_LAYOUT),
        cfg!(target_endian = "little")
    );
    let mut expected = [8_u32.to_le_bytes(), 2_u32.to_le_bytes()].concat();
    for record in &values {
        expected.extend_from_slice(&record.id.to_le_bytes());
        for value in record.values {
            expected.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(reused::<Vec<Record>>(&values)?.as_ref(), expected);

    let arrays = vec![[[1_u16, 2, 3], [4, 5, 6]], [[7, 8, 9], [10, 11, 12]]];
    let bytes = reused::<Vec<[[u16; 3]; 2]>>(&arrays)?;
    let expected: Vec<u8> = (1_u16..=12).flat_map(u16::to_le_bytes).collect();
    assert_eq!(&bytes.as_ref()[8..], expected);
    Ok(())
}

#[test]
fn padding_and_adapter_layouts_use_element_codecs() -> Result<(), Box<dyn Error>> {
    assert!(!std::hint::black_box(Padded::NATIVE_LAYOUT));
    assert!(!std::hint::black_box(TrailingPadding::NATIVE_LAYOUT));
    let trailing = reused::<Vec<TrailingPadding>>(&vec![TrailingPadding {
        values: [1.0, 2.0, 3.0],
    }])?;
    assert_eq!(&trailing.as_ref()[28..32], &[0; 4]);
    let bytes = reused::<Vec<Padded>>(&vec![
        Padded { small: 7, wide: 11 },
        Padded {
            small: 13,
            wide: 17,
        },
    ])?;
    assert_eq!(
        &bytes.as_ref()[8..],
        &[7, 0, 0, 0, 11, 0, 0, 0, 13, 0, 0, 0, 17, 0, 0, 0]
    );

    let values = vec![
        External {
            first: 1,
            second: 2,
        },
        External {
            first: 3,
            second: 4,
        },
    ];
    let bytes = reused::<Vec<ReorderedAdapter>>(&values)?;
    assert_eq!(&bytes.as_ref()[8..], &[2, 0, 1, 0, 4, 0, 3, 0]);
    Ok(())
}

#[test]
fn empty_and_zero_sized_vectors_keep_their_counts() -> Result<(), Box<dyn Error>> {
    assert_eq!(
        reused::<Vec<Record>>(&vec![])?.as_ref(),
        &[8, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        reused::<Vec<()>>(&vec![(); 3])?.as_ref(),
        &[8, 0, 0, 0, 3, 0, 0, 0]
    );
    assert_eq!(
        reused::<Vec<[u64; 0]>>(&vec![[]; 2])?.as_ref(),
        &[8, 0, 0, 0, 2, 0, 0, 0]
    );
    Ok(())
}

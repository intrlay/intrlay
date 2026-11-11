//! Contiguous native copies retain wire bytes and codec fallback behavior.

use std::error::Error;

use intrlay::{Intrlay, OwnedBuffer, compute_layout, create, write_into};

#[derive(Clone, Copy, Intrlay)]
#[repr(C)]
struct Record {
    id: u32,
    values: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Intrlay)]
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
    // Checked reconstruction accepts unaligned input and preserves float payloads.
    let mut unaligned = vec![0xa5];
    unaligned.extend_from_slice(&expected);
    let decoded = Vec::<Record>::decode(&unaligned, 1)?;
    let mut copied = [Record {
        id: 0,
        values: [0.0; 3],
    }; 2];
    Vec::<Record>::view(&unaligned, 1)?.copy_to(&mut copied);
    for (source, decoded) in values.iter().zip(&decoded) {
        assert_eq!(source.id, decoded.id);
        assert_eq!(
            source.values.map(f32::to_bits),
            decoded.values.map(f32::to_bits)
        );
    }
    for (source, copied) in values.iter().zip(copied) {
        assert_eq!(source.id, copied.id);
        assert_eq!(
            source.values.map(f32::to_bits),
            copied.values.map(f32::to_bits)
        );
    }
    for length in 0..expected.len() {
        assert!(Vec::<Record>::decode(&expected[..length], 0).is_err());
    }
    let decoded = Record::decode(&unaligned, 9)?;
    assert_eq!(
        decoded.values.map(f32::to_bits),
        values[0].values.map(f32::to_bits)
    );

    let arrays = vec![[[1_u16, 2, 3], [4, 5, 6]], [[7, 8, 9], [10, 11, 12]]];
    let bytes = reused::<Vec<[[u16; 3]; 2]>>(&arrays)?;
    let expected: Vec<u8> = (1_u16..=12).flat_map(u16::to_le_bytes).collect();
    assert_eq!(&bytes.as_ref()[8..], expected);
    assert_eq!(Vec::<[[u16; 3]; 2]>::decode(bytes.as_ref(), 0)?, arrays);
    let mut copied = [[[0; 3]; 2]; 2];
    Vec::<[[u16; 3]; 2]>::view(bytes.as_ref(), 0)?.copy_to(&mut copied);
    assert_eq!(copied.as_slice(), arrays);
    let fixed = reused::<[[u16; 3]; 2]>(&arrays[0])?;
    assert_eq!(
        fixed.as_ref(),
        intrlay::create_from::<[[u16; 3]; 2]>(&arrays[0])?.as_ref()
    );
    <[[u16; 3]; 2]>::validate(fixed.as_ref(), 0)?;
    assert_eq!(
        // SAFETY: Validation established the complete nested array encoding.
        unsafe { <[[u16; 3]; 2]>::decode_unchecked(fixed.as_ref(), 0) },
        arrays[0]
    );
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
    let decoded = Vec::<Padded>::decode(bytes.as_ref(), 0)?;
    assert_eq!((decoded[1].small, decoded[1].wide), (13, 17));
    let mut copied = [Padded { small: 0, wide: 0 }; 2];
    Vec::<Padded>::view(bytes.as_ref(), 0)?.copy_to(&mut copied);
    assert_eq!(copied.as_slice(), decoded);

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
    let decoded = Vec::<ReorderedAdapter>::decode(bytes.as_ref(), 0)?;
    assert_eq!((decoded[1].first, decoded[1].second), (3, 4));
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
    let units = reused::<Vec<()>>(&vec![(); 3])?;
    assert_eq!(Vec::<()>::decode(units.as_ref(), 0)?.len(), 3);
    Vec::<()>::view(units.as_ref(), 0)?.copy_to(&mut [(); 3]);
    Vec::<Record>::view(reused::<Vec<Record>>(&vec![])?.as_ref(), 0)?.copy_to(&mut []);
    let arrays = reused::<Vec<[u64; 0]>>(&vec![[]; 2])?;
    assert_eq!(Vec::<[u64; 0]>::decode(arrays.as_ref(), 0)?.len(), 2);
    assert!(Vec::<Record>::decode(reused::<Vec<Record>>(&vec![])?.as_ref(), 0)?.is_empty());
    Ok(())
}

#[test]
fn native_representation_does_not_skip_boolean_validation() -> Result<(), Box<dyn Error>> {
    let mut bytes = create::<Vec<[bool; 2]>>(&vec![[true, false]])?
        .as_ref()
        .to_vec();
    let mut copied = [[false; 2]];
    Vec::<[bool; 2]>::view(&bytes, 0)?.copy_to(&mut copied);
    assert_eq!(copied, [[true, false]]);
    bytes[9] = 2;
    assert!(Vec::<[bool; 2]>::decode(&bytes, 0).is_err());
    assert!(Vec::<[bool; 2]>::view(&bytes, 0).is_err());
    assert!(<[bool; 2]>::decode(&bytes, 8).is_err());
    Ok(())
}

#[test]
fn copy_to_decodes_options_and_rejects_mismatched_lengths_before_writing()
-> Result<(), Box<dyn Error>> {
    let values = vec![Some(7_u32), None, Some(u32::MAX)];
    let bytes = create::<Vec<Option<u32>>>(&values)?;
    let view = Vec::<Option<u32>>::view(bytes.as_ref(), 0)?;
    let mut destination = [Some(1); 3];
    view.copy_to(&mut destination);
    assert_eq!(destination.as_slice(), values);

    let mut too_short = [Some(2); 2];
    let mut too_long = [Some(3); 4];
    for destination in [too_short.as_mut_slice(), too_long.as_mut_slice()] {
        let before = destination.to_vec();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| view.copy_to(destination)))
                .is_err()
        );
        assert_eq!(destination, before);
    }
    Ok(())
}

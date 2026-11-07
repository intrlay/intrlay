//! Trusted scalar access preserves the same bytes as checked public operations.

use intrlay::{Intrlay, Scalar, SequenceView};

#[test]
fn numeric_views_and_decoding_preserve_unaligned_bits() {
    macro_rules! check {
        ($ty:ty, $value:expr) => {{
            let value: $ty = $value;
            let encoded = value.to_le_bytes();
            for offset in 1..=9 {
                let mut storage = vec![0xa5; offset + encoded.len()];
                storage[offset..].copy_from_slice(&encoded);
                // SAFETY: All bytes of this numeric encoding are initialized and in bounds.
                let actual = unsafe { <$ty as Intrlay>::view(&storage, offset) };
                // SAFETY: Decode borrows the same complete valid numeric encoding.
                let decoded = unsafe { <$ty as Intrlay>::decode(&storage, offset) };
                assert_eq!(actual.to_le_bytes(), encoded);
                assert_eq!(decoded.to_le_bytes(), encoded);
                assert_eq!(
                    <$ty as Scalar>::read(&storage, offset).to_le_bytes(),
                    encoded
                );
            }
        }};
    }
    check!(i8, -93);
    check!(u8, 231);
    check!(i16, -23_579);
    check!(u16, 54_321);
    check!(i32, -1_234_567_890);
    check!(u32, 0xdcba_9876);
    check!(i64, -0x1234_5678_9abc_def0);
    check!(u64, 0xfedc_ba98_7654_3210);
    check!(f32, -13.25);
    check!(f64, 9876.125);
    check!(f32, -0.0);
    check!(f64, -0.0);
    check!(f32, f32::from_bits(0x7fc1_2345));
    check!(f64, f64::from_bits(0x7ff8_1234_5678_9abc));
}

#[test]
#[should_panic(expected = "range end index")]
fn safe_scalar_read_checks_its_region() {
    let _ = <u64 as Scalar>::read(&[0; 8], 1);
}

#[test]
fn sequence_access_checks_element_indices() {
    let mut bytes = [0xa5; 9];
    bytes[1..5].copy_from_slice(&123_u32.to_le_bytes());
    bytes[5..9].copy_from_slice(&456_u32.to_le_bytes());
    // SAFETY: Two complete u32 encodings start at byte one.
    let values = unsafe { SequenceView::<u32>::new_unchecked(&bytes, 1, 2) };
    assert_eq!(values.get(0), Some(123));
    assert_eq!(values.get(1), Some(456));
    assert_eq!(values.get(2), None);
    assert_eq!(values.get(usize::MAX), None);
}

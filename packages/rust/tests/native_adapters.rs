//! Native-layout proofs permit bulk copies without treating Rust padding as bytes.

use std::{error::Error, hint::black_box};

use intrlay::Intrlay;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
struct External {
    coordinates: [f32; 4],
}

#[derive(Intrlay)]
#[intrlay(remote = External)]
#[expect(dead_code, reason = "mirror fields describe the external type")]
struct Direct {
    coordinates: [f32; 4],
}

#[derive(Clone, Copy, Intrlay)]
#[intrlay(remote = External, from = Getter::into_external)]
struct Getter {
    #[intrlay(getter = Getter::coordinates)]
    coordinates: [f32; 4],
}

impl Getter {
    fn coordinates(value: &External) -> [f32; 4] {
        value.coordinates
    }
    fn into_external(value: Self) -> External {
        External {
            coordinates: value.coordinates,
        }
    }
}

// SAFETY: External is exactly four initialized f32s; getters preserve their order.
#[derive(Clone, Copy, Intrlay)]
#[intrlay(remote = External, from = Proven::into_external, unsafe(native_layout))]
struct Proven {
    #[intrlay(getter = Proven::coordinates)]
    coordinates: [f32; 4],
}

impl Proven {
    fn coordinates(value: &External) -> [f32; 4] {
        value.coordinates
    }
    fn into_external(value: Self) -> External {
        External {
            coordinates: value.coordinates,
        }
    }
}

#[repr(C)]
struct Padded {
    byte: u8,
    word: u32,
}

#[derive(Intrlay)]
#[intrlay(remote = Padded)]
#[expect(dead_code, reason = "mirror fields describe the external type")]
struct PaddingCodec {
    byte: u8,
    word: u32,
}

#[repr(C)]
struct Reordered {
    second: u32,
    first: u32,
}

#[derive(Intrlay)]
#[intrlay(remote = Reordered)]
#[expect(dead_code, reason = "mirror fields describe the external type")]
struct OrderCodec {
    first: u32,
    second: u32,
}

#[derive(Debug, PartialEq)]
struct BorrowedExternal<T> {
    value: T,
}

#[derive(Intrlay)]
#[intrlay(remote = BorrowedExternal<T>, from = BorrowedGetter::<T>::into_external)]
struct BorrowedGetter<T: Intrlay<Value = T>> {
    #[intrlay(getter = BorrowedGetter::<T>::value)]
    value: T,
}

impl<T: Intrlay<Value = T>> BorrowedGetter<T> {
    fn value(value: &BorrowedExternal<T>) -> &T {
        &value.value
    }

    fn into_external(value: Self) -> BorrowedExternal<T> {
        BorrowedExternal { value: value.value }
    }
}

fn check_borrowed_getter<T>(value: T) -> Result<(), Box<dyn Error>>
where
    T: Intrlay<Value = T> + std::fmt::Debug + PartialEq,
{
    #[repr(align(16))]
    struct Storage([u8; 64]);

    let source = BorrowedExternal { value };
    let expected = intrlay::create::<T>(&source.value)?;
    let mut storage = Storage([0xcd; 64]);
    let destination = &mut storage.0[..expected.as_ref().len()];
    intrlay::write_into::<BorrowedGetter<T>>(&source, destination)?;
    // Check complete writes in initialized storage before exercising allocation.
    assert_eq!(destination, expected.as_ref());
    let encoded = intrlay::create::<BorrowedGetter<T>>(&source)?;
    assert_eq!(encoded.as_ref(), expected.as_ref());
    assert_eq!(BorrowedGetter::<T>::decode(encoded.as_ref(), 0)?, source);
    Ok(())
}

#[test]
fn borrowed_getters_copy_values_with_the_codec_size() -> Result<(), Box<dyn Error>> {
    check_borrowed_getter(42_u32)?;
    check_borrowed_getter(42_u64)?;
    check_borrowed_getter([42_u64, 99])?;
    Ok(())
}

#[test]
fn direct_fields_prove_layout_and_getters_require_an_explicit_contract()
-> Result<(), Box<dyn Error>> {
    assert!(black_box(Direct::NATIVE_LAYOUT));
    assert!(!black_box(Getter::NATIVE_LAYOUT));
    assert!(black_box(Proven::NATIVE_LAYOUT));
    assert!(!black_box(PaddingCodec::NATIVE_LAYOUT));
    assert!(!black_box(OrderCodec::NATIVE_LAYOUT));
    let values = vec![
        External {
            coordinates: [1.0, -2.0, 3.5, 4.0]
        };
        17
    ];
    let expected = intrlay::create::<Vec<Getter>>(&values)?;
    let direct = intrlay::create::<Vec<Direct>>(&values)?;
    let proven = intrlay::create::<Vec<Proven>>(&values)?;
    let borrowed = intrlay::create_from::<Vec<Proven>>(values.as_slice())?;
    assert_eq!(direct.as_ref(), expected.as_ref());
    assert_eq!(proven.as_ref(), expected.as_ref());
    assert_eq!(borrowed.as_ref(), expected.as_ref());
    assert_eq!(Vec::<Proven>::decode(proven.as_ref(), 0)?, values);
    let mut copied = [External {
        coordinates: [0.0; 4],
    }; 17];
    Vec::<Proven>::view(proven.as_ref(), 0)?.copy_to(&mut copied);
    assert_eq!(copied.as_slice(), values);
    Vec::<Getter>::view(expected.as_ref(), 0)?.copy_to(&mut copied);
    assert_eq!(copied.as_slice(), values);
    Ok(())
}

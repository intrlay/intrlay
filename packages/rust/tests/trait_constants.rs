//! Source constants must not override the codec's layout and initialization proofs.

use std::{error::Error, hint::black_box};

use intrlay::{Intrlay, OwnedBuffer, compute_layout, layout::Layout};

#[derive(Debug, PartialEq, Intrlay)]
#[repr(C)]
struct Padded {
    first: u8,
    second: u32,
}

impl Padded {
    const NATIVE_LAYOUT: bool = true;
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(C, align(8))]
struct Aligned {
    value: u32,
}

impl Aligned {
    const LAYOUT: Layout = Layout::new(4, 4);
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(C)]
struct Native {
    first: u32,
    second: u32,
}

impl Native {
    const LAYOUT: Layout = Layout::new(16, 16);
    const NATIVE_LAYOUT: bool = false;
}

#[test]
fn inherent_native_layout_cannot_enable_copying_padding() -> Result<(), Box<dyn Error>> {
    assert!(black_box(Padded::NATIVE_LAYOUT));
    assert!(!black_box(<Padded as Intrlay>::NATIVE_LAYOUT));
    assert!(!black_box(<Padded as Intrlay>::WRITES_ALL_BYTES));
    let source = Padded {
        first: 7,
        second: 11,
    };
    let view = PaddedView::create(&source)?;
    assert_eq!(view.as_bytes(), &[7, 0, 0, 0, 11, 0, 0, 0]);
    assert_eq!(Padded::from(view), source);
    let mut bytes = OwnedBuffer::new(compute_layout::<Padded>(&source)?)?;
    bytes.as_mut().fill(0xa5);
    let view = PaddedView::create_in_place(&source, bytes.as_mut())?;
    assert_eq!(view.as_bytes(), &[7, 0, 0, 0, 11, 0, 0, 0]);
    Ok(())
}

#[test]
fn inherent_layout_cannot_skip_trailing_padding_initialization() -> Result<(), Box<dyn Error>> {
    assert_eq!(Aligned::LAYOUT.size(), 4);
    assert_eq!(<Aligned as Intrlay>::LAYOUT.size(), 8);
    assert!(!black_box(<Aligned as Intrlay>::NATIVE_LAYOUT));
    assert!(!black_box(<Aligned as Intrlay>::WRITES_FIXED_LAYOUT));
    assert!(!black_box(<Aligned as Intrlay>::WRITES_ALL_BYTES));
    let source = Aligned { value: 7 };
    let view = AlignedView::create(&source)?;
    assert_eq!(view.as_bytes(), &[7, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(Aligned::from(view), source);
    Ok(())
}

#[test]
fn inherent_constants_do_not_disable_proven_native_fast_paths() -> Result<(), Box<dyn Error>> {
    assert_eq!(Native::LAYOUT.size(), 16);
    assert!(!black_box(Native::NATIVE_LAYOUT));
    assert_eq!(<Native as Intrlay>::LAYOUT.size(), 8);
    assert_eq!(
        black_box(<Native as Intrlay>::NATIVE_LAYOUT),
        cfg!(target_endian = "little")
    );
    assert!(black_box(<Native as Intrlay>::WRITES_FIXED_LAYOUT));
    assert!(black_box(<Native as Intrlay>::WRITES_ALL_BYTES));
    let source = Native {
        first: 7,
        second: 11,
    };
    let view = NativeView::create(&source)?;
    assert_eq!(view.as_bytes(), &[7, 0, 0, 0, 11, 0, 0, 0]);
    assert_eq!(Native::from(view), source);
    Ok(())
}

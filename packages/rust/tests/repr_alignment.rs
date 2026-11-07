//! Native alignment hints determine wire layout independently of their order.

use std::{error::Error, mem};

use intrlay::{
    Intrlay,
    schema::{Describe, Layouts, Registry},
};

#[derive(Intrlay)]
#[repr(C, align(32))]
#[repr(align(8))]
struct Decreasing {
    value: u64,
}

#[derive(Intrlay)]
#[repr(C, align(8))]
#[repr(align(32))]
struct Increasing {
    value: u64,
}

#[derive(Intrlay)]
#[repr(C, align(32), align(8))]
struct Combined {
    value: u64,
}

#[derive(Intrlay)]
#[repr(C, align(32))]
#[repr(align(8))]
enum Choice {
    Empty,
    Value(u64),
}

fn check_layout<C: Intrlay<Value = C> + Describe>(source: &C) -> Result<(), Box<dyn Error>> {
    assert_eq!(mem::align_of::<C>(), 32);
    assert_eq!(C::LAYOUT.alignment(), mem::align_of::<C>());
    assert_eq!(C::LAYOUT.size(), mem::size_of::<C>());
    assert_eq!(<[C; 2] as Intrlay>::LAYOUT.size(), 2 * mem::size_of::<C>());

    let encoded = intrlay::create::<C>(source)?;
    assert_eq!(encoded.as_ref().len(), C::LAYOUT.size());
    assert_eq!(encoded.as_ref().as_ptr() as usize % 32, 0);

    let mut registry = Registry::new();
    registry.export::<C>("Item");
    registry.export::<[C; 2]>("Pair");
    let schema = registry.finish();
    let definition = &schema.definitions[std::any::type_name::<C>()];
    assert_eq!(definition.alignment, 32);
    let mut layouts = Layouts::new(&schema);
    assert_eq!(layouts.of(&schema.exports["Item"])?, C::LAYOUT);
    assert_eq!(
        layouts.of(&schema.exports["Pair"])?,
        <[C; 2] as Intrlay>::LAYOUT
    );
    Ok(())
}

#[test]
fn all_alignment_hints_contribute_to_the_native_and_wire_layouts() -> Result<(), Box<dyn Error>> {
    check_layout(&Decreasing { value: 7 })?;
    check_layout(&Increasing { value: 11 })?;
    check_layout(&Combined { value: 13 })?;
    check_layout(&Choice::Empty)?;
    check_layout(&Choice::Value(17))?;
    Ok(())
}

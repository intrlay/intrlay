#[path = "../../../../target/compositions/bindings.rs"]
mod bindings;
use bindings::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nested = [
        intrlay::ZeroedSequence::new(2),
        intrlay::ZeroedSequence::new(0),
    ];
    let zero_input = ZeroedArraysInput {
        id: 73_u32,
        points: intrlay::ZeroedSequence::new(3),
        flags: intrlay::ZeroedSequence::new(4),
        matrix: intrlay::Zeroed,
        present: Some(intrlay::Zeroed),
        nested: nested.as_slice(),
        empty: intrlay::Zeroed,
    };
    let expected = std::fs::read("target/compositions/zeroed.bin")?;
    assert_eq!(
        intrlay::create_from::<ZeroedArrays>(&zero_input)?.as_ref(),
        expected
    );
    let mut names = RenamedListView::create(&RenamedList {
        items: vec![Renamed {
            first: 11,
            second: 22,
        }],
    })?;
    assert_eq!(names.items().get(0).ok_or("item")?.first(), 11);
    names
        .items_mut()
        .get_mut(0)
        .ok_or("item")?
        .second_mut()
        .set(23);
    assert_eq!(names.items().get(0).ok_or("item")?.second(), 23);
    let mut wrapped = WrappedScalarsView::create(&WrappedScalars {
        present: Some(1),
        absent: None,
        boxed: Box::new(2),
        nested: Some(Box::new(Some(3))),
        missing_inner: Some(Box::new(None)),
        items: vec![Some(Box::new(4)), None],
        fixed: [Some(5), None],
        read_only: Some(6),
        write_only: Some(7),
    })?;
    wrapped.present_mut().ok_or("present")?.set(11);
    wrapped.read_only_mut().ok_or("readonly")?.set(100);
    assert_eq!(wrapped.write_only(), Some(7));
    assert_eq!(wrapped.read_only(), Some(100));
    wrapped.boxed_mut().set(13);
    wrapped
        .nested_mut()
        .ok_or("nested")?
        .ok_or("inner")?
        .set(14);
    assert_eq!(wrapped.present(), Some(11));
    assert_eq!(wrapped.boxed(), 13);
    assert_eq!(wrapped.nested(), Some(Some(14)));
    assert_eq!(wrapped.absent(), None);
    assert_eq!(wrapped.missing_inner(), Some(None));
    println!("Rust generated composition regressions passed");
    Ok(())
}

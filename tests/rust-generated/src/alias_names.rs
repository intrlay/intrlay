#[path = "../../../target/regressions/alias-names.rs"]
mod generated;

use generated::{BatchView, MaybeView, NestedView, OwnedView, PairView, Second, SecondView, B};

pub fn run() {
    let mut batch = BatchView::create(&vec![B { value: 1 }]).unwrap();
    batch.value_mut().get_mut(0).unwrap().value_mut().set(2);
    assert_eq!(batch.value().get(0).unwrap().value(), 2);

    let maybe = MaybeView::create(&Some(B { value: 3 })).unwrap();
    assert_eq!(maybe.value().unwrap().value(), 3);
    let owned = OwnedView::create(&Box::new(B { value: 4 })).unwrap();
    assert_eq!(owned.value().value(), 4);
    let pair = PairView::create(&[B { value: 5 }, B { value: 6 }]).unwrap();
    assert_eq!(pair.value().get(1).unwrap().value(), 6);
    let nested = NestedView::create(&vec![Some(Box::new(B { value: 7 })), None]).unwrap();
    assert_eq!(nested.value().get(0).unwrap().unwrap().value(), 7);
    assert!(nested.value().get(1).unwrap().is_none());
    let second = SecondView::create(&Second { value: 8 }).unwrap();
    assert_eq!(second.value(), 8);

    let source = vec![B { value: 9 }];
    let layout = intrlay::compute_layout::<Vec<B>>(&source).unwrap();
    let mut bytes = intrlay::OwnedBuffer::new(layout).unwrap();
    let view = BatchView::create_in_place(&source, bytes.as_mut()).unwrap();
    assert_eq!(view.value().get(0).unwrap().value(), 9);
}

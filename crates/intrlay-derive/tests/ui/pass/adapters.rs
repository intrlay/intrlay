use wire::{Intrlay, schema::{Registry, Layouts}};

#[derive(Debug, PartialEq)]
struct External { private: u32 }
fn external_value(value: &External) -> u32 { value.private }
fn external_from(value: ExternalWire) -> External { External { private: value.value } }

#[derive(Intrlay)]
#[intrlay(remote = External, from = external_from)]
struct ExternalWire {
    #[intrlay(getter = external_value)]
    value: u32,
}

#[derive(Debug, PartialEq, Intrlay)]
struct Message {
    #[intrlay(with = ExternalWire)]
    external: External,
}

#[derive(Debug, PartialEq, Intrlay)]
struct Fixed<const N: usize> { values: [u32; N] }

fn main() {
    let source = Message { external: External { private: 42 } };
    let view = MessageView::create(&source).unwrap();
    let restored: Message = view.into();
    assert_eq!(source, restored);
    let mut registry = Registry::new();
    registry.export::<Message>("Message");
    Layouts::new(&registry.finish()).validate().unwrap();
    let source = Fixed::<3> { values: [1, 2, 3] };
    let restored: Fixed<3> = FixedView::create(&source).unwrap().into();
    assert_eq!(source, restored);
    println!("remote/from/getter/with and const generic array roundtrips passed");
}

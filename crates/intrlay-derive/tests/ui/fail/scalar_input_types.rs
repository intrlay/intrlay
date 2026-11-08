use wire::{BoxInput, EncodeSource, Intrlay, SequenceInput};

#[derive(Intrlay)]
struct Item {
    value: f32,
}

fn accepts<C: Intrlay>(_: &impl EncodeSource<C>) {}

fn main() {
    accepts::<bool>(&1_u8);
    accepts::<u64>(&1_u32);
    accepts::<Vec<u32>>(&vec![1_u16]);
    accepts::<[f32; 2]>(&[1_f64, 2.0]);
    accepts::<Vec<Item>>(&vec![ItemInput { value: 1_f64 }]);
    accepts::<Vec<u32>>(&SequenceInput::new([1_u16].into_iter()));
    accepts::<Option<Box<u32>>>(&Some(BoxInput(1_u16)));
}

#[derive(wire::Intrlay)]
#[repr(u8)]
enum Value { First = 4u8, #[intrlay(tag = 4)] Second = 7u8 }
fn main() {}

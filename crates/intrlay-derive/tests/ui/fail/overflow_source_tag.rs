const TAG: u64 = 4_294_967_296;
#[derive(wire::Intrlay)]
#[repr(u64)]
enum Value { TooLarge = TAG }
fn main() {}

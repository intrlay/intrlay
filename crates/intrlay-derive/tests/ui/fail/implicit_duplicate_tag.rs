#[derive(wire::Intrlay)]
enum Value { #[intrlay(tag=4)] A, B, #[intrlay(tag=5)] C }
fn main() {}

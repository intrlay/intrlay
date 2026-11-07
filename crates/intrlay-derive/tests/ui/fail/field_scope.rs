#[derive(wire::Intrlay)]
struct Value { #[intrlay(from=does_not_exist)] value: u32 }
fn main() {}

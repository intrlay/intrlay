#[derive(wire::Intrlay)]
struct Value { #[intrlay(create=false)] value: u32 }
fn main() {}

#[derive(wire::Intrlay)]
enum Value { #[intrlay(tag=u32::MAX)] A, B }
fn main() {}

#[derive(wire::Intrlay)]
struct Value { #[intrlay(project = "fields", project = "indexed")] values: Vec<u32> }
fn main() {}

#[derive(wire::Intrlay)]
#[intrlay(generate_setters=false, generate_setters=true)]
struct Value(u32);
fn main() {}

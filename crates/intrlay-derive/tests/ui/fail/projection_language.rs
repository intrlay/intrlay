#[derive(wire::Intrlay)]
#[intrlay(javascript(generate_getters=false))]
struct Value(u32);
fn main() {}

const TAG: u32 = 4;
#[derive(wire::Intrlay)]
enum Value { #[intrlay(tag=TAG)] A, #[intrlay(tag=2+2)] B }
fn main() {}

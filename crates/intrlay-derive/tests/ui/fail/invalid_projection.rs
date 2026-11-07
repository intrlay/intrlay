#[derive(wire::Intrlay)]
#[intrlay(project = "bool")]
struct Boolean(bool);

#[derive(wire::Intrlay)]
#[intrlay(project = "quaternion")]
struct Unknown([f32; 4]);

fn main() {}

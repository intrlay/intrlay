#[derive(wire::Intrlay)]
#[intrlay(typescript(typed_array = "f32"))]
struct TypeScript(f32);

#[derive(wire::Intrlay)]
#[intrlay(python(typed_array = "f32"))]
struct Python(f32);

#[derive(wire::Intrlay)]
#[intrlay(dart(typed_array = "f32"))]
struct Dart(f32);

#[derive(wire::Intrlay)]
#[intrlay(typescript(fields))]
struct Fields(f32);

#[derive(wire::Intrlay)]
struct Indexed {
    #[intrlay(python(indexed))]
    items: Vec<f32>,
}

fn main() {}

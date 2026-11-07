use wire::Intrlay;

#[derive(Intrlay)]
#[intrlay(rust(project = "f32"))]
struct RustProjection(f32);

#[derive(Intrlay)]
#[intrlay(cpp(project = "f32"))]
struct CppProjection(f32);

#[derive(Intrlay)]
#[intrlay(csharp(project = "f32"))]
struct CSharpProjection(f32);

#[derive(Intrlay)]
struct TypeScriptIndexed {
    #[intrlay(typescript(project = "indexed"))]
    items: Vec<f32>,
}

#[derive(Intrlay)]
struct PythonIndexed {
    #[intrlay(python(project = "indexed"))]
    items: Vec<f32>,
}

#[derive(Intrlay)]
struct DartIndexed {
    #[intrlay(dart(project = "indexed"))]
    items: Vec<f32>,
}

fn main() {}

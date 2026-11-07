#[derive(wire::Intrlay)]
#[intrlay(rust(generate_getters = false))]
struct Rust { value: u32 }

#[derive(wire::Intrlay)]
#[intrlay(cpp(generate_setters = false))]
struct Cpp { value: u32 }

#[derive(wire::Intrlay)]
#[intrlay(csharp(generate_setters = false))]
struct CSharp { value: u32 }

fn main() {}

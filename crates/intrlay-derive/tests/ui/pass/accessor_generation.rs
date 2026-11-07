#[derive(wire::Intrlay)]
#[intrlay(generate_getters = false, generate_setters = false)]
struct Child { value: u32 }

#[derive(wire::Intrlay)]
struct Parent {
    #[intrlay(generate_setters = false)]
    child: Child,
    #[intrlay(generate_getters = false)]
    scalar: u32,
}

fn main() {
    let mut view = ParentView::create(&Parent { child: Child { value: 1 }, scalar: 2 }).unwrap();
    view.child_mut().value_mut().set(3);
    view.scalar_mut().set(4);
    assert_eq!(view.child().value(), 3);
    assert_eq!(view.scalar(), 4);
    let mut registry = wire::schema::Registry::default();
    registry.export::<Parent>("Parent");
    let schema = registry.finish();
    let child = schema.definitions.values().find(|definition| definition.name == "Child").unwrap();
    for language in [wire::schema::Language::Rust, wire::schema::Language::Cpp, wire::schema::Language::CSharp] {
        assert!(!child.targets.contains_key(&language));
    }
    for language in [wire::schema::Language::TypeScript, wire::schema::Language::Python, wire::schema::Language::Dart] {
        assert_eq!(child.targets[&language].generate_getters, Some(false));
        assert_eq!(child.targets[&language].generate_setters, Some(false));
    }
}

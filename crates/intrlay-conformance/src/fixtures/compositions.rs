//! Feature combinations compiled and executed by tests/compositions consumers.

use super::Fixtures;

use intrlay::{
    Intrlay,
    schema::{Language, NameCase, Registry},
};
use intrlay_codegen::{
    GenerateOptions, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility,
    generate, generate_with_options,
};
use std::{error::Error, path::Path};

#[derive(Intrlay)]
#[allow(
    non_snake_case,
    reason = "exercise source names that collide after casing"
)]
struct Renamed {
    #[intrlay(name = "first")]
    foo_bar: u32,
    #[intrlay(name = "second")]
    fooBar: u32,
}

#[derive(Intrlay)]
struct RenamedList {
    items: Vec<Renamed>,
}

#[derive(Intrlay)]
struct IndexedCollision {
    #[intrlay(name = "some_name")]
    first: u32,
    #[intrlay(name = "someName")]
    second: u32,
}

#[derive(Intrlay)]
#[repr(C, align(16))]
#[intrlay(transparent, project = "f32")]
struct PaddedScalar(f32);

#[derive(Intrlay)]
struct ProjectionSet {
    direct: PaddedScalar,
    optional: Option<PaddedScalar>,
    boxed: Box<PaddedScalar>,
    elements: Vec<PaddedScalar>,
    #[intrlay(generate_setters = false)]
    read_only: Option<PaddedScalar>,
}

#[derive(Intrlay)]
struct WrappedScalars {
    present: Option<u32>,
    absent: Option<u32>,
    boxed: Box<u32>,
    nested: Option<Box<Option<u32>>>,
    missing_inner: Option<Box<Option<u32>>>,
    items: Vec<Option<Box<u32>>>,
    fixed: [Option<u32>; 2],
    #[intrlay(generate_setters = false)]
    read_only: Option<u32>,
    #[intrlay(generate_getters = false)]
    write_only: Option<u32>,
}

#[derive(Intrlay)]
enum CachedChoice {
    Empty,
    Value { child: Option<Shared> },
}

#[derive(Intrlay)]
struct CachedChildren {
    child: Option<Shared>,
    absent: Option<Shared>,
    boxed: Box<Shared>,
    #[allow(
        clippy::option_option,
        reason = "exercise distinct nested presence states"
    )]
    nested: Option<Option<Shared>>,
    choice: CachedChoice,
}

#[derive(Intrlay)]
struct Shared {
    value: u32,
}

#[derive(Intrlay)]
struct SharedAlpha {
    shared: Shared,
}

#[derive(Intrlay)]
struct SharedBeta {
    shared: Shared,
}

#[derive(Intrlay)]
enum Collision {
    Tag { value: u32 },
    Other,
}

#[derive(Intrlay)]
enum RenamedVariant {
    #[intrlay(csharp(name = "Tagged"))]
    Tag {
        value: u32,
    },
    Other,
}

#[derive(Intrlay)]
struct FourFloats {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Intrlay)]
enum CompactUnion {
    A(FourFloats),
    B(FourFloats),
    C(FourFloats),
    D(FourFloats),
    E(FourFloats),
    F(FourFloats),
    G(FourFloats),
    H(FourFloats),
}

#[derive(Intrlay)]
enum ReferenceUnion {
    A(String),
    B(String),
    C(String),
    D(String),
    E(String),
    F(String),
    G(String),
    H(String),
}

#[derive(Intrlay)]
enum NestedChoice {
    Label(String),
    Rotation(FourFloats),
    Values(Vec<u32>),
}

#[derive(Intrlay)]
struct ManagedPayload {
    label: String,
    samples: Vec<f32>,
    counts: Vec<u32>,
    optional: Option<NestedChoice>,
}

#[derive(Intrlay)]
enum ManagedUnion {
    Empty,
    Floats { values: Vec<f32> },
    Numbers { values: Vec<u32> },
    Record(ManagedPayload),
    Nested { value: Box<Self> },
}

#[derive(Intrlay)]
enum Storage {
    Value {
        #[intrlay(csharp(name = "storage"))]
        storage: u32,
        #[intrlay(csharp(name = "storage2"))]
        second: u32,
    },
}

#[derive(Intrlay)]
enum NumericData {
    Number(u64),
    Composite(Storage),
}

#[derive(Intrlay)]
enum HelperNames {
    Storage(Storage),
    NumericData(NumericData),
}

#[derive(Intrlay)]
enum PrivateHelper {
    #[intrlay(csharp(name = "_storage"))]
    Value(u32),
}

#[derive(Intrlay)]
struct Position {
    position: u32,
}

#[derive(Intrlay)]
enum PayloadCollision {
    Item { payload_collision_item: u32 },
}

#[derive(Intrlay)]
struct ViewCollision {
    #[intrlay(csharp(name = "ViewCollisionView"))]
    value: u32,
}

#[derive(Intrlay)]
struct RenamedPosition {
    #[intrlay(csharp(name = "Coordinate"))]
    renamed_position: u32,
}

#[derive(Intrlay)]
enum Buffers {
    Empty,
    Pair { a: Vec<f32>, b: Vec<f32> },
}

#[derive(Intrlay)]
enum NestedBuffers {
    Inner {
        #[intrlay(csharp(name = "storage"))]
        buffers: Buffers,
        #[intrlay(csharp(name = "payload"))]
        second: Buffers,
        #[intrlay(csharp(name = "payload2"))]
        number: u32,
    },
}

#[derive(Intrlay)]
struct SetterNames {
    #[intrlay(dart(generate_getters = false))]
    value: u32,
    set_value: u32,
}

#[derive(Intrlay)]
struct WrappedSetterNames {
    value: Option<Box<u32>>,
    set_value: u32,
}

#[derive(Intrlay)]
struct Root {
    root_bindings: Vec<u32>,
}

#[derive(Intrlay)]
struct NumericItem {
    id: u32,
    position: [f32; 3],
    matrix: [[f32; 2]; 2],
    padded: PaddedScalar,
    #[intrlay(project = "f32")]
    weights: Vec<f32>,
    wide: [u64; 2],
    #[intrlay(generate_setters = false)]
    read_only: [f32; 2],
    #[intrlay(generate_getters = false)]
    write_only: [i16; 2],
}

#[derive(Intrlay)]
struct NumericItems {
    items: Vec<NumericItem>,
}

#[derive(Intrlay)]
struct ComponentCollision {
    position: [f32; 3],
    position_component: f32,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
struct ReadElement {
    value: f32,
}

#[derive(Intrlay)]
#[intrlay(create = false, generate_setters = false)]
struct ReadFrame {
    values: Vec<ReadElement>,
}

#[test]
fn anonymous_collections_inherit_code_size_hints() -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<ReadFrame>("ReadFrame");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let source = generate(&schema, language)?;
        for absent in [
            "_write",
            "_measure",
            "createInPlace",
            "create_in_place",
            "setValue",
            "set_value",
        ] {
            assert!(!source.contains(absent), "{language:?}: {absent}");
        }
    }
    let mut registry = Registry::new();
    registry.export::<ReadFrame>("ReadFrame");
    registry.export::<Vec<ReadElement>>("Standalone");
    let source = generate(&registry.finish(), Language::TypeScript)?;
    assert!(source.contains("public static createInPlace"));
    assert!(source.contains("_writeReadElement"));
    Ok(())
}

#[test]
fn numeric_component_names_are_validated() {
    let mut registry = Registry::new();
    registry.export::<Vec<ComponentCollision>>("Items");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        assert!(
            matches!(generate(&schema, language), Err(error) if error.0.contains("indexed numeric component accessor"))
        );
    }
}

#[derive(Intrlay)]
#[intrlay(name = "Point")]
struct UpperHelper {
    number: u32,
}

#[derive(Intrlay)]
#[intrlay(
    typescript(name = "point"),
    python(name = "point"),
    dart(name = "point"),
    cpp(name = "point"),
    csharp(name = "point")
)]
struct LowerHelper {
    text: String,
}

#[derive(Intrlay)]
struct HelperCollision {
    upper: UpperHelper,
    lower: LowerHelper,
}

#[derive(Intrlay)]
enum VariantSource {
    Payload(u32),
}

#[derive(Intrlay)]
#[intrlay(rust(name = "VariantSourceVariantView"))]
struct VariantConflict {
    value: u32,
}

#[derive(Intrlay)]
struct PythonConstructionName {
    #[intrlay(python(name = "__new__"))]
    value: u32,
}

#[derive(Intrlay)]
struct PythonStorageName {
    #[intrlay(python(name = "__getattribute__"))]
    value: u32,
}

#[test]
fn reject_helper_names_after_final_casing() {
    let mut registry = Registry::new();
    registry.export::<HelperCollision>("HelperCollision");
    let schema = registry.finish();
    for language in [
        Language::TypeScript,
        Language::Python,
        Language::Dart,
        Language::CSharp,
        Language::Cpp,
    ] {
        let error = generate(&schema, language)
            .err()
            .unwrap_or_else(|| panic!("accepted helper collision in {language:?}"));
        assert!(
            error.0.contains("codec helper") || error.0.contains("wire descriptor"),
            "{error}"
        );
    }
    let mut registry = Registry::new();
    registry.export::<VariantSource>("VariantSource");
    registry.export::<VariantConflict>("VariantConflict");
    assert!(
        matches!(generate(&registry.finish(), Language::Rust), Err(error) if error.0.contains("variant view"))
    );
}

#[test]
fn reject_python_object_protocol_properties() {
    let mut registry = Registry::new();
    registry.export::<PythonConstructionName>("PythonConstructionName");
    assert!(
        matches!(generate(&registry.finish(), Language::Python), Err(error) if error.0.contains("__new__"))
    );
    let mut registry = Registry::new();
    registry.export::<PythonStorageName>("PythonStorageName");
    assert!(
        matches!(generate(&registry.finish(), Language::Python), Err(error) if error.0.contains("__getattribute__"))
    );
}

fn generate_composition_fixtures(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("compositions");

    let mut registry = Registry::new();
    registry.export::<RenamedList>("RenamedList");
    registry.export::<RenamedVariant>("RenamedVariant");
    registry.export::<ProjectionSet>("ProjectionSet");
    registry.export::<WrappedScalars>("WrappedScalars");
    registry.export::<CachedChildren>("CachedChildren");
    registry.export::<Option<Box<Option<u32>>>>("OptionalRoot");
    registry.export::<Option<Shared>>("CachedRoot");
    registry.export::<CompactUnion>("CompactUnion");
    registry.export::<ReferenceUnion>("ReferenceUnion");
    registry.export::<ManagedUnion>("ManagedUnion");
    registry.export::<Vec<()>>("UnitSequence");
    registry.export::<NumericItems>("NumericItems");
    let schema = registry.finish();
    let native_value = ManagedUnion::Record(ManagedPayload {
        label: "shared".into(),
        samples: vec![1.0, 2.0],
        counts: vec![3, 4],
        optional: Some(NestedChoice::Values(vec![5, 6])),
    });
    fixtures.insert(
        directory.join("managed-union.bin"),
        ManagedUnionView::create(&native_value)?.as_bytes(),
    )?;
    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::Cpp, "hpp"),
        (Language::CSharp, "cs"),
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("bindings.{extension}")),
            generate_with_options(
                &schema,
                language,
                GenerateOptions {
                    cpp_namespace: Some("composition".into()),
                    ..Default::default()
                },
            )?,
        )?;
    }
    csharp_modules(fixtures, &directory)?;
    csharp_unions(fixtures, &directory)?;
    let mut registry = Registry::new();
    registry.export::<SetterNames>("SetterNames");
    fixtures.insert(
        directory.join("setter-names.dart"),
        generate(&registry.finish(), Language::Dart)?,
    )?;
    Ok(())
}

fn csharp_unions(fixtures: &mut Fixtures, directory: &Path) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<HelperNames>("HelperNames");
    registry.export::<PrivateHelper>("PrivateHelper");
    registry.export::<RenamedPosition>("RenamedPosition");
    registry.export::<NestedBuffers>("NestedBuffers");
    fixtures.insert(
        directory.join("native-unions.cs"),
        generate_with_options(
            &registry.finish(),
            Language::CSharp,
            GenerateOptions {
                csharp_namespace: Some("Composition.Regression".into()),
                ..Default::default()
            },
        )?,
    )?;
    fixtures.insert(
        directory.join("nested-buffers.bin"),
        NestedBuffersView::create(&NestedBuffers::Inner {
            buffers: Buffers::Pair {
                a: vec![10.0, 20.0],
                b: vec![10.0, 20.0],
            },
            second: Buffers::Empty,
            number: 7,
        })?
        .as_bytes(),
    )?;
    Ok(())
}

fn csharp_modules(fixtures: &mut Fixtures, directory: &Path) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Root>("Root");
    fixtures.insert(
        directory.join("binding-name.cs"),
        generate_with_options(
            &registry.finish(),
            Language::CSharp,
            GenerateOptions {
                csharp_namespace: Some("Composition.BindingName".into()),
                ..Default::default()
            },
        )?,
    )?;
    for (name, registry) in [
        ("Alpha", {
            let mut registry = Registry::new();
            registry.export::<SharedAlpha>("SharedAlpha");
            registry
        }),
        ("Beta", {
            let mut registry = Registry::new();
            registry.export::<SharedBeta>("SharedBeta");
            registry
        }),
    ] {
        fixtures.insert(
            directory.join(format!("shared-{name}.cs")),
            generate_with_options(
                &registry.finish(),
                Language::CSharp,
                GenerateOptions {
                    csharp_namespace: Some(format!("Composition.{name}")),
                    ..Default::default()
                },
            )?,
        )?;
    }
    let mut registry = Registry::new();
    registry.export::<Shared>("Shared");
    let mut policy = NamingPolicy::for_language(Language::CSharp);
    policy.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Field),
            visibility: Some(Visibility::Private),
            ..Default::default()
        },
        style: NameStyle::new(NameCase::CamelCase),
    });
    fixtures.insert(
        directory.join("naming.cs"),
        generate_with_options(
            &registry.finish(),
            Language::CSharp,
            GenerateOptions {
                csharp_namespace: Some("Composition.Naming".into()),
                naming: Some(policy),
                ..Default::default()
            },
        )?,
    )?;
    Ok(())
}

#[test]
fn reject_native_enum_collisions_and_invalid_namespaces() {
    let mut registry = Registry::new();
    registry.export::<Collision>("Collision");
    assert!(
        matches!(generate(&registry.finish(), Language::CSharp), Err(error) if error.0.contains("enum discriminant"))
    );
    let mut registry = Registry::new();
    registry.export::<Shared>("Shared");
    let schema = registry.finish();
    for namespace in ["", ".", "a..b", "a.class", "a.1", "a;b"] {
        assert!(
            matches!(generate_with_options(&schema, Language::CSharp, GenerateOptions { csharp_namespace: Some(namespace.into()), ..Default::default() }), Err(error) if error.0.contains("invalid C# namespace"))
        );
    }
}

#[test]
fn reject_indexed_names_that_collide_after_method_casing() {
    let mut registry = Registry::new();
    registry.export::<Vec<IndexedCollision>>("Items");
    let schema = registry.finish();
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        assert!(
            matches!(generate(&schema, language), Err(error) if error.0.contains("indexed field accessor"))
        );
    }
}

#[test]
fn reject_csharp_members_with_the_enclosing_type_name() {
    for schema in [
        {
            let mut registry = Registry::new();
            registry.export::<Position>("Position");
            registry.finish()
        },
        {
            let mut registry = Registry::new();
            registry.export::<PayloadCollision>("PayloadCollision");
            registry.finish()
        },
        {
            let mut registry = Registry::new();
            registry.export::<ViewCollision>("ViewCollision");
            registry.finish()
        },
    ] {
        assert!(
            matches!(generate(&schema, Language::CSharp), Err(error) if error.0.contains("enclosing") && error.0.contains("assign an explicit target name"))
        );
    }
}

#[test]
fn reject_dart_wrapped_scalar_setter_collisions() {
    let mut registry = Registry::new();
    registry.export::<WrappedSetterNames>("WrappedSetterNames");
    assert!(
        matches!(generate(&registry.finish(), Language::Dart), Err(error) if error.0.contains("setValue") && error.0.contains("field setter"))
    );
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_composition_fixtures(fixtures)?;
    Ok(())
}

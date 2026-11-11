//! Compiler-selected adapter schemas preserve ordinary recursive wire types.

mod common;

use std::{error::Error, path::PathBuf};

use intrlay_frontend_cpp::{Options, extract};
use intrlay_schema::{DefinitionKind, Language, Scalar, Type};

fn options() -> Options {
    Options {
        arguments: vec![format!(
            "-I{}",
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/cpp/include")
                .display()
        )],
        ..Options::default()
    }
}

#[test]
fn adapters_resolve_recursively_and_preserve_schema_metadata() -> Result<(), Box<dyn Error>> {
    let schema = extract(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapters.hpp"),
        &options(),
    )?;
    assert_eq!(schema.exports["Scalar"], Type::Scalar(Scalar::F32));
    let flat = common::definition(&schema, "FlattenedBounds");
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        assert_eq!(
            flat.targets[&language].exposure,
            Some(intrlay_schema::Exposure::Flatten)
        );
    }

    let Type::Named(endpoints) = &schema.exports["Endpoints"] else {
        return Err("expected endpoints".into());
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[endpoints].kind else {
        return Err("expected endpoint alternatives".into());
    };
    assert_eq!(variants[0].name, "Start");
    assert_eq!(variants[1].name, "End");
    let DefinitionKind::Record(fields) = &common::definition(&schema, "Message").kind else {
        return Err("expected message".into());
    };
    assert_eq!(
        fields[1].ty,
        Type::Sequence(Box::new(common::named(&schema, "Position")))
    );
    assert_eq!(
        fields[1].targets[&Language::TypeScript].exposure,
        Some(intrlay_schema::Exposure::TypedArray(Scalar::F32))
    );
    assert_eq!(
        fields[1].targets[&Language::Python].exposure,
        fields[1].targets[&Language::TypeScript].exposure
    );
    assert_eq!(fields[0].ty, common::named(&schema, "Pose"));
    let pose = common::definition(&schema, "Pose");
    let DefinitionKind::Record(pose) = &pose.kind else {
        return Err("expected pose".into());
    };
    assert_eq!(pose[0].docs, "Position in world units.");
    assert_eq!(pose[1].ty, Type::String);
    let tree = common::definition(&schema, "Tree");
    let DefinitionKind::Record(fields) = &tree.kind else {
        return Err("expected tree".into());
    };
    assert_eq!(
        fields[1].ty,
        Type::Sequence(Box::new(common::named(&schema, "Tree")))
    );
    Ok(())
}

#[test]
fn invalid_adapter_schema_is_a_frontend_error() -> Result<(), Box<dyn Error>> {
    let cases = [
        (
            "struct A {}; template<> struct intrlay::Adapter<A> { using schema_type = A[2]; }; using Root INTRLAY() = A;",
            "recursive adapter schema requires a named record or enum",
        ),
        (
            "#include <vector>\nstruct A {}; template<> struct intrlay::Adapter<A> { using schema_type = std::vector<A>; }; using Root INTRLAY() = A;",
            "recursive adapter schema requires a named record or enum",
        ),
        (
            "struct A {}; struct B {}; template<> struct intrlay::Adapter<A> { using schema_type = B; }; template<> struct intrlay::Adapter<B> { using schema_type = A; }; using Root INTRLAY() = A;",
            "cyclic adapter schema_type",
        ),
        (
            "struct A {}; template<> struct intrlay::Adapter<A> {}; using Root INTRLAY() = A;",
            "schema_type",
        ),
        (
            "struct A {}; template<> struct intrlay::Adapter<A> { using schema_type = void; }; using Root INTRLAY() = A;",
            "sizeof",
        ),
    ];
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    for (source, diagnostic) in cases {
        std::fs::write(
            &path,
            format!("#include <intrlay/annotations.hpp>\n{source}"),
        )?;
        let error = extract(&path, &options())
            .err()
            .ok_or("invalid adapter accepted")?;
        assert!(error.to_string().contains(diagnostic), "{error}");
    }
    Ok(())
}

#[test]
fn unrelated_adapters_preserve_public_type_paths() -> Result<(), Box<dyn Error>> {
    // All of these have usable public paths but their canonical display names
    // contain anonymous scopes, source locations, or inaccessible type names.
    let cases = [
        "namespace outer { namespace { struct Child { int value; }; } using Root INTRLAY() = Child; }",
        "namespace outer { namespace { struct INTRLAY() Root { int value; }; } }",
        "namespace outer { inline namespace version { struct INTRLAY() Root { int value; }; } }",
        "namespace outer { namespace { namespace inner { struct INTRLAY() Root { int value; }; } } namespace inner {} }",
        "namespace outer { namespace { namespace inner { struct Local { float value; }; } } namespace inner { struct INTRLAY() Root { int value; }; } }",
        "extern \"C\" { struct INTRLAY() Root { int value; }; }",
        "extern \"C\" { typedef int Root INTRLAY(); }",
        "namespace { struct INTRLAY(name=\"Local\") Item { int local; }; } struct INTRLAY() Item { float global; };",
        "struct INTRLAY() Item { int value; }; inline int Item = 3;",
        "namespace types { struct A { int value; }; template<class> struct Box; template<> struct Box<A> { A child; }; using Root INTRLAY() = Box<A>; }",
        "class Wrapper { struct Hidden { int value; }; public: Hidden child; }; using Root INTRLAY() = Wrapper;",
        "struct INTRLAY() Root { struct { int value; } child; };",
        "class Wrapper { struct Hidden { int value; }; public: using Public = Hidden; }; using Root INTRLAY() = Wrapper::Public;",
        r"
            class Wrapper {
                template<int> struct Hidden { int value; };
                static_assert(sizeof(Hidden<1>) + sizeof(Hidden<2>) + sizeof(Hidden<3>)
                    + sizeof(Hidden<4>) + sizeof(Hidden<5>) + sizeof(Hidden<6>) > 0);
            public:
                std::vector<Hidden<1>> vector;
                std::optional<Hidden<2>> optional;
                std::unique_ptr<const Hidden<3>> boxed;
                std::array<Hidden<4>, 2> fixed;
                const Hidden<5> raw[2];
                std::variant<std::monostate, Hidden<6>> choice;
            };
            using Root INTRLAY() = Wrapper;
        ",
    ];
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    let plain = options();
    let mut adapted = options();
    adapted.arguments.push("-DENABLE_ADAPTER".into());
    for source in cases {
        std::fs::write(
            &path,
            format!(
                r"#include <intrlay/annotations.hpp>
                #include <array>
                #include <memory>
                #include <optional>
                #include <variant>
                #include <vector>
                {source}
                #ifdef ENABLE_ADAPTER
                struct Other {{}};
                template<> struct intrlay::Adapter<Other> {{ using schema_type = int; }};
                #endif
                "
            ),
        )?;
        assert_eq!(extract(&path, &plain)?, extract(&path, &adapted)?);
    }
    Ok(())
}

#[test]
fn adapters_can_expose_unnamable_native_and_schema_types() -> Result<(), Box<dyn Error>> {
    let cases = [
        r"
            namespace { struct Native { float value; }; }
            INTRLAY_ADAPT_STRUCT(Native, Value, value);
            using Root INTRLAY() = Native;
        ",
        r"
            struct Native { float value; };
            template<> class intrlay::Adapter<Native> {
                struct Value { float value; };
            public:
                using schema_type = Value;
            };
            using Root INTRLAY() = Native;
        ",
    ];
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    for source in cases {
        std::fs::write(
            &path,
            format!("#include <intrlay/annotations.hpp>\n{source}"),
        )?;
        let schema = extract(&path, &options())?;
        let Type::Named(identity) = &schema.exports["Root"] else {
            return Err("expected adapted record".into());
        };
        let definition = &schema.definitions[identity];
        assert_eq!(definition.name, "Value");
        let DefinitionKind::Record(fields) = &definition.kind else {
            return Err("expected adapted fields".into());
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "value");
        assert_eq!(fields[0].ty, Type::Scalar(Scalar::F32));
    }
    Ok(())
}

#[test]
fn same_display_name_does_not_merge_distinct_adapters() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    std::fs::write(
        &path,
        r"#include <intrlay/annotations.hpp>
        inline auto make_integer() { struct Value { int value; }; return Value{}; }
        inline auto make_float() { struct Value { float value; }; return Value{}; }
        using Integer INTRLAY() = decltype(make_integer());
        using Float INTRLAY() = decltype(make_float());
        template<> struct intrlay::Adapter<Integer> { using schema_type = int; };
        template<> struct intrlay::Adapter<Float> { using schema_type = float; };
        ",
    )?;
    let schema = extract(&path, &options())?;
    assert_eq!(schema.exports["Integer"], Type::Scalar(Scalar::I32));
    assert_eq!(schema.exports["Float"], Type::Scalar(Scalar::F32));
    Ok(())
}

#[test]
fn local_record_aliases_are_distinct_without_adapters() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    std::fs::write(
        &path,
        r#"#include <intrlay/annotations.hpp>
        inline auto records() {
            struct INTRLAY(name="IntRecord") Value { int value; };
            using Integer = Value;
            {
                struct INTRLAY(name="FloatRecord") Value { float value; };
                struct Pair { Integer first; Value second; };
                return Pair{};
            }
        }
        using Integer INTRLAY() = decltype(records().first);
        using Float INTRLAY() = decltype(records().second);
        "#,
    )?;
    let schema = extract(&path, &options())?;
    assert_ne!(schema.exports["Integer"], schema.exports["Float"]);
    assert_eq!(schema.definitions.len(), 2);
    for (name, scalar) in [("IntRecord", Scalar::I32), ("FloatRecord", Scalar::F32)] {
        let DefinitionKind::Record(fields) = &common::definition(&schema, name).kind else {
            return Err("expected local record".into());
        };
        assert_eq!(fields[0].ty, Type::Scalar(scalar));
    }
    Ok(())
}

#[test]
fn local_schema_records_and_variants_keep_distinct_identities() -> Result<(), Box<dyn Error>> {
    let schema = extract(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapters.hpp"),
        &options(),
    )?;
    assert_eq!(
        schema.exports["LocalInteger"],
        schema.exports["IntegerAlias"]
    );
    assert_ne!(schema.exports["LocalInteger"], schema.exports["LocalFloat"]);
    assert_ne!(
        schema.exports["LocalIntegerChoices"],
        schema.exports["LocalFloatChoices"]
    );
    for (name, scalar, alternatives) in [
        (
            "LocalInteger",
            Scalar::I32,
            ["FirstInteger", "SecondInteger"],
        ),
        ("LocalFloat", Scalar::F32, ["FirstFloat", "SecondFloat"]),
    ] {
        let DefinitionKind::Record(fields) = &common::definition(&schema, name).kind else {
            return Err("expected local schema record".into());
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].ty, Type::Scalar(scalar));
        let Type::Named(choice) = &schema.exports[&format!("{name}Choices")] else {
            return Err("expected local variant".into());
        };
        let DefinitionKind::Enum(variants) = &schema.definitions[choice].kind else {
            return Err("expected variant definition".into());
        };
        assert_eq!(variants.len(), 2);
        for (variant, expected_name) in variants.iter().zip(alternatives) {
            assert_eq!(variant.name, expected_name);
            assert_eq!(variant.fields[0].ty, schema.exports[name]);
        }
    }
    Ok(())
}

#[test]
fn structural_template_arguments_preserve_types_aliases_and_variants() -> Result<(), Box<dyn Error>>
{
    let schema = extract(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapters.hpp"),
        &options(),
    )?;
    assert_eq!(
        schema.exports["SamplesOne"],
        schema.exports["SamplesOneAlias"]
    );
    assert_eq!(
        schema.exports["SamplesOne"],
        schema.exports["ConstSamplesOne"]
    );
    assert_ne!(schema.exports["SamplesOne"], schema.exports["SamplesTwo"]);
    for (name, length) in [("SamplesOne", 1), ("SamplesTwo", 2), ("SamplesThree", 3)] {
        let Type::Named(identity) = &schema.exports[name] else {
            return Err("expected structural specialization".into());
        };
        let DefinitionKind::Record(fields) = &schema.definitions[identity].kind else {
            return Err("expected structural record".into());
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(
            fields[0].ty,
            Type::Array {
                element: Box::new(Type::Scalar(Scalar::F32)),
                length
            },
        );
    }
    let Type::Named(identity) = &schema.exports["SampleChoices"] else {
        return Err("expected structural variant".into());
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[identity].kind else {
        return Err("expected variant definition".into());
    };
    for (variant, name, payload) in [
        (&variants[0], "Single", "SamplesOne"),
        (&variants[1], "Pair", "SamplesTwo"),
    ] {
        assert_eq!(variant.name, name);
        assert_eq!(variant.fields[0].ty, schema.exports[payload]);
    }
    Ok(())
}

#[test]
fn adapters_do_not_turn_unbounded_array_owners_into_single_objects() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    std::fs::write(
        &path,
        r"#include <intrlay/annotations.hpp>
        #include <memory>
        struct Other {};
        template<> struct intrlay::Adapter<Other> { using schema_type = int; };
        struct INTRLAY() Root { std::unique_ptr<int[]> values; };
        ",
    )?;
    let error = extract(&path, &options())
        .err()
        .ok_or("unbounded array owner accepted as a single object")?;
    assert!(error.0.contains("IncompleteArray"), "{error}");
    Ok(())
}

#[test]
fn complex_declarations_require_concrete_export_aliases() -> Result<(), Box<dyn Error>> {
    let cases = [
        "struct Owner { struct Value; }; struct INTRLAY(name=\"Root\") Owner::Value { int value; };",
        "struct Owner { enum class Value; }; enum class INTRLAY(name=\"Root\") Owner::Value { First = 1 };",
        "template<class> struct Value; template<> struct INTRLAY(name=\"Root\") Value<int> { int value; };",
    ];
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    for declaration in cases {
        std::fs::write(
            &path,
            format!(
                "#include <intrlay/annotations.hpp>\n{declaration}\n\
                 #ifdef ENABLE_ADAPTER\nstruct Other {{}};\n\
                 template<> struct intrlay::Adapter<Other> {{ using schema_type = int; }};\n#endif\n"
            ),
        )?;
        for enabled in [false, true] {
            let mut options = options();
            if enabled {
                options.arguments.push("-DENABLE_ADAPTER".into());
            }
            let error = extract(&path, &options)
                .err()
                .ok_or("complex direct export accepted")?;
            assert!(
                error.0.contains("annotated namespace-scope type alias"),
                "{error}"
            );
        }
    }
    Ok(())
}

#[test]
fn concrete_aliases_export_nested_and_specialized_types() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    std::fs::write(
        &path,
        r"#include <intrlay/annotations.hpp>
        struct Model { struct Payload; enum class Kind; };
        struct Model::Payload { int value; };
        enum class Model::Kind { First = 1, Last = 3 };
        using Payload INTRLAY() = Model::Payload;
        using Kind INTRLAY() = Model::Kind;
        template<class> struct Storage;
        template<> struct Storage<int> { int value; };
        using IntegerStorage INTRLAY() = Storage<int>;
        #ifdef ENABLE_ADAPTER
        template<> struct intrlay::Adapter<Model::Payload> { using schema_type = float; };
        #endif
        ",
    )?;
    let mut options = options();
    let plain = extract(&path, &options)?;
    options.arguments.push("-DENABLE_ADAPTER".into());
    let adapted = extract(&path, &options)?;
    assert!(matches!(plain.exports["Payload"], Type::Named(_)));
    assert_eq!(adapted.exports["Payload"], Type::Scalar(Scalar::F32));
    let Type::Named(kind) = &adapted.exports["Kind"] else {
        return Err("enum alias expected".into());
    };
    let DefinitionKind::Enum(variants) = &adapted.definitions[kind].kind else {
        return Err("enum variants expected".into());
    };
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.tag)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    let Type::Named(storage) = &adapted.exports["IntegerStorage"] else {
        return Err("specialization alias expected".into());
    };
    let DefinitionKind::Record(fields) = &adapted.definitions[storage].kind else {
        return Err("specialized fields expected".into());
    };
    assert_eq!(fields[0].ty, Type::Scalar(Scalar::I32));
    Ok(())
}

#[test]
fn ordinary_exports_preserve_preprocessed_definitions() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    std::fs::write(directory.path().join("values.inc"), "First = 1, Last = 3")?;
    std::fs::write(
        &path,
        r#"#include <intrlay/annotations.hpp>
        #define DEFINE_RECORD(Name) struct INTRLAY() Name { int value; };
        DEFINE_RECORD(Root)
        enum class INTRLAY() Mode {
        #include "values.inc"
        };
        #ifdef ENABLE_ADAPTER
        struct Other {};
        template<> struct intrlay::Adapter<Other> { using schema_type = int; };
        #endif
        "#,
    )?;
    let mut options = options();
    let plain = extract(&path, &options)?;
    options.arguments.push("-DENABLE_ADAPTER".into());
    assert_eq!(plain, extract(&path, &options)?);
    assert!(plain.exports.contains_key("Root"));
    let DefinitionKind::Enum(variants) = &common::definition(&plain, "Mode").kind else {
        return Err("included enumerators expected".into());
    };
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.tag)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    Ok(())
}

#[test]
fn probe_type_capture_preserves_adapter_access_constraints() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("schema.hpp");
    std::fs::write(
        &path,
        r"#include <intrlay/annotations.hpp>
        struct INTRLAY() Root {
            int value;
        private:
            using PrivateSchema = float;
        };
        template<class T> requires requires { typename T::PrivateSchema; }
        struct intrlay::Adapter<T> { using schema_type = typename T::PrivateSchema; };
        ",
    )?;
    let schema = extract(&path, &options())?;
    let DefinitionKind::Record(fields) = &common::definition(&schema, "Root").kind else {
        return Err("probe bypassed a private member in an adapter constraint".into());
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].ty, Type::Scalar(Scalar::I32));
    Ok(())
}

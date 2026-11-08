//! Name diagnostics follow the declarations actually emitted in each target scope.

use super::Fixtures;

use std::{collections::BTreeMap, error::Error, fmt::Write};

use intrlay_codegen::{
    GenerateOptions, Modifier, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector,
    Visibility, generate, generate_with_options,
};
use intrlay_schema::{
    Definition, DefinitionKind, Field, Language, NameCase, Scalar, Schema, TargetOptions, Type,
    Variant,
};

fn scalar(name: &str) -> Field {
    Field::new(name, Type::Scalar(Scalar::U32))
}

fn record(name: &str, fields: Vec<Field>) -> Definition {
    Definition {
        name: name.into(),
        docs: String::new(),
        kind: DefinitionKind::Record(fields),
        alignment: 1,
        targets: BTreeMap::new(),
        transparent: false,
    }
}

fn schema(root: Definition) -> Schema {
    let name = root.name.clone();
    Schema {
        exports: BTreeMap::from([(name.clone(), Type::Named(name.clone()))]),
        definitions: BTreeMap::from([(name, root)]),
    }
}

fn named(name: &str) -> TargetOptions {
    TargetOptions {
        name: Some(name.into()),
        ..Default::default()
    }
}

fn reference_schema(language: Language, name: &str) -> Schema {
    let mut field = scalar("number");
    field.targets.insert(language, named(name));
    let mut source = schema(record(
        "Packet",
        vec![field, Field::new("item", Type::Named("Child".into()))],
    ));
    source
        .definitions
        .insert("Child".into(), record("Child", vec![scalar("number")]));
    source
        .exports
        .insert("Child".into(), Type::Named("Child".into()));
    source
}

fn reference_fields(source: &mut Schema) -> Result<&mut Vec<Field>, Box<dyn Error>> {
    let DefinitionKind::Record(fields) = &mut source
        .definitions
        .get_mut("Packet")
        .ok_or("missing Packet")?
        .kind
    else {
        unreachable!()
    };
    Ok(fields)
}

fn save_reference_fixture(
    fixtures: &mut Fixtures,
    source: &Schema,
    language: Language,
    name: &str,
) -> Result<(), Box<dyn Error>> {
    save_reference_with_tail(fixtures, source, language, name, "")
}

fn save_reference_with_tail(
    fixtures: &mut Fixtures,
    source: &Schema,
    language: Language,
    name: &str,
    tail: &str,
) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("reference-scopes");

    fixtures.insert(directory.join(name), generate(source, language)? + tail)?;
    Ok(())
}

fn dart_members_cannot_shadow_referenced_codecs(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut source = reference_schema(Language::Dart, "_writePacket");
    rejects(
        &source,
        Language::Dart,
        GenerateOptions::default(),
        "referenced type or codec",
    )?;
    let root = source
        .definitions
        .get_mut("Packet")
        .ok_or("missing Packet")?;
    root.targets.insert(
        Language::Dart,
        TargetOptions {
            create: Some(false),
            ..Default::default()
        },
    );
    // Inline replacement still calls the writer without any factories.
    rejects(
        &source,
        Language::Dart,
        GenerateOptions::default(),
        "referenced type or codec",
    )?;
    root_options(&mut source, Language::Dart)?.generate_setters = Some(false);
    save_reference_with_tail(
        fixtures,
        &source,
        Language::Dart,
        "read_only.dart",
        "\ncore.int checkPrivate(PacketView view) => view._writePacket;\n",
    )?;

    let mut source = reference_schema(Language::Dart, "_measurePacket");
    reference_fields(&mut source)?.push(Field::new("text", Type::String));
    rejects(
        &source,
        Language::Dart,
        GenerateOptions::default(),
        "referenced type or codec",
    )?;
    root_options(&mut source, Language::Dart)?.create = Some(false);
    save_reference_with_tail(
        fixtures,
        &source,
        Language::Dart,
        "no_measure.dart",
        "\ncore.int checkPrivate(core.int value, PacketView view) {\n  view._measurePacket = value;\n  return view._measurePacket;\n}\n",
    )?;

    // Child's writer is called by Packet's setter, not only by module codecs.
    let mut source = reference_schema(Language::Dart, "_writeChild");
    rejects(
        &source,
        Language::Dart,
        GenerateOptions::default(),
        "referenced type or codec",
    )?;
    reference_fields(&mut source)?[1].targets.insert(
        Language::Dart,
        TargetOptions {
            generate_setters: Some(false),
            ..Default::default()
        },
    );
    save_reference_with_tail(
        fixtures,
        &source,
        Language::Dart,
        "no_mutator.dart",
        "\ncore.int checkPrivate(PacketInput value, PacketView view) {\n  view._writeChild = value._writeChild;\n  return view._writeChild;\n}\n",
    )?;
    Ok(())
}

fn root_options(
    source: &mut Schema,
    language: Language,
) -> Result<&mut TargetOptions, Box<dyn Error>> {
    Ok(source
        .definitions
        .get_mut("Packet")
        .ok_or("missing Packet")?
        .targets
        .entry(language)
        .or_default())
}

fn dart_members_reserve_only_types_used_in_their_class(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    for name in ["ChildInput", "ChildView"] {
        let mut source = reference_schema(Language::Dart, name);
        rejects(
            &source,
            Language::Dart,
            GenerateOptions::default(),
            "referenced",
        )?;
        reference_fields(&mut source)?[0]
            .targets
            .insert(Language::Dart, named("count"));
        save_reference_fixture(fixtures, &source, Language::Dart, "renamed.dart")?;
        reference_fields(&mut source)?[0]
            .targets
            .insert(Language::Dart, named(name));
        reference_fields(&mut source)?.pop();
        save_reference_fixture(
            fixtures,
            &source,
            Language::Dart,
            &format!("unrelated_{name}.dart"),
        )?;
    }
    let mut source = reference_schema(Language::Dart, "ChildView");
    reference_fields(&mut source)?[1].targets.insert(
        Language::Dart,
        TargetOptions {
            generate_getters: Some(false),
            generate_setters: Some(false),
            ..Default::default()
        },
    );
    save_reference_fixture(fixtures, &source, Language::Dart, "omitted.dart")?;
    Ok(())
}

fn python_annotations_reserve_types_but_method_bodies_keep_module_lookup(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    for name in ["ChildInput", "ChildView"] {
        let mut source = reference_schema(Language::Python, name);
        rejects(
            &source,
            Language::Python,
            GenerateOptions::default(),
            "referenced type or codec",
        )?;
        reference_fields(&mut source)?[0]
            .targets
            .insert(Language::Python, named("count"));
        save_reference_fixture(fixtures, &source, Language::Python, "renamed.py")?;
        reference_fields(&mut source)?[0]
            .targets
            .insert(Language::Python, named(name));
        reference_fields(&mut source)?.pop();
        save_reference_fixture(
            fixtures,
            &source,
            Language::Python,
            &format!("unrelated_{name}.py"),
        )?;
    }
    let mut source = reference_schema(Language::Python, "ChildView");
    reference_fields(&mut source)?[1].targets.insert(
        Language::Python,
        TargetOptions {
            generate_getters: Some(false),
            generate_setters: Some(false),
            ..Default::default()
        },
    );
    save_reference_fixture(fixtures, &source, Language::Python, "omitted.py")?;

    for name in ["_write_packet", "_write_child", "_measure_packet"] {
        let mut source = reference_schema(Language::Python, name);
        reference_fields(&mut source)?.push(Field::new("text", Type::String));
        save_reference_fixture(fixtures, &source, Language::Python, &format!("{name}.py"))?;
    }
    Ok(())
}

#[test]
fn dart_class_members_cannot_shadow_import_prefixes() -> Result<(), Box<dyn Error>> {
    for name in ["core", "typed", "intrlay"] {
        let mut field = scalar("value");
        field.targets.insert(Language::Dart, named(name));
        rejects(
            &schema(record("Root", vec![field.clone()])),
            Language::Dart,
            GenerateOptions::default(),
            name,
        )?;
        rejects(
            &enum_schema(Language::Dart, "Present", field),
            Language::Dart,
            GenerateOptions::default(),
            name,
        )?;
    }
    for name in ["convert", "coreValue", "typedValue", "intrlayValue"] {
        let mut field = scalar("value");
        field.targets.insert(Language::Dart, named(name));
        generate(&schema(record("Root", vec![field])), Language::Dart)?;
    }
    Ok(())
}

#[test]
fn csharp_contextual_native_names_preserve_valid_properties() -> Result<(), Box<dyn Error>> {
    for name in ["var", "record", "scoped", "required", "file"] {
        let mut root = record("Root", vec![scalar("value")]);
        root.targets.insert(Language::CSharp, named(name));
        rejects(
            &schema(root),
            Language::CSharp,
            GenerateOptions::default(),
            "unsupported C# native type name",
        )?;
        let mut field = scalar("value");
        field.targets.insert(Language::CSharp, named(name));
        generate(&schema(record("Root", vec![field])), Language::CSharp)?;
    }
    for name in [
        "Var",
        "Record",
        "Scoped",
        "Required",
        "File",
        "dynamic",
        "nint",
        "nuint",
        "notnull",
        "unmanaged",
        "global",
        "alias",
        "args",
    ] {
        let mut root = record("Root", vec![scalar("value")]);
        root.targets.insert(Language::CSharp, named(name));
        generate(&schema(root), Language::CSharp)?;
    }
    Ok(())
}

fn csharp_intrinsic_keywords_are_rejected_only_in_unescaped_declarations(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut fields = Vec::new();
    for name in ["__arglist", "__makeref", "__reftype", "__refvalue"] {
        let mut root = record("Root", vec![scalar("value")]);
        root.targets.insert(Language::CSharp, named(name));
        rejects(
            &schema(root),
            Language::CSharp,
            GenerateOptions::default(),
            "reserved CSharp identifier",
        )?;
        let mut field = scalar("value");
        field.targets.insert(Language::CSharp, named(name));
        let mut escaped = field.clone();
        escaped.name = name.into();
        fields.push(escaped);
        let source = schema(record("Root", vec![field]));
        generate(&source, Language::CSharp)?;

        let error = generate_with_options(
            &source,
            Language::CSharp,
            GenerateOptions {
                csharp: intrlay_codegen::CSharpOptions {
                    namespace: Some(format!("Example.{name}")),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .err()
        .ok_or("intrinsic namespace should fail")?;
        assert!(error.to_string().contains("invalid C# namespace"));
    }
    let generated = generate_with_options(
        &schema(record("IntrinsicProperties", fields)),
        Language::CSharp,
        GenerateOptions {
            csharp: intrlay_codegen::CSharpOptions {
                namespace: Some("EdgeCases.Intrinsics".into()),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let directory = std::path::PathBuf::from("edge-cases");

    fixtures.insert(directory.join("intrinsic-properties.cs"), generated)?;
    Ok(())
}

fn root_value_naming(language: Language) -> GenerateOptions {
    let mut naming = NamingPolicy::for_language(language);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Property),
                visibility: Some(Visibility::Public),
                ..Default::default()
            },
            style: NameStyle {
                prefix: "set_".into(),
                ..NameStyle::new(NameCase::SnakeCase)
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Method),
                ..Default::default()
            },
            style: NameStyle {
                suffix: "_value".into(),
                ..NameStyle::new(NameCase::SnakeCase)
            },
        },
    ]);
    GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    }
}

#[test]
fn root_value_getters_conflict_with_distinct_setter_methods() -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let leaf = Type::Named("Leaf".into());
        for root in [
            Type::Boxed(Box::new(leaf.clone())),
            Type::Optional(Box::new(leaf)),
        ] {
            let mut source = schema(record("Leaf", vec![scalar("number")]));
            source.exports = BTreeMap::from([("Root".into(), root)]);
            generate(&source, language)?;
            rejects(
                &source,
                language,
                root_value_naming(language),
                "value setter method",
            )?;
        }

        // Native scalar properties share one name between their getter and setter.
        // String roots emit no setter, so the same policy is valid there too.
        for root in [
            Type::Scalar(Scalar::U32),
            Type::Optional(Box::new(Type::Scalar(Scalar::U32))),
            Type::String,
        ] {
            generate_with_options(
                &Schema {
                    exports: BTreeMap::from([("Root".into(), root)]),
                    definitions: BTreeMap::new(),
                },
                language,
                root_value_naming(language),
            )?;
        }

        // A Box cannot mutate its payload when that payload omits its setters.
        let mut leaf = record("Leaf", vec![scalar("number")]);
        leaf.targets.insert(
            language,
            TargetOptions {
                generate_setters: Some(false),
                ..Default::default()
            },
        );
        let mut source = schema(leaf);
        source.exports = BTreeMap::from([(
            "Root".into(),
            Type::Boxed(Box::new(Type::Named("Leaf".into()))),
        )]);
        generate_with_options(&source, language, root_value_naming(language))?;
    }
    Ok(())
}

#[test]
fn typescript_infrastructure_checks_each_class_scope() -> Result<(), Box<dyn Error>> {
    let source = schema(record("Root", vec![scalar("value")]));
    let mut naming = NamingPolicy::for_language(Language::TypeScript);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                visibility: Some(Visibility::Private),
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::SnakeCase,
                prefix: String::new(),
                suffix: "_bytes".into(),
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Property),
                visibility: Some(Visibility::Public),
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::SnakeCase,
                prefix: "storage_".into(),
                suffix: String::new(),
            },
        },
    ]);
    rejects(
        &source,
        Language::TypeScript,
        GenerateOptions {
            naming: Some(naming),
            ..Default::default()
        },
        "view instance infrastructure",
    )?;

    // A static constant and an instance field may share a spelling.
    let mut naming = NamingPolicy::for_language(Language::TypeScript);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                require: vec![Modifier::Static, Modifier::Const],
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::SnakeCase,
                prefix: "offset_".into(),
                suffix: String::new(),
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                visibility: Some(Visibility::Private),
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::SnakeCase,
                prefix: String::new(),
                suffix: "_byte_size".into(),
            },
        },
    ]);
    let output = generate_with_options(
        &source,
        Language::TypeScript,
        GenerateOptions {
            naming: Some(naming),
            ..Default::default()
        },
    )?;
    assert!(output.contains("static readonly offset_byte_size"));
    assert!(output.contains("private readonly offset_byte_size"));
    Ok(())
}

fn factory_collision_naming(language: Language, factory: &str) -> GenerateOptions {
    let mut naming = NamingPolicy::for_language(language);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                require: vec![Modifier::Const],
                ..Default::default()
            },
            style: NameStyle {
                suffix: format!("_{factory}"),
                ..NameStyle::new(NameCase::SnakeCase)
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Method),
                require: vec![Modifier::Static],
                ..Default::default()
            },
            style: NameStyle {
                prefix: "byte_size_".into(),
                ..NameStyle::new(NameCase::SnakeCase)
            },
        },
    ]);
    GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    }
}

#[test]
fn view_factories_cannot_shadow_constants_unless_omitted() -> Result<(), Box<dyn Error>> {
    for language in [
        Language::Python,
        Language::Dart,
        Language::CSharp,
        Language::TypeScript,
        Language::Cpp,
    ] {
        for factory in ["create", "create_in_place", "compute_byte_length"] {
            for mut source in [
                value_enum_schema(language, "Ready"),
                schema(record("Packet", vec![Field::new("text", Type::String)])),
            ] {
                rejects(
                    &source,
                    language,
                    factory_collision_naming(language, factory),
                    &format!("byte_size_{factory}"),
                )?;
                for definition in source.definitions.values_mut() {
                    definition.targets.entry(language).or_default().create = Some(false);
                }
                generate_with_options(
                    &source,
                    language,
                    factory_collision_naming(language, factory),
                )?;
            }
        }
    }
    Ok(())
}

fn omitted_factories_generate_compilable_members(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    for (language, path) in [
        (Language::Python, "reference-scopes/omitted_factories.py"),
        (Language::Dart, "reference-scopes/omitted_factories.dart"),
        (Language::CSharp, "naming/omitted-factories.cs"),
    ] {
        let mut source = value_enum_schema(language, "Ready");
        for definition in source.definitions.values_mut() {
            definition.targets.entry(language).or_default().create = Some(false);
        }
        let mut options = factory_collision_naming(language, "create");
        options.csharp.namespace = Some("IntrlayGenerated.OmittedFactories".into());
        fixtures.insert(path, generate_with_options(&source, language, options)?)?;
    }

    // Rust's fixed helper names remain reserved only when derive emits them.
    let mut root = record("Packet", vec![scalar("create")]);
    root.targets.insert(
        Language::Rust,
        TargetOptions {
            create: Some(false),
            ..Default::default()
        },
    );
    fixtures.insert(
        "naming/omitted-factories.rs",
        generate(&schema(root), Language::Rust)?,
    )?;

    // These native-access helpers do not exist on a payload-bearing record.
    fixtures.insert(
        "edge-cases/omitted-accessors.cs",
        generate_with_options(
            &schema(record(
                "AvailableNames",
                vec![
                    scalar("try_get_native_span"),
                    Field::new("to_value", Type::String),
                ],
            )),
            Language::CSharp,
            GenerateOptions {
                csharp: intrlay_codegen::CSharpOptions {
                    namespace: Some("EdgeCases.OmittedApis".into()),
                    ..Default::default()
                },
                ..Default::default()
            },
        )?,
    )?;
    Ok(())
}

fn collection_collision_naming(
    language: Language,
    prefix: &str,
    other: SymbolSelector,
) -> GenerateOptions {
    let mut naming = NamingPolicy::for_language(language);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Property),
                ..Default::default()
            },
            style: NameStyle {
                prefix: prefix.into(),
                ..NameStyle::new(NameCase::PascalCase)
            },
        },
        NamingRule {
            selector: other,
            style: NameStyle {
                suffix: "Length".into(),
                ..NameStyle::new(NameCase::CamelCase)
            },
        },
    ]);
    GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    }
}

fn collection_method_naming(language: Language, prefix: &str) -> GenerateOptions {
    collection_collision_naming(
        language,
        prefix,
        SymbolSelector {
            kind: Some(SymbolKind::Method),
            exclude: vec![Modifier::Static],
            ..Default::default()
        },
    )
}

fn collection_static_naming(language: Language) -> GenerateOptions {
    collection_collision_naming(
        language,
        "byteSize",
        SymbolSelector {
            kind: Some(SymbolKind::Field),
            require: vec![Modifier::Const],
            ..Default::default()
        },
    )
}

#[test]
fn collection_members_reject_collisions_for_every_element_shape() -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Dart] {
        let mut source = value_enum_schema(language, "Ready");
        source
            .definitions
            .insert("Entry".into(), record("Entry", vec![scalar("number")]));
        for element in [
            Type::Named("Choice".into()),
            Type::Named("Entry".into()),
            Type::String,
        ] {
            for collection in [
                Type::Sequence(Box::new(element.clone())),
                Type::Array {
                    element: Box::new(element.clone()),
                    length: 3,
                },
            ] {
                source.exports = BTreeMap::from([("Items".into(), collection)]);
                if language == Language::TypeScript {
                    rejects(
                        &source,
                        language,
                        collection_method_naming(language, "get"),
                        "collection element getter",
                    )?;
                    if element == Type::String {
                        // A string cannot be replaced in place, so no set method is declared.
                        generate_with_options(
                            &source,
                            language,
                            collection_method_naming(language, "set"),
                        )?;
                    } else {
                        rejects(
                            &source,
                            language,
                            collection_method_naming(language, "set"),
                            "collection element setter",
                        )?;
                    }
                } else {
                    rejects(
                        &source,
                        language,
                        collection_static_naming(language),
                        "collection length",
                    )?;
                    // Dart uses operators, not configurable get/set method names.
                    generate_with_options(
                        &source,
                        language,
                        collection_method_naming(language, "get"),
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[test]
fn collection_getter_validation_preserves_write_only_borrowing() -> Result<(), Box<dyn Error>> {
    let language = Language::TypeScript;
    for (element, borrows) in [(Type::Named("Choice".into()), true), (Type::String, false)] {
        let mut source = value_enum_schema(language, "Ready");
        let mut root = record(
            "Root",
            vec![Field::new("items", Type::Sequence(Box::new(element)))],
        );
        root.targets.insert(
            language,
            TargetOptions {
                generate_getters: Some(false),
                ..Default::default()
            },
        );
        source.definitions.insert("Root".into(), root);
        source.exports = BTreeMap::from([("Root".into(), Type::Named("Root".into()))]);
        let options = collection_method_naming(language, "get");
        if borrows {
            // Writable child views still need a get method when scalar getters are omitted.
            rejects(&source, language, options, "collection element getter")?;
        } else {
            generate_with_options(&source, language, options)?;
        }
    }
    Ok(())
}

fn collection_scopes_generate_compilable_members(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Dart, Language::Python] {
        let mut source = value_enum_schema(language, "Ready");
        source.exports = BTreeMap::from([(
            "Items".into(),
            Type::Sequence(Box::new(Type::Named("Choice".into()))),
        )]);
        let options = if language == Language::TypeScript {
            // A TypeScript static constant and an instance getter have separate scopes.
            collection_static_naming(language)
        } else {
            collection_method_naming(language, "get")
        };
        let (directory, name) = match language {
            Language::TypeScript => ("naming", "collection-static.ts"),
            Language::Dart => ("reference-scopes", "collection.dart"),
            _ => ("reference-scopes", "collection.py"),
        };
        fixtures.insert(
            std::path::Path::new(directory).join(name),
            generate_with_options(&source, language, options)?,
        )?;
    }
    let source = Schema {
        exports: BTreeMap::from([("Items".into(), Type::Sequence(Box::new(Type::String)))]),
        definitions: BTreeMap::new(),
    };
    fixtures.insert(
        "naming/collection-omitted.ts",
        generate_with_options(
            &source,
            Language::TypeScript,
            collection_method_naming(Language::TypeScript, "set"),
        )?,
    )?;
    Ok(())
}

fn rust_derives_use_the_names_validated_by_codegen(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let source = schema(record("Root", vec![scalar("Create")]));
    let mut naming = NamingPolicy::for_language(Language::Rust);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Property),
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::SnakeCase,
                prefix: "get_".into(),
                suffix: String::new(),
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Type),
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::PascalCase,
                prefix: String::new(),
                suffix: "Data".into(),
            },
        },
    ]);
    let options = GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    };
    let generated = generate_with_options(&source, Language::Rust, options.clone())?;
    assert!(generated.contains("#[intrlay(rust(name = \"get_create\"))]"));
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(directory.join("rust-derive-names.rs"), generated)?;
    let mut collision = source;
    let mut other = record("Other", vec![scalar("number")]);
    other.targets.insert(Language::Rust, named("RootDataView"));
    collision.definitions.insert("Other".into(), other);
    collision
        .exports
        .insert("Other".into(), Type::Named("Other".into()));
    rejects(&collision, Language::Rust, options, "RootDataView")?;
    Ok(())
}

fn rust_enum_mutators_use_configured_property_names(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut source = enum_schema(Language::Rust, "A", scalar("b_get_c"));
    let DefinitionKind::Enum(variants) = &mut source
        .definitions
        .get_mut("Choice")
        .ok_or("choice expected")?
        .kind
    else {
        return Err("enum expected".into());
    };
    variants.push(Variant {
        name: "AGetB".into(),
        tag: 1,
        fields: vec![scalar("c")],
        synthetic_payload: false,
        docs: String::new(),
        targets: BTreeMap::new(),
    });
    let mut naming = NamingPolicy::for_language(Language::Rust);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Property),
            ..Default::default()
        },
        style: NameStyle {
            prefix: "get_".into(),
            ..NameStyle::new(NameCase::SnakeCase)
        },
    });
    let options = GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    };
    generate(&source, Language::Rust)?;
    rejects(&source, Language::Rust, options.clone(), "a_get_b_get_c")?;
    let DefinitionKind::Enum(variants) = &mut source
        .definitions
        .get_mut("Choice")
        .ok_or("choice expected")?
        .kind
    else {
        return Err("enum expected".into());
    };
    variants[1].targets.insert(Language::Rust, named("Other"));
    let generated = generate_with_options(&source, Language::Rust, options)?;
    let directory = std::path::PathBuf::from("regressions");

    fixtures.insert(directory.join("rust-enum-names.rs"), generated)?;
    Ok(())
}

fn rust_enum_payloads_do_not_declare_individual_getters(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    // Create/from only contributes create_from_mut and replace_create_from;
    // its read access is a field on ChoiceVariantView::Create.
    let source = enum_schema(Language::Rust, "Create", scalar("from"));
    fixtures.insert(
        "regressions/rust-enum-accessors.rs",
        generate(&source, Language::Rust)?,
    )?;

    // Records do have individual getters, so their factory collisions remain errors.
    rejects(
        &schema(record("Packet", vec![scalar("create_from")])),
        Language::Rust,
        GenerateOptions::default(),
        "view construction",
    )?;
    Ok(())
}

fn rust_enum_derive_names_are_valid_identifiers(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut naming = NamingPolicy::for_language(Language::Rust);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Field),
            ..Default::default()
        },
        style: NameStyle {
            prefix: "wire_".into(),
            ..NameStyle::new(NameCase::SnakeCase)
        },
    });
    let options = GenerateOptions {
        naming: Some(naming.clone()),
        ..Default::default()
    };
    // Raw Rust source identifiers export their semantic spelling. Renaming only
    // the stored field leaves a keyword in the derive's accessor annotation.
    for name in ["match", "type", "gen", "self"] {
        let source = enum_schema(Language::Rust, "Data", scalar(name));
        rejects(
            &source,
            Language::Rust,
            options.clone(),
            "reserved Rust identifier",
        )?;
    }

    // An explicit target name overrides both roles without any automatic escaping.
    let mut field = scalar("match");
    field.targets.insert(Language::Rust, named("pattern"));
    generate_with_options(
        &enum_schema(Language::Rust, "Data", field),
        Language::Rust,
        options,
    )?;

    // Separate field and accessor naming rules can also produce a valid annotation.
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Property),
            ..Default::default()
        },
        style: NameStyle {
            prefix: "get_".into(),
            ..NameStyle::new(NameCase::SnakeCase)
        },
    });
    fixtures.insert(
        "regressions/rust-enum-keywords.rs",
        generate_with_options(
            &enum_schema(Language::Rust, "Data", scalar("match")),
            Language::Rust,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
        )?,
    )?;
    Ok(())
}

#[test]
fn typescript_type_keywords_remain_valid_property_names() -> Result<(), Box<dyn Error>> {
    for name in [
        "object",
        "undefined",
        "intrinsic",
        "readonly",
        "await",
        "infer",
        "keyof",
        "unique",
    ] {
        let mut root = record("Root", vec![scalar("number")]);
        root.targets.insert(Language::TypeScript, named(name));
        rejects(
            &schema(root),
            Language::TypeScript,
            GenerateOptions::default(),
            "reserved TypeScript identifier",
        )?;
        let mut field = scalar("value");
        field.targets.insert(Language::TypeScript, named(name));
        generate(&schema(record("Root", vec![field])), Language::TypeScript)?;
    }
    Ok(())
}

#[test]
fn dart_classes_reserve_their_own_names() -> Result<(), Box<dyn Error>> {
    for name in ["RootInput", "RootView"] {
        let mut field = scalar("value");
        field.targets.insert(Language::Dart, named(name));
        rejects(
            &schema(record("Root", vec![field])),
            Language::Dart,
            GenerateOptions::default(),
            "enclosing",
        )?;
    }
    let mut field = scalar("value");
    field
        .targets
        .insert(Language::Dart, named("ChoicePresentInput"));
    rejects(
        &enum_schema(Language::Dart, "Present", field),
        Language::Dart,
        GenerateOptions::default(),
        "enclosing native type",
    )?;
    for name in ["_payload", "__payload"] {
        let mut private = scalar("private");
        private.targets.insert(Language::Dart, named(name));
        rejects(
            &schema(record("Root", vec![private, scalar("payload")])),
            Language::Dart,
            GenerateOptions::default(),
            "constructor parameter",
        )?;
    }
    Ok(())
}

#[test]
fn csharp_native_input_names_remain_available_without_construction() -> Result<(), Box<dyn Error>> {
    for collection in [
        Type::Sequence(Box::new(Type::Named("Item".into()))),
        Type::Array {
            element: Box::new(Type::Named("Item".into())),
            length: 2,
        },
    ] {
        let mut root = record("Root", vec![Field::new("items", collection)]);
        root.targets.insert(
            Language::CSharp,
            TargetOptions {
                create: Some(false),
                ..Default::default()
            },
        );
        let mut source = schema(root);
        source.definitions.insert(
            "Item".into(),
            record("ElementAccess", vec![scalar("number")]),
        );
        generate(&source, Language::CSharp)?;
    }
    Ok(())
}

#[test]
fn rust_input_export_aliases_reserve_their_declarations() -> Result<(), Box<dyn Error>> {
    let mut source = schema(record("Primary", vec![scalar("number")]));
    source
        .exports
        .insert("Secondary".into(), Type::Named("Primary".into()));
    source.definitions.insert(
        "Collision".into(),
        record("SecondaryInput", vec![scalar("number")]),
    );
    rejects(
        &source,
        Language::Rust,
        GenerateOptions::default(),
        "export input alias",
    )?;
    source.definitions.remove("Collision");
    generate(&source, Language::Rust)?;
    Ok(())
}

#[test]
fn csharp_span_inputs_reserve_declarations_and_enclosing_members() -> Result<(), Box<dyn Error>> {
    for create in [false, true] {
        let mut field = scalar("number");
        field
            .targets
            .insert(Language::CSharp, named("RootSpanInput"));
        let mut root = record(
            "Root",
            vec![Field::new("text", Type::String), field.clone()],
        );
        root.targets.insert(
            Language::CSharp,
            TargetOptions {
                create: Some(create),
                ..Default::default()
            },
        );
        rejects(
            &schema(root),
            Language::CSharp,
            GenerateOptions::default(),
            "enclosing native type",
        )?;
        // Fixed records emit only their ordinary Input declaration.
        generate(&schema(record("Root", vec![field])), Language::CSharp)?;
    }
    let mut source = schema(record("Root", vec![Field::new("text", Type::String)]));
    let mut other = record("Other", vec![scalar("number")]);
    other.targets.insert(Language::CSharp, named("RootSpan"));
    source.definitions.insert("Other".into(), other);
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions::default(),
        "RootSpanInput",
    )?;

    let mut source = schema(record("Root", vec![Field::new("text", Type::String)]));
    source
        .definitions
        .get_mut("Root")
        .ok_or("root")?
        .targets
        .insert(Language::CSharp, named("Renamed"));
    let mut other = record("Other", vec![scalar("number")]);
    other.targets.insert(Language::CSharp, named("RenamedSpan"));
    source.definitions.insert("Other".into(), other);
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions::default(),
        "RenamedSpanInput",
    )?;
    Ok(())
}

#[test]
fn csharp_codecs_cannot_match_their_enclosing_binding_class() -> Result<(), Box<dyn Error>> {
    let mut source = schema(record(
        "Root",
        vec![Field::new("child", Type::Named("Child".into()))],
    ));
    let mut child = record("Child", vec![scalar("number")]);
    child.targets.insert(Language::CSharp, named("Bindings"));
    source.definitions.insert("Child".into(), child);
    let mut naming = NamingPolicy::for_language(Language::CSharp);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Type),
            ..Default::default()
        },
        style: NameStyle {
            case: NameCase::PascalCase,
            prefix: "Write".into(),
            suffix: String::new(),
        },
    });
    let options = GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    };
    rejects(
        &source,
        Language::CSharp,
        options.clone(),
        "enclosing binding class",
    )?;
    source
        .definitions
        .get_mut("Child")
        .ok_or("child")?
        .targets
        .insert(Language::CSharp, named("Entry"));
    generate_with_options(&source, Language::CSharp, options)?;
    Ok(())
}

#[test]
fn csharp_source_factories_reserve_their_public_names() -> Result<(), Box<dyn Error>> {
    let mut field = scalar("source");
    field.targets.insert(Language::CSharp, named("FromSources"));
    let source = schema(record(
        "Root",
        vec![
            field.clone(),
            Field::new(
                "values",
                Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
            ),
        ],
    ));
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions::default(),
        "generic input factory",
    )?;

    // A scalar-only input has no source factory to conflict with this property.
    generate(&schema(record("Root", vec![field])), Language::CSharp)?;
    Ok(())
}

fn enum_schema(language: Language, variant: &str, field: Field) -> Schema {
    enum_schema_with_fields(language, variant, vec![field])
}

fn value_enum_schema(language: Language, variant: &str) -> Schema {
    enum_schema_with_fields(language, variant, Vec::new())
}

fn enum_schema_with_fields(language: Language, variant: &str, fields: Vec<Field>) -> Schema {
    let mut root = record("Choice", Vec::new());
    root.kind = DefinitionKind::Enum(vec![Variant {
        name: "Present".into(),
        tag: 0,
        fields,
        synthetic_payload: false,
        docs: String::new(),
        targets: BTreeMap::from([(language, named(variant))]),
    }]);
    schema(root)
}

#[test]
fn native_value_enum_members_reserve_their_actual_scopes() -> Result<(), Box<dyn Error>> {
    for name in [
        "tag",
        "values",
        "index",
        "hashCode",
        "runtimeType",
        "toString",
        "noSuchMethod",
        "core",
        "ChoiceInput",
    ] {
        rejects(
            &value_enum_schema(Language::Dart, name),
            Language::Dart,
            GenerateOptions::default(),
            name,
        )?;
        // Payload alternatives remain separate classes with none of these
        // enum-member conflicts.
        generate(
            &enum_schema(Language::Dart, name, scalar("number")),
            Language::Dart,
        )?;
    }
    for name in ["value__", "ChoiceInput"] {
        rejects(
            &value_enum_schema(Language::CSharp, name),
            Language::CSharp,
            GenerateOptions::default(),
            name,
        )?;
    }
    // Native C# enums have neither a generated Tag property nor factories,
    // and their constants can use names inherited from System.Enum.
    for name in ["Tag", "FromPresent", "ToString", "GetType", "Equals"] {
        generate(&value_enum_schema(Language::CSharp, name), Language::CSharp)?;
    }
    Ok(())
}

#[test]
fn typescript_enum_members_reject_the_object_prototype_setter() -> Result<(), Box<dyn Error>> {
    for source in [
        value_enum_schema(Language::TypeScript, "__proto__"),
        enum_schema(Language::TypeScript, "__proto__", scalar("number")),
    ] {
        rejects(
            &source,
            Language::TypeScript,
            GenerateOptions::default(),
            "numeric enum object prototype",
        )?;
    }
    Ok(())
}

#[test]
fn python_value_enum_members_reject_metaclass_and_ignored_names() -> Result<(), Box<dyn Error>> {
    for name in [
        "_value_",
        "_ignore_",
        "_missing_",
        "_future_",
        "__members__",
        "__doc__",
        "__hidden",
        "_ChoiceInput__hidden",
        "mro",
        "name",
        "as_integer_ratio",
        "bit_count",
        "bit_length",
        "conjugate",
        "denominator",
        "from_bytes",
        "imag",
        "is_integer",
        "to_bytes",
        "__",
        "___value__",
    ] {
        rejects(
            &value_enum_schema(Language::Python, name),
            Language::Python,
            GenerateOptions::default(),
            "Python IntEnum",
        )?;
        if name == "_ChoiceInput__hidden" {
            generate(
                &enum_schema(Language::Python, name, scalar("number")),
                Language::Python,
            )?;
        } else {
            rejects(
                &enum_schema(Language::Python, name, scalar("number")),
                Language::Python,
                GenerateOptions::default(),
                "Python IntEnum",
            )?;
        }
    }
    for name in ["value", "real", "numerator", "_private", "_", "trailing__"] {
        generate(&value_enum_schema(Language::Python, name), Language::Python)?;
    }
    rejects(
        &enum_schema(Language::Python, "_ChoiceKind__hidden", scalar("number")),
        Language::Python,
        GenerateOptions::default(),
        "Python IntEnum",
    )?;
    Ok(())
}

fn python_accepted_enum_names_compile_and_run(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let names = ["value", "real", "numerator", "_private", "_", "trailing__"];
    let mut source = Schema {
        exports: BTreeMap::new(),
        definitions: BTreeMap::new(),
    };
    for (type_name, payload) in [("ValueNames", false), ("PayloadNames", true)] {
        let mut definition = record(type_name, Vec::new());
        definition.kind = DefinitionKind::Enum(
            names
                .iter()
                .zip(37_u32..)
                .map(|(name, tag)| Variant {
                    name: format!("Variant{tag}"),
                    tag,
                    fields: if payload {
                        vec![scalar("number")]
                    } else {
                        Vec::new()
                    },
                    synthetic_payload: false,
                    docs: String::new(),
                    targets: BTreeMap::from([(Language::Python, named(name))]),
                })
                .collect(),
        );
        source
            .exports
            .insert(type_name.into(), Type::Named(type_name.into()));
        source.definitions.insert(type_name.into(), definition);
    }
    let mut output = generate(&source, Language::Python)?;
    for (name, tag) in names.iter().zip(37_u32..) {
        write!(
            output,
            "\nassert ValueNamesView.create(ValueNamesInput.{name}).tag == {tag}\n\
             assert PayloadNamesView.create({{'kind': PayloadNamesKind.{name}, 'number': 7}}).tag == {tag}\n"
        )?;
    }
    fixtures.insert("reference-scopes/native_enum_names.py", output)?;
    Ok(())
}

#[test]
fn payload_enum_kinds_reserve_names_and_export_aliases() -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Python] {
        let mut source = enum_schema(language, "Present", scalar("number"));
        source
            .exports
            .insert("WriteChoice".into(), Type::Named("Choice".into()));
        let mut naming = NamingPolicy::for_language(language);
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Function),
                ..Default::default()
            },
            style: NameStyle {
                suffix: "Kind".into(),
                ..NameStyle::new(NameCase::PascalCase)
            },
        });
        let options = GenerateOptions {
            naming: Some(naming),
            ..Default::default()
        };
        rejects(&source, language, options.clone(), "export enum kind alias")?;

        // No codec is declared when construction and mutation are both omitted,
        // but the input type and its aliased discriminant remain available.
        source
            .definitions
            .get_mut("Choice")
            .ok_or("choice")?
            .targets
            .insert(
                language,
                TargetOptions {
                    generate_getters: Some(false),
                    generate_setters: Some(false),
                    create: Some(false),
                    ..Default::default()
                },
            );
        let generated = generate_with_options(&source, language, options)?;
        assert!(generated.contains("ChoiceKind"));
        assert!(generated.contains("WriteChoiceKind"));
    }
    Ok(())
}

#[test]
fn payload_enum_kinds_follow_variant_casing_with_sparse_tags() -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Python] {
        let mut source = enum_schema(language, "Present", scalar("number"));
        let DefinitionKind::Enum(variants) =
            &mut source.definitions.get_mut("Choice").ok_or("choice")?.kind
        else {
            return Err("enum expected".into());
        };
        variants[0].name = "VeryReady".into();
        variants[0].targets.clear();
        variants[0].tag = 91;
        let mut naming = NamingPolicy::for_language(language);
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Variant),
                ..Default::default()
            },
            style: NameStyle::new(NameCase::SnakeCase),
        });
        let generated = generate_with_options(
            &source,
            language,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
        )?;
        assert!(generated.contains("very_ready = 91"));
        assert!(generated.contains("ChoiceKind.very_ready"));
    }
    Ok(())
}

#[test]
fn value_enum_validation_uses_final_names_and_ignores_accessor_flags() -> Result<(), Box<dyn Error>>
{
    let language = Language::Dart;
    let mut source = value_enum_schema(language, "wire_tag");
    let mut naming = NamingPolicy::for_language(language);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Property),
            visibility: Some(Visibility::Public),
            ..Default::default()
        },
        style: NameStyle {
            prefix: "wire_".into(),
            ..NameStyle::new(NameCase::SnakeCase)
        },
    });
    let options = GenerateOptions {
        naming: Some(naming),
        ..Default::default()
    };
    for getters in [false, true] {
        for setters in [false, true] {
            source
                .definitions
                .get_mut("Choice")
                .ok_or("choice")?
                .targets
                .insert(
                    language,
                    TargetOptions {
                        generate_getters: Some(getters),
                        generate_setters: Some(setters),
                        create: Some(false),
                        ..Default::default()
                    },
                );
            rejects(&source, language, options.clone(), "enum discriminant")?;
        }
    }
    // The enclosing enum follows the export alias and any explicit type name.
    for language in [Language::Dart, Language::CSharp] {
        let mut source = value_enum_schema(language, "AliasInput");
        source.exports = BTreeMap::from([("Alias".into(), Type::Named("Choice".into()))]);
        rejects(&source, language, GenerateOptions::default(), "AliasInput")?;
        source
            .definitions
            .get_mut("Choice")
            .ok_or("choice")?
            .targets
            .insert(language, named("Renamed"));
        generate(&source, language)?;
    }
    for (language, name) in [(Language::Dart, "Values"), (Language::Python, "Mro")] {
        let mut source = value_enum_schema(language, "Present");
        let DefinitionKind::Enum(variants) =
            &mut source.definitions.get_mut("Choice").ok_or("choice")?.kind
        else {
            return Err("enum expected".into());
        };
        variants[0].name = name.into();
        variants[0].targets.clear();
        let mut naming = NamingPolicy::for_language(language);
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Variant),
                ..Default::default()
            },
            style: NameStyle::new(NameCase::SnakeCase),
        });
        rejects(
            &source,
            language,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
            &name.to_lowercase(),
        )?;
    }
    Ok(())
}

#[test]
fn only_discriminated_input_objects_reserve_kind() -> Result<(), Box<dyn Error>> {
    for language in [
        Language::Rust,
        Language::Cpp,
        Language::CSharp,
        Language::Dart,
    ] {
        generate(&enum_schema(language, "Value", scalar("kind")), language)?;
    }
    for language in [Language::TypeScript, Language::Python] {
        rejects(
            &enum_schema(language, "Value", scalar("kind")),
            language,
            GenerateOptions::default(),
            "variant discriminant",
        )?;
    }
    Ok(())
}

fn rejects(
    schema: &Schema,
    language: Language,
    options: GenerateOptions,
    detail: &str,
) -> Result<(), Box<dyn Error>> {
    let error = generate_with_options(schema, language, options)
        .err()
        .ok_or_else(|| {
            format!(
                "expected a generated-name collision: {language:?}, {detail}, exports {:?}",
                schema.exports
            )
        })?;
    assert!(error.to_string().contains(detail), "{language:?}: {error}");
    assert!(
        error.to_string().contains("assign an explicit target name"),
        "{error}"
    );
    Ok(())
}

#[test]
fn container_generation_flags_determine_actual_setter_names() -> Result<(), Box<dyn Error>> {
    for (language, setter) in [
        (Language::Python, "set_value"),
        (Language::TypeScript, "setValue"),
    ] {
        let mut root = record(
            "Root",
            vec![
                scalar("value"),
                Field::new("set_value", Type::Named("Child".into())),
            ],
        );
        root.targets.insert(
            language,
            TargetOptions {
                generate_getters: Some(false),
                ..Default::default()
            },
        );
        let mut data = schema(root);
        data.definitions
            .insert("Child".into(), record("Child", vec![scalar("id")]));
        rejects(&data, language, GenerateOptions::default(), setter)?;

        // A scalar with no getter only emits set_set_value/setSetValue, so its
        // source name may coexist with the first field's named setter.
        let DefinitionKind::Record(fields) =
            &mut data.definitions.get_mut("Root").ok_or("missing Root")?.kind
        else {
            return Err("expected record".into());
        };
        fields[1].ty = Type::Scalar(Scalar::U32);
        generate(&data, language)?;
    }
    Ok(())
}

#[test]
fn csharp_readonly_views_reserve_enclosing_type_names() -> Result<(), Box<dyn Error>> {
    for field in ["root_view_read_only", "root_span_view_read_only"] {
        rejects(
            &schema(record("Root", vec![scalar(field)])),
            Language::CSharp,
            GenerateOptions::default(),
            "enclosing view type",
        )?;
        let mut renamed = scalar(field);
        renamed.targets.insert(Language::CSharp, named("Value"));
        generate(&schema(record("Root", vec![renamed])), Language::CSharp)?;
    }
    Ok(())
}

#[test]
fn cpp_alternative_operators_are_reserved_identifiers() -> Result<(), Box<dyn Error>> {
    for keyword in [
        "and", "and_eq", "bitand", "bitor", "compl", "not", "not_eq", "or", "or_eq", "xor",
        "xor_eq",
    ] {
        let mut field = scalar("value");
        field.targets.insert(Language::Cpp, named(keyword));
        rejects(
            &schema(record("Root", vec![field])),
            Language::Cpp,
            GenerateOptions::default(),
            "reserved Cpp identifier",
        )?;
    }
    Ok(())
}

#[test]
fn python_runtime_view_members_keep_their_literal_names() -> Result<(), Box<dyn Error>> {
    for name in [
        "buffer",
        "_storage",
        "_offset",
        "__init__",
        "__slots__",
        "_intrlay",
        "_builtins",
        "_typing",
        "_abc",
    ] {
        let mut field = scalar("value");
        field.targets.insert(Language::Python, named(name));
        rejects(
            &schema(record("Root", vec![field])),
            Language::Python,
            GenerateOptions::default(),
            "view infrastructure",
        )?;
    }
    let mut field = scalar("buffer");
    field.targets.insert(Language::Python, named("payload"));
    generate(&schema(record("Root", vec![field])), Language::Python)?;
    Ok(())
}

#[test]
fn python_object_methods_reserve_only_emitted_accessors() -> Result<(), Box<dyn Error>> {
    for name in [
        "__dir__",
        "__doc__",
        "__eq__",
        "__format__",
        "__ge__",
        "__getstate__",
        "__gt__",
        "__hash__",
        "__le__",
        "__lt__",
        "__ne__",
        "__reduce__",
        "__reduce_ex__",
        "__repr__",
        "__sizeof__",
        "__str__",
        "__subclasshook__",
    ] {
        let mut field = scalar("value");
        field.targets.insert(Language::Python, named(name));
        rejects(
            &schema(record("Root", vec![field.clone()])),
            Language::Python,
            GenerateOptions::default(),
            "inherited object member",
        )?;

        // Without a getter, scalar setters are ordinary set_* methods. Neither
        // they nor input-only dictionary keys replace the inherited methods.
        field
            .targets
            .entry(Language::Python)
            .or_default()
            .generate_getters = Some(false);
        generate(
            &schema(record("Root", vec![field.clone()])),
            Language::Python,
        )?;
        field
            .targets
            .entry(Language::Python)
            .or_default()
            .generate_setters = Some(false);
        generate(&schema(record("Root", vec![field])), Language::Python)?;
    }
    Ok(())
}

fn python_object_names_without_properties_compile_and_run(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let fields = ["__repr__", "__str__", "__eq__"]
        .into_iter()
        .map(|name| {
            let mut field = scalar(name);
            field.targets.insert(Language::Python, named(name));
            field
        })
        .collect::<Vec<_>>();
    let mut source = schema(record("InputOnly", fields.clone()));
    source
        .definitions
        .insert("SetterOnly".into(), record("SetterOnly", fields));
    source
        .exports
        .insert("SetterOnly".into(), Type::Named("SetterOnly".into()));
    for (name, definition) in &mut source.definitions {
        definition.targets.insert(
            Language::Python,
            TargetOptions {
                generate_getters: Some(false),
                generate_setters: Some(name == "SetterOnly"),
                ..Default::default()
            },
        );
    }
    save_reference_fixture(fixtures, &source, Language::Python, "object_names.py")
}

#[test]
fn enum_tag_accessors_reserve_their_final_names() -> Result<(), Box<dyn Error>> {
    for language in [
        Language::Cpp,
        Language::CSharp,
        Language::TypeScript,
        Language::Python,
        Language::Dart,
    ] {
        let collision = enum_schema(language, "_", scalar("tag"));
        rejects(
            &collision,
            language,
            GenerateOptions::default(),
            "enum tag accessor",
        )?;

        let mut field = scalar("tag");
        field.targets.insert(language, named("payload"));
        generate(&enum_schema(language, "_", field), language)?;

        let mut field = scalar("tag");
        field.targets.insert(language, named("tag"));
        let mut naming = NamingPolicy::for_language(language);
        for kind in [SymbolKind::Property, SymbolKind::Field] {
            naming.rules.push(NamingRule {
                selector: SymbolSelector {
                    kind: Some(kind),
                    visibility: Some(Visibility::Public),
                    ..Default::default()
                },
                style: NameStyle {
                    case: NameCase::SnakeCase,
                    prefix: "prefix_".into(),
                    suffix: String::new(),
                },
            });
        }
        rejects(
            &enum_schema(language, "_", field),
            language,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
            "prefix_tag",
        )?;
    }
    Ok(())
}

#[test]
fn omitted_enum_tag_accessors_do_not_reserve_names() -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let mut schema = enum_schema(language, "_", scalar("tag"));
        schema
            .definitions
            .get_mut("Choice")
            .ok_or("missing choice")?
            .targets
            .insert(
                language,
                TargetOptions {
                    generate_getters: Some(false),
                    ..Default::default()
                },
            );
        generate(&schema, language)?;
    }
    Ok(())
}

#[test]
fn inherited_object_names_are_rejected_after_casing() -> Result<(), Box<dyn Error>> {
    for (language, fields) in [
        (
            Language::Dart,
            &["to_string", "runtime_type", "hash_code", "no_such_method"][..],
        ),
        (
            Language::CSharp,
            &[
                "to_string",
                "get_type",
                "get_hash_code",
                "equals",
                "reference_equals",
                "memberwise_clone",
                "finalize",
            ][..],
        ),
    ] {
        for name in fields {
            rejects(
                &schema(record("Root", vec![scalar(name)])),
                language,
                GenerateOptions::default(),
                "inherited object member",
            )?;
            let mut field = scalar(name);
            field.targets.insert(language, named("payload"));
            generate(&schema(record("Root", vec![field])), language)?;
        }
    }
    Ok(())
}

#[test]
fn native_and_view_object_member_scopes_are_checked_independently() -> Result<(), Box<dyn Error>> {
    let source = schema(record("Root", vec![scalar("to_string")]));
    for language in [Language::Dart, Language::CSharp] {
        let mut naming = NamingPolicy::for_language(language);
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(if language == Language::Dart {
                    SymbolKind::Field
                } else {
                    SymbolKind::Property
                }),
                visibility: Some(Visibility::Public),
                require: vec![Modifier::Readonly],
                ..Default::default()
            },
            style: NameStyle {
                case: NameCase::PascalCase,
                prefix: "Input".into(),
                suffix: String::new(),
            },
        });
        rejects(
            &source,
            language,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
            "field accessor",
        )?;
    }
    rejects(
        &enum_schema(Language::CSharp, "ToString", scalar("number")),
        Language::CSharp,
        GenerateOptions::default(),
        "inherited object member",
    )?;
    // Dart alternatives are classes, not members on the enclosing enum value.
    generate(
        &enum_schema(Language::Dart, "toString", scalar("number")),
        Language::Dart,
    )?;
    Ok(())
}

#[test]
fn csharp_binding_class_reserves_its_final_name() -> Result<(), Box<dyn Error>> {
    let mut source = schema(record(
        "Root",
        vec![Field::new("child", Type::Named("Payload".into()))],
    ));
    let mut payload = record("Payload", vec![scalar("number")]);
    payload.targets.insert(Language::CSharp, named("Bindings"));
    source.definitions.insert("Payload".into(), payload);
    // The schema's base name is no longer an emitted input declaration.
    generate(&source, Language::CSharp)?;
    for case in [NameCase::PascalCase, NameCase::SnakeCase] {
        source
            .definitions
            .get_mut("Payload")
            .ok_or("payload")?
            .targets
            .insert(Language::CSharp, named(&case.apply("Bindings")));
        let mut naming = NamingPolicy::for_language(Language::CSharp);
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Type),
                ..Default::default()
            },
            style: NameStyle {
                case,
                prefix: String::new(),
                suffix: "Input".into(),
            },
        });
        rejects(
            &source,
            Language::CSharp,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
            "generated binding class",
        )?;
    }
    Ok(())
}

#[test]
fn csharp_alias_views_reserve_their_actual_type_names() -> Result<(), Box<dyn Error>> {
    for field_name in ["secondary_view", "secondary_span_view"] {
        let mut source = schema(record("Primary", vec![scalar(field_name)]));
        source
            .exports
            .insert("Secondary".into(), Type::Named("Primary".into()));
        rejects(
            &source,
            Language::CSharp,
            GenerateOptions::default(),
            "enclosing view type",
        )?;
        let mut field = scalar(field_name);
        field.targets.insert(Language::CSharp, named("Payload"));
        source
            .definitions
            .insert("Primary".into(), record("Primary", vec![field]));
        generate(&source, Language::CSharp)?;
    }

    // Export aliases currently emit a mutable Memory/Span pair, without extra
    // readonly aliases. Do not reserve a class name that is never declared.
    let mut source = schema(record(
        "Primary",
        vec![
            scalar("secondary_view_read_only"),
            scalar("secondary_span_view_read_only"),
        ],
    ));
    source
        .exports
        .insert("Secondary".into(), Type::Named("Primary".into()));
    generate(&source, Language::CSharp)?;
    Ok(())
}

#[test]
fn csharp_collection_helpers_do_not_conflict_with_input_suffixes() -> Result<(), Box<dyn Error>> {
    let item = Type::Named("Item".into());
    for collection in [
        Type::Sequence(Box::new(item.clone())),
        Type::Array {
            element: Box::new(item.clone()),
            length: 2,
        },
        Type::Sequence(Box::new(Type::Optional(Box::new(Type::Boxed(Box::new(
            item.clone(),
        )))))),
        Type::Sequence(Box::new(Type::Sequence(Box::new(item.clone())))),
    ] {
        let mut source = schema(record("Root", vec![Field::new("items", collection)]));
        source.definitions.insert(
            "Item".into(),
            record("ElementAccess", vec![scalar("number")]),
        );
        generate(&source, Language::CSharp)?;
    }
    let mut source = schema(record(
        "Root",
        vec![
            Field::new("item", item),
            Field::new(
                "numbers",
                Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
            ),
        ],
    ));
    source.definitions.insert(
        "Item".into(),
        record("ElementAccess", vec![scalar("number")]),
    );
    generate(&source, Language::CSharp)?;
    Ok(())
}

#[test]
fn csharp_native_view_accessors_reserve_their_final_names() -> Result<(), Box<dyn Error>> {
    for field in ["as_span", "to_value"] {
        rejects(
            &schema(record("Root", vec![scalar(field)])),
            Language::CSharp,
            GenerateOptions::default(),
            "native view access",
        )?;
        let mut renamed = scalar(field);
        renamed.targets.insert(Language::CSharp, named("Payload"));
        generate(&schema(record("Root", vec![renamed])), Language::CSharp)?;
    }
    // Records do not expose a collection's native-span method. Payload-bearing
    // records also omit ToValue because their owned layout is not the wire layout.
    generate(
        &schema(record(
            "Root",
            vec![
                scalar("try_get_native_span"),
                Field::new("to_value", Type::String),
            ],
        )),
        Language::CSharp,
    )?;
    Ok(())
}

#[test]
fn csharp_inline_and_scoped_types_reserve_their_names() -> Result<(), Box<dyn Error>> {
    for (name, conflict) in [
        ("FixedArray3", false),
        ("RootSpanView", false),
        ("RootSpanViewReadOnly", false),
        ("RootSpan", true),
    ] {
        let mut source = schema(record(
            "Root",
            vec![
                Field::new(
                    "numbers",
                    Type::Array {
                        element: Box::new(Type::Scalar(Scalar::U32)),
                        length: 3,
                    },
                ),
                Field::new("child", Type::Named("Child".into())),
            ],
        ));
        source
            .definitions
            .insert("Child".into(), record(name, vec![scalar("number")]));
        if conflict {
            rejects(
                &source,
                Language::CSharp,
                GenerateOptions::default(),
                "span view",
            )?;
        } else {
            // Input suffixes separate these names from helper/view declarations.
            generate(&source, Language::CSharp)?;
        }
    }
    Ok(())
}

#[test]
fn csharp_collection_views_reserve_their_structural_names() -> Result<(), Box<dyn Error>> {
    let mut source = schema(record(
        "Root",
        vec![
            Field::new(
                "numbers",
                Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
            ),
            Field::new("item", Type::Named("Item".into())),
        ],
    ));
    source.definitions.insert(
        "Item".into(),
        record("SequenceOfU32", vec![scalar("number")]),
    );
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions::default(),
        "view",
    )?;
    source
        .definitions
        .get_mut("Item")
        .ok_or("missing item")?
        .targets
        .insert(Language::CSharp, named("IntrlaySequenceOfU32View"));
    generate(&source, Language::CSharp)?;
    Ok(())
}

#[test]
fn cpp_infrastructure_collisions_follow_actual_class_scopes() -> Result<(), Box<dyn Error>> {
    for (backing, field) in [
        ("buffer", scalar("number")),
        (
            "storage",
            Field::new(
                "numbers",
                Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
            ),
        ),
    ] {
        let source = schema(record("Root", vec![field]));
        let naming = NamingPolicy {
            fallback: NameStyle::new(NameCase::SnakeCase),
            rules: vec![
                NamingRule {
                    selector: SymbolSelector {
                        kind: Some(SymbolKind::Field),
                        visibility: Some(Visibility::Private),
                        ..Default::default()
                    },
                    style: NameStyle {
                        suffix: "_bytes".into(),
                        ..NameStyle::new(NameCase::SnakeCase)
                    },
                },
                NamingRule {
                    selector: SymbolSelector {
                        kind: Some(SymbolKind::Property),
                        visibility: Some(Visibility::Public),
                        ..Default::default()
                    },
                    style: NameStyle {
                        prefix: format!("{backing}_"),
                        ..NameStyle::new(NameCase::SnakeCase)
                    },
                },
            ],
        };
        rejects(
            &source,
            Language::Cpp,
            GenerateOptions {
                naming: Some(naming),
                ..Default::default()
            },
            "view infrastructure",
        )?;
    }
    Ok(())
}

#[test]
fn view_accessors_cannot_shadow_storage() -> Result<(), Box<dyn Error>> {
    for language in [Language::Cpp, Language::CSharp] {
        for (accessor, kind, roots) in [
            (
                "value",
                SymbolKind::Property,
                vec![
                    Type::Scalar(Scalar::U32),
                    Type::Unit,
                    Type::String,
                    Type::Optional(Box::new(Type::Scalar(Scalar::U32))),
                    Type::Boxed(Box::new(Type::Scalar(Scalar::U32))),
                ],
            ),
            ("set", SymbolKind::Method, vec![Type::Scalar(Scalar::U32)]),
            (
                "count",
                SymbolKind::Property,
                vec![
                    Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
                    Type::Array {
                        element: Box::new(Type::Scalar(Scalar::U32)),
                        length: 3,
                    },
                ],
            ),
        ] {
            if language == Language::Cpp && accessor == "count" {
                continue;
            }
            let mut roots = roots;
            if language == Language::CSharp && accessor == "set" {
                roots.extend([
                    Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
                    Type::Array {
                        element: Box::new(Type::Scalar(Scalar::U32)),
                        length: 3,
                    },
                ]);
            }
            let mut naming = NamingPolicy::for_language(language);
            naming.rules.extend([
                NamingRule {
                    selector: SymbolSelector {
                        kind: Some(kind),
                        visibility: Some(Visibility::Public),
                        ..Default::default()
                    },
                    style: NameStyle {
                        suffix: "_buffer".into(),
                        ..NameStyle::new(NameCase::SnakeCase)
                    },
                },
                NamingRule {
                    selector: SymbolSelector {
                        kind: Some(SymbolKind::Field),
                        visibility: Some(Visibility::Private),
                        ..Default::default()
                    },
                    style: NameStyle {
                        prefix: format!("{accessor}_"),
                        ..NameStyle::new(NameCase::SnakeCase)
                    },
                },
            ]);
            for root in roots {
                let source = Schema {
                    exports: BTreeMap::from([("Root".into(), root)]),
                    definitions: BTreeMap::new(),
                };
                generate(&source, language)?;
                rejects(
                    &source,
                    language,
                    GenerateOptions {
                        naming: Some(naming.clone()),
                        ..Default::default()
                    },
                    &format!("generated name {accessor}_buffer conflicts"),
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn cpp_collection_facades_always_reserve_replace() -> Result<(), Box<dyn Error>> {
    let mut naming = NamingPolicy::for_language(Language::Cpp);
    naming.rules.extend([
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Method),
                ..Default::default()
            },
            style: NameStyle {
                suffix: "_storage".into(),
                ..NameStyle::new(NameCase::SnakeCase)
            },
        },
        NamingRule {
            selector: SymbolSelector {
                kind: Some(SymbolKind::Field),
                visibility: Some(Visibility::Private),
                ..Default::default()
            },
            style: NameStyle {
                prefix: "replace_".into(),
                ..NameStyle::new(NameCase::SnakeCase)
            },
        },
    ]);
    for root in [
        Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
        Type::Array {
            element: Box::new(Type::Scalar(Scalar::U32)),
            length: 3,
        },
        Type::Sequence(Box::new(Type::String)),
    ] {
        let source = Schema {
            exports: BTreeMap::from([("Root".into(), root)]),
            definitions: BTreeMap::new(),
        };
        rejects(
            &source,
            Language::Cpp,
            GenerateOptions {
                naming: Some(naming.clone()),
                ..Default::default()
            },
            "generated name replace_storage conflicts",
        )?;
    }
    Ok(())
}

#[test]
fn cpp_view_scopes_reserve_actual_template_alias_and_macro_names() -> Result<(), Box<dyn Error>> {
    for name in [
        "Byte",
        "BasicRootView",
        "Metadata",
        "INTRLAY_MEMBER",
        "INTRLAY_RESTRICT",
    ] {
        let mut field = scalar("number");
        field.targets.insert(Language::Cpp, named(name));
        rejects(
            &schema(record("Root", vec![field])),
            Language::Cpp,
            GenerateOptions::default(),
            name,
        )?;
    }
    // The export is now INTRLAY_MEMBERInput, so it does not invoke the macro.
    generate(
        &schema(record("INTRLAY_MEMBER", vec![scalar("number")])),
        Language::Cpp,
    )?;
    let mut root = record("Root", vec![scalar("number")]);
    root.targets
        .insert(Language::Cpp, named("INTRLAY_RESTRICT"));
    rejects(
        &schema(root),
        Language::Cpp,
        GenerateOptions::default(),
        "reserved Cpp identifier",
    )?;
    generate(
        &schema(record("IntrlayRestrict", vec![scalar("intrlay_restrict")])),
        Language::Cpp,
    )?;
    // The include-side implementation no longer declares these local aliases.
    for name in ["Factory", "ElementAccess"] {
        let mut field = scalar("number");
        field.targets.insert(Language::Cpp, named(name));
        generate(&schema(record("Root", vec![field])), Language::Cpp)?;
    }
    Ok(())
}

#[test]
fn dart_loader_names_follow_the_emitted_cache_and_getter() -> Result<(), Box<dyn Error>> {
    let mut duplicate = scalar("other");
    duplicate
        .targets
        .insert(Language::Dart, named("_loadChild"));
    let mut child = Field::new(
        "child",
        Type::Optional(Box::new(Type::Boxed(Box::new(Type::Named("Child".into()))))),
    );
    let mut source = schema(record("Root", vec![child.clone(), duplicate.clone()]));
    source
        .definitions
        .insert("Child".into(), record("Child", vec![scalar("number")]));
    rejects(
        &source,
        Language::Dart,
        GenerateOptions::default(),
        "cached child loader",
    )?;

    child.targets.insert(
        Language::Dart,
        TargetOptions {
            generate_getters: Some(false),
            generate_setters: Some(false),
            ..Default::default()
        },
    );
    source.definitions.insert(
        "Root".into(),
        record("Root", vec![child, duplicate.clone()]),
    );
    generate(&source, Language::Dart)?;

    source.definitions.insert(
        "Root".into(),
        record(
            "Root",
            vec![Field::new("child", Type::Named("Child".into())), duplicate],
        ),
    );
    generate(&source, Language::Dart)?;

    // Tagged accessors update their cache inline, so no loader reserves a name.
    let mut tagged = enum_schema(
        Language::Dart,
        "Data",
        Field::new(
            "child",
            Type::Optional(Box::new(Type::Boxed(Box::new(Type::Named("Child".into()))))),
        ),
    );
    tagged
        .definitions
        .insert("Child".into(), record("Child", vec![scalar("number")]));
    let DefinitionKind::Enum(variants) = &mut tagged
        .definitions
        .get_mut("Choice")
        .ok_or("missing Choice")?
        .kind
    else {
        unreachable!()
    };
    variants.push(Variant {
        name: "Load".into(),
        tag: 1,
        fields: vec![scalar("data_child")],
        synthetic_payload: false,
        docs: String::new(),
        targets: BTreeMap::new(),
    });
    let mut naming = NamingPolicy::for_language(Language::Dart);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Method),
            visibility: Some(Visibility::Private),
            ..Default::default()
        },
        style: NameStyle::new(NameCase::CamelCase),
    });
    generate_with_options(
        &tagged,
        Language::Dart,
        GenerateOptions {
            naming: Some(naming),
            ..Default::default()
        },
    )?;
    Ok(())
}

fn renamed_fields_and_unrelated_helper_names_generate_compilable_csharp(
    fixtures: &mut Fixtures,
) -> Result<(), Box<dyn Error>> {
    let mut field = scalar("to_string");
    field.targets.insert(Language::CSharp, named("Payload"));
    let mut source = schema(record(
        "NameScopeRoot",
        vec![
            field,
            Field::new("item", Type::Named("Item".into())),
            Field::new(
                "numbers",
                Type::Sequence(Box::new(Type::Scalar(Scalar::U32))),
            ),
        ],
    ));
    source.definitions.insert(
        "Item".into(),
        record("ElementAccess", vec![scalar("number")]),
    );
    let generated = generate_with_options(
        &source,
        Language::CSharp,
        GenerateOptions {
            csharp: intrlay_codegen::CSharpOptions {
                namespace: Some("EdgeCases.NameScopes".into()),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let directory = std::path::PathBuf::from("edge-cases");

    fixtures.insert(directory.join("name-scopes.cs"), generated)?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    omitted_factories_generate_compilable_members(fixtures)?;
    collection_scopes_generate_compilable_members(fixtures)?;
    dart_members_cannot_shadow_referenced_codecs(fixtures)?;
    dart_members_reserve_only_types_used_in_their_class(fixtures)?;
    python_annotations_reserve_types_but_method_bodies_keep_module_lookup(fixtures)?;
    python_accepted_enum_names_compile_and_run(fixtures)?;
    python_object_names_without_properties_compile_and_run(fixtures)?;
    csharp_intrinsic_keywords_are_rejected_only_in_unescaped_declarations(fixtures)?;
    rust_derives_use_the_names_validated_by_codegen(fixtures)?;
    rust_enum_mutators_use_configured_property_names(fixtures)?;
    rust_enum_payloads_do_not_declare_individual_getters(fixtures)?;
    rust_enum_derive_names_are_valid_identifiers(fixtures)?;
    renamed_fields_and_unrelated_helper_names_generate_compilable_csharp(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn rust_enum_derive_names_are_valid_identifiers() -> Result<(), Box<dyn std::error::Error>> {
        super::rust_enum_derive_names_are_valid_identifiers(&mut super::Fixtures::default())
    }

    #[test]
    fn rust_enum_payloads_do_not_declare_individual_getters()
    -> Result<(), Box<dyn std::error::Error>> {
        super::rust_enum_payloads_do_not_declare_individual_getters(&mut super::Fixtures::default())
    }

    #[test]
    fn csharp_intrinsic_keywords_are_rejected_only_in_unescaped_declarations()
    -> Result<(), Box<dyn std::error::Error>> {
        super::csharp_intrinsic_keywords_are_rejected_only_in_unescaped_declarations(
            &mut super::Fixtures::default(),
        )
    }
    #[test]
    fn rust_derives_use_the_names_validated_by_codegen() -> Result<(), Box<dyn std::error::Error>> {
        super::rust_derives_use_the_names_validated_by_codegen(&mut super::Fixtures::default())
    }
}

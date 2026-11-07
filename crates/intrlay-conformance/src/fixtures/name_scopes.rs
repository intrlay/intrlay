//! Name diagnostics follow the declarations actually emitted in each target scope.

use super::Fixtures;

#[cfg(test)]
use intrlay_codegen::Modifier;

use std::{collections::BTreeMap, error::Error};

use intrlay_codegen::{
    GenerateOptions, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility,
    generate, generate_with_options,
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
        "\ncore.int checkPrivate(Packet value, PacketView view) => value._writePacket + view._writePacket;\n",
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
        "\ncore.int checkPrivate(Packet value, PacketView view) {\n  view._measurePacket = value._measurePacket;\n  return view._measurePacket;\n}\n",
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
        "\ncore.int checkPrivate(Packet value, PacketView view) {\n  view._writeChild = value._writeChild;\n  return view._writeChild;\n}\n",
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
    for name in ["Child", "ChildView"] {
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
    for name in ["Child", "ChildView"] {
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
                csharp_namespace: Some(format!("Example.{name}")),
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
            csharp_namespace: Some("EdgeCases.Intrinsics".into()),
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
    for name in ["Root", "RootView"] {
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
    field.targets.insert(Language::Dart, named("ChoicePresent"));
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
fn csharp_native_span_names_are_checked_without_construction() -> Result<(), Box<dyn Error>> {
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
        rejects(
            &source,
            Language::CSharp,
            GenerateOptions::default(),
            "collection infrastructure",
        )?;
        source
            .definitions
            .get_mut("Item")
            .ok_or("item")?
            .targets
            .insert(Language::CSharp, named("Entry"));
        generate(&source, Language::CSharp)?;
    }
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

fn enum_schema(language: Language, variant: &str, field: Field) -> Schema {
    let mut root = record("Choice", Vec::new());
    root.kind = DefinitionKind::Enum(vec![Variant {
        name: "Present".into(),
        tag: 0,
        fields: vec![field],
        synthetic_payload: false,
        docs: String::new(),
        targets: BTreeMap::from([(language, named(variant))]),
    }]);
    schema(root)
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
        .ok_or("expected a generated-name collision")?;
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
    source
        .definitions
        .insert("Payload".into(), record("Bindings", vec![scalar("number")]));
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions::default(),
        "generated binding class",
    )?;

    let mut naming = NamingPolicy::for_language(Language::CSharp);
    naming.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Type),
            ..Default::default()
        },
        style: NameStyle::new(NameCase::SnakeCase),
    });
    source
        .definitions
        .get_mut("Payload")
        .ok_or("missing payload")?
        .targets
        .insert(Language::CSharp, named("bindings"));
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions {
            naming: Some(naming),
            ..Default::default()
        },
        "bindings",
    )?;
    source
        .definitions
        .get_mut("Payload")
        .ok_or("missing payload")?
        .targets
        .insert(Language::CSharp, named("Payload"));
    generate(&source, Language::CSharp)?;
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
fn csharp_collection_helpers_reserve_only_visible_native_type_names() -> Result<(), Box<dyn Error>>
{
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
        rejects(
            &source,
            Language::CSharp,
            GenerateOptions::default(),
            "collection infrastructure",
        )?;
        source
            .definitions
            .get_mut("Item")
            .ok_or("missing item")?
            .targets
            .insert(Language::CSharp, named("Entry"));
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
    for field in ["as_span", "to_value", "try_get_native_span"] {
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
    Ok(())
}

#[test]
fn csharp_inline_and_scoped_types_reserve_their_names() -> Result<(), Box<dyn Error>> {
    for (name, role) in [
        ("FixedArray3", "native inline array"),
        ("RootSpanView", "span view"),
        ("RootSpanViewReadOnly", "read-only span view"),
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
        rejects(&source, Language::CSharp, GenerateOptions::default(), role)?;
        source
            .definitions
            .get_mut("Child")
            .ok_or("missing child")?
            .targets
            .insert(Language::CSharp, named("Payload"));
        generate(&source, Language::CSharp)?;
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
        record("SequenceOfU32View", vec![scalar("number")]),
    );
    rejects(
        &source,
        Language::CSharp,
        GenerateOptions::default(),
        "source value and view",
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
    rejects(
        &schema(record("INTRLAY_MEMBER", vec![scalar("number")])),
        Language::Cpp,
        GenerateOptions::default(),
        "generated member-access macro",
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
            csharp_namespace: Some("EdgeCases.NameScopes".into()),
            ..Default::default()
        },
    )?;
    let directory = std::path::PathBuf::from("edge-cases");

    fixtures.insert(directory.join("name-scopes.cs"), generated)?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    dart_members_cannot_shadow_referenced_codecs(fixtures)?;
    dart_members_reserve_only_types_used_in_their_class(fixtures)?;
    python_annotations_reserve_types_but_method_bodies_keep_module_lookup(fixtures)?;
    csharp_intrinsic_keywords_are_rejected_only_in_unescaped_declarations(fixtures)?;
    rust_derives_use_the_names_validated_by_codegen(fixtures)?;
    rust_enum_mutators_use_configured_property_names(fixtures)?;
    renamed_fields_and_unrelated_helper_names_generate_compilable_csharp(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
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

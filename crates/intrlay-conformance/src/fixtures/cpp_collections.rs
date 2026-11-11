//! Shared C++ collection facades retain metadata, recursion, and naming policies.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, NameCase, Registry},
};
use intrlay_codegen::{
    GenerateOptions, Modifier, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector,
    Visibility, generate_with_options,
};

#[derive(Intrlay)]
struct Branch {
    value: u32,
    children: Vec<Branch>,
}

#[derive(Intrlay)]
#[repr(align(16))]
struct Padded {
    value: f32,
}

#[derive(Intrlay)]
#[intrlay(create = false)]
struct BorrowOnly {
    values: [u32; 2],
}

#[derive(Intrlay)]
struct Collections {
    fixed: [f32; 3],
    nested: Vec<[f32; 3]>,
    optional: [Option<Box<u32>>; 3],
    tree: Vec<Branch>,
    padded: [Padded; 2],
    empty: [f64; 0],
    units: Vec<()>,
    flags: [bool; 2],
    borrowed: BorrowOnly,
}

#[derive(Intrlay)]
struct BooleanInputs {
    scalar: bool,
    fixed: [bool; 3],
    dynamic: Vec<bool>,
    maybe: Option<bool>,
    boxed: Box<bool>,
}

fn generate_boolean_inputs(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<BooleanInputs>("BooleanInputs");
    fixtures.insert(
        "cpp-collections/boolean-inputs.hpp",
        generate_with_options(
            &registry.finish(),
            Language::Cpp,
            GenerateOptions {
                cpp: intrlay_codegen::CppOptions {
                    namespace: Some("cpp_collections".into()),
                },
                ..Default::default()
            },
        )?,
    )?;
    fixtures.insert(
        "cpp-collections/boolean-inputs.bin",
        BooleanInputsView::create(&BooleanInputs {
            scalar: true,
            fixed: [true, true, false],
            dynamic: vec![true, false, true],
            maybe: Some(true),
            boxed: Box::new(true),
        })?
        .as_bytes(),
    )?;
    Ok(())
}

fn generate_cpp_shared_collections(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Collections>("Collections");
    registry.export::<[f32; 3]>("Triplet");
    registry.export::<[(); 3]>("UnitTriplet");
    registry.export::<Vec<Option<Box<u32>>>>("OptionalSequence");
    let schema = registry.finish();
    let directory = std::path::PathBuf::from("cpp-collections");

    let source = generate_with_options(
        &schema,
        Language::Cpp,
        GenerateOptions {
            cpp: intrlay_codegen::CppOptions {
                namespace: Some("cpp_collections".into()),
            },
            ..GenerateOptions::default()
        },
    )?;
    assert!(source.contains("using BasicIntrlayArray3OfF32View = ::intrlay::ArrayView<"));
    assert!(!source.contains("class BasicIntrlayArray"));
    assert!(!source.contains("class BasicIntrlaySequence"));
    assert!(!source.contains("class CollectionView"));
    fixtures.insert(directory.join("bindings.hpp"), source)?;

    // Exercise all collection member categories, including static factories.
    let mut naming = NamingPolicy::for_language(Language::Cpp);
    for (kind, prefix, require, exclude) in [
        (SymbolKind::Type, "Api", vec![], vec![]),
        (SymbolKind::Property, "Read", vec![], vec![]),
        (SymbolKind::Method, "Access", vec![], vec![Modifier::Static]),
        (SymbolKind::Method, "Build", vec![Modifier::Static], vec![]),
        (SymbolKind::Field, "Layout", vec![Modifier::Const], vec![]),
    ] {
        naming.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(kind),
                visibility: Some(Visibility::Public),
                require,
                exclude,
            },
            style: NameStyle {
                case: NameCase::PascalCase,
                prefix: prefix.into(),
                suffix: String::new(),
            },
        });
    }
    let source = generate_with_options(
        &schema,
        Language::Cpp,
        GenerateOptions {
            cpp: intrlay_codegen::CppOptions {
                namespace: Some("cpp_collections::custom".into()),
            },
            naming: Some(naming),
            ..GenerateOptions::default()
        },
    )?;
    assert_eq!(source.matches("class CollectionView {").count(), 1);
    assert!(!source.contains("class BasicIntrlayArray"));
    fixtures.insert(directory.join("custom.hpp"), source)?;

    let input = Collections {
        fixed: [1.0, 2.0, 3.0],
        nested: vec![[4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
        optional: [Some(Box::new(11)), None, Some(Box::new(13))],
        tree: vec![Branch {
            value: 17,
            children: vec![Branch {
                value: 19,
                children: vec![],
            }],
        }],
        padded: [Padded { value: 23.0 }, Padded { value: 29.0 }],
        empty: [],
        units: vec![(), (), ()],
        flags: [true, false],
        borrowed: BorrowOnly { values: [31, 37] },
    };
    fixtures.insert(
        directory.join("collections.bin"),
        CollectionsView::create(&input)?.as_bytes(),
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_cpp_shared_collections(fixtures)?;
    generate_boolean_inputs(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn generate_cpp_shared_collections() -> Result<(), Box<dyn std::error::Error>> {
        super::generate_cpp_shared_collections(&mut super::Fixtures::default())
    }
}

//! Shared subtypes and recursive payloads must retain their canonical layouts.

use std::{collections::BTreeMap, error::Error};

use intrlay_schema::{Definition, DefinitionKind, Field, Layouts, Scalar, Schema, Type, Variant};

fn definition(kind: DefinitionKind, alignment: usize) -> Definition {
    Definition {
        name: "Value".into(),
        docs: String::new(),
        kind,
        alignment,
        targets: BTreeMap::new(),
        transparent: false,
    }
}

fn named(name: &str) -> Type {
    Type::Named(name.into())
}

#[test]
fn shared_enum_subtypes_do_not_expand_every_variant_combination() -> Result<(), Box<dyn Error>> {
    let mut schema = Schema::default();
    let mut child = Type::Scalar(Scalar::U32);
    for level in 0..24 {
        let name = format!("Choice{level}");
        let variants = ["Left", "Right"]
            .into_iter()
            .zip(0..)
            .map(|(name, tag)| Variant {
                name: name.into(),
                tag,
                fields: vec![Field::new("value", child.clone())],
                docs: String::new(),
                targets: BTreeMap::new(),
                synthetic_payload: false,
            })
            .collect();
        schema
            .definitions
            .insert(name.clone(), definition(DefinitionKind::Enum(variants), 1));
        child = Type::Named(name);
    }
    schema.exports.insert("Root".into(), child.clone());
    let mut layouts = Layouts::new(&schema);
    assert_eq!(layouts.of(&child)?.size(), 100);
    assert_eq!(layouts.buffer_alignment(&child)?, 4);
    let resolved = layouts.validate()?;
    for level in 0..24 {
        let name = format!("Choice{level}");
        assert_eq!(resolved[&name].size(), 4 * (level + 2));
        assert_eq!(layouts.of(&Type::Named(name.clone()))?, resolved[&name]);
    }
    Ok(())
}

#[test]
fn recursive_payload_alignment_is_independent_of_query_order() -> Result<(), Box<dyn Error>> {
    let mut schema = Schema::default();
    for (name, fields, alignment) in [
        (
            "A",
            vec![
                Field::new("child", Type::Boxed(Box::new(named("B")))),
                Field::new("payload", Type::Sequence(Box::new(named("Aligned")))),
            ],
            1,
        ),
        (
            "B",
            vec![Field::new("parent", Type::Boxed(Box::new(named("A"))))],
            1,
        ),
        (
            "Aligned",
            vec![Field::new("value", Type::Scalar(Scalar::U8))],
            32,
        ),
    ] {
        schema.definitions.insert(
            name.into(),
            definition(DefinitionKind::Record(fields), alignment),
        );
    }
    // B can finish its recursive visit before A reaches its aligned payload.
    // It must not retain that incomplete visit as B's transitive alignment.
    for order in [["A", "B"], ["B", "A"]] {
        let mut layouts = Layouts::new(&schema);
        for name in order.into_iter().chain(order) {
            assert_eq!(layouts.buffer_alignment(&named(name))?, 32);
        }
        assert_eq!(layouts.of(&named("A"))?.size(), 12);
        assert_eq!(layouts.of(&named("B"))?.size(), 4);
        assert_eq!(layouts.validate()?["Aligned"].size(), 32);
    }
    Ok(())
}

#[test]
fn failed_resolution_does_not_cache_partial_layouts() -> Result<(), Box<dyn Error>> {
    let mut schema = Schema::default();
    for (name, field) in [
        ("A", named("B")),
        ("B", named("A")),
        ("Missing", named("Undefined")),
        ("Valid", Type::Scalar(Scalar::U64)),
    ] {
        schema.definitions.insert(
            name.into(),
            definition(DefinitionKind::Record(vec![Field::new("value", field)]), 1),
        );
    }
    let mut layouts = Layouts::new(&schema);
    for _ in 0..2 {
        assert!(
            layouts
                .of(&named("A"))
                .is_err_and(|error| error.0.contains("inline recursive type"))
        );
        assert!(layouts.buffer_alignment(&named("B")).is_err());
        assert!(
            layouts
                .of(&named("Missing"))
                .is_err_and(|error| error.0.contains("undefined type"))
        );
        assert_eq!(layouts.of(&named("Valid"))?.size(), 8);
        assert_eq!(layouts.buffer_alignment(&named("Valid"))?, 8);
        assert!(layouts.validate().is_err());
    }
    Ok(())
}

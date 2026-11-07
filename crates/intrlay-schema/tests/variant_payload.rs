//! Native payload metadata is explicit, validated, and independent of wire layout.

use intrlay_schema::{Field, Layouts, Scalar, Schema, Type, Variant};
use std::collections::BTreeMap;

#[test]
fn existing_metadata_defaults_to_ordinary_fields() -> Result<(), Box<dyn std::error::Error>> {
    let variant: Variant =
        serde_json::from_str(r#"{"name":"Value","tag":0,"fields":[],"docs":""}"#)?;
    assert!(!variant.synthetic_payload);
    Ok(())
}

#[test]
fn synthetic_payload_changes_native_mapping_only() -> Result<(), Box<dyn std::error::Error>> {
    let mut variant = Variant {
        name: "Value".into(),
        tag: 3,
        fields: vec![Field::new("renamed_payload", Type::Scalar(Scalar::U32))],
        synthetic_payload: false,
        docs: String::new(),
        targets: BTreeMap::new(),
    };
    let schema = Schema::default();
    let mut layouts = Layouts::new(&schema);
    let ordinary = layouts.variants(&[variant.clone()], 1)?;
    variant.synthetic_payload = true;
    assert_eq!(layouts.variants(&[variant.clone()], 1)?, ordinary);
    let encoded = serde_json::to_string(&variant)?;
    assert_eq!(serde_json::from_str::<Variant>(&encoded)?, variant);
    Ok(())
}

#[test]
fn synthetic_payload_requires_one_field() {
    let mut variant = Variant {
        name: "Value".into(),
        tag: 0,
        fields: Vec::new(),
        synthetic_payload: true,
        docs: String::new(),
        targets: BTreeMap::new(),
    };
    let schema = Schema::default();
    let mut layouts = Layouts::new(&schema);
    assert!(layouts.variants(&[variant.clone()], 1).is_err());
    variant.fields = vec![
        Field::new("first", Type::Unit),
        Field::new("second", Type::Unit),
    ];
    assert!(layouts.variants(&[variant], 1).is_err());
}

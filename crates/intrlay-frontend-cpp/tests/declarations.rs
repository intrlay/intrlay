//! Export identity and template-pattern checks use real Clang declarations.

use std::{error::Error, path::PathBuf};

use intrlay_frontend_cpp::{Options, extract};
use intrlay_schema::{DefinitionKind, Type};

fn fixture(name: &str, definition: Option<&str>) -> (PathBuf, Options) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut arguments = vec![format!(
        "-I{}",
        directory.join("../../packages/cpp/include").display()
    )];
    if let Some(definition) = definition {
        arguments.push(format!("-D{definition}"));
    }
    (
        directory.join("tests/fixtures").join(name),
        Options {
            arguments,
            ..Options::default()
        },
    )
}

#[test]
fn redeclarations_share_an_export_identity() -> Result<(), Box<dyn Error>> {
    let (path, options) = fixture("redeclarations.hpp", None);
    let schema = extract(path, &options)?;
    assert_eq!(schema.exports.len(), 4);
    assert_eq!(schema.exports["Root"], schema.exports["Alias"]);
    assert_eq!(schema.exports["Root"], Type::Named("Root".into()));
    let DefinitionKind::Record(fields) = &schema.definitions["Root"].kind else {
        return Err("root record expected".into());
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "value");
    Ok(())
}

#[test]
fn distinct_exports_and_conflicting_annotations_still_fail() {
    for definition in [
        "INTRLAY_CONFLICT_SAME_TYPE",
        "INTRLAY_CONFLICT_DIFFERENT_TYPE",
        "INTRLAY_CONFLICT_NAMESPACE",
        "INTRLAY_CONFLICT_ANNOTATION",
    ] {
        let (path, options) = fixture("redeclarations.hpp", Some(definition));
        let error = extract(path, &options)
            .err()
            .unwrap_or_else(|| panic!("{definition} must fail"));
        assert!(
            error.0.contains("duplicate export") || error.0.contains("conflicting"),
            "{error}"
        );
    }
}

#[test]
fn selected_template_patterns_cannot_hide_inherited_fields() -> Result<(), Box<dyn Error>> {
    for case in [0, 1, 2, 3, 4, 8, 9, 11, 13, 15, 17] {
        let (path, options) = fixture("template_bases.hpp", Some(&format!("INTRLAY_CASE={case}")));
        let error = extract(path, &options)
            .err()
            .ok_or_else(|| format!("case {case}: inherited fields were discarded"))?;
        assert!(
            error.0.contains("inheritance requires an explicit mirror"),
            "case {case}: {error}"
        );
    }
    for case in [5, 6, 7, 10, 12, 14, 16] {
        let (path, options) = fixture("template_bases.hpp", Some(&format!("INTRLAY_CASE={case}")));
        let schema = extract(path, &options)?;
        let Type::Named(identity) = &schema.exports["Root"] else {
            return Err("record export expected".into());
        };
        let DefinitionKind::Record(fields) = &schema.definitions[identity].kind else {
            return Err("record definition expected".into());
        };
        assert_eq!(fields.len(), usize::from(case != 7));
    }
    Ok(())
}

#[test]
fn variant_names_resolve_aliases_after_their_first_use() -> Result<(), Box<dyn Error>> {
    let (path, options) = fixture("codec_cases.hpp", None);
    let schema = extract(path, &options)?;
    let Type::Named(identity) = &schema.exports["Endpoints"] else {
        return Err("named variant expected".into());
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[identity].kind else {
        return Err("variant definition expected".into());
    };
    assert_eq!(variants[0].name, "Start");
    assert_eq!(variants[1].name, "End");
    assert_eq!(variants[0].tag, 0);
    assert_eq!(variants[1].tag, 1);
    assert_eq!(variants[0].fields[0].ty, variants[1].fields[0].ty);
    let DefinitionKind::Record(fields) = &schema.definitions["codec_fixture::Repeated"].kind else {
        return Err("record definition expected".into());
    };
    assert_eq!(fields[0].ty, schema.exports["Endpoints"]);
    let DefinitionKind::Record(fields) = &schema.definitions["codec_fixture::CodecCases"].kind
    else {
        return Err("record definition expected".into());
    };
    let Type::Named(identity) = &fields[1].ty else {
        return Err("named alternative expected".into());
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[identity].kind else {
        return Err("variant definition expected".into());
    };
    assert_eq!(variants[0].name, "Monostate");
    assert_eq!(variants[1].name, "ValueRecord");
    Ok(())
}

#[test]
fn variant_names_reject_ambiguous_or_inapplicable_annotations() -> Result<(), Box<dyn Error>> {
    for (definition, diagnostic) in [
        ("INTRLAY_DUPLICATE", "duplicate alternative name"),
        ("INTRLAY_OUT_OF_RANGE", "alternative index 2"),
        ("INTRLAY_NOT_VARIANT", "std::variant alias"),
        ("INTRLAY_CONFLICTING_ALIASES", "conflicting names"),
        ("INTRLAY_COLLIDING_NAMES", "duplicate alternative name"),
    ] {
        let (path, options) = fixture("variant_names.hpp", Some(definition));
        let error = extract(path, &options)
            .err()
            .ok_or("invalid annotation accepted")?;
        assert!(error.0.contains(diagnostic), "{definition}: {error}");
    }
    Ok(())
}

#[test]
fn recursive_variant_names_resolve_completed_payloads() -> Result<(), Box<dyn Error>> {
    let (path, options) = fixture("variant_names.hpp", None);
    let schema = extract(path, &options)?;
    let Type::Named(identity) = &schema.exports["Branch"] else {
        return Err("named variant expected".into());
    };
    let DefinitionKind::Enum(variants) = &schema.definitions[identity].kind else {
        return Err("variant definition expected".into());
    };
    assert_eq!(variants[1].name, "BoxOfNode");
    assert!(schema.definitions.contains_key("Node"));
    Ok(())
}

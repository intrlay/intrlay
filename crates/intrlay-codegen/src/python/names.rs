//! Default naming for generated Python declarations.

use intrlay_schema::NameCase;

use crate::{
    Modifier, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility,
};

pub fn policy() -> NamingPolicy {
    let mut policy = NamingPolicy::conventional(NameCase::SnakeCase);
    policy.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Field),
            require: vec![Modifier::Const],
            ..SymbolSelector::default()
        },
        style: NameStyle::new(NameCase::ScreamingSnakeCase),
    });
    policy.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Field),
            visibility: Some(Visibility::Private),
            ..SymbolSelector::default()
        },
        style: NameStyle {
            case: NameCase::SnakeCase,
            prefix: "_".into(),
            suffix: String::new(),
        },
    });
    for (kind, case) in [
        (SymbolKind::Type, NameCase::PascalCase),
        (SymbolKind::Function, NameCase::SnakeCase),
    ] {
        policy.rules.push(NamingRule {
            selector: SymbolSelector {
                kind: Some(kind),
                visibility: Some(Visibility::Internal),
                ..SymbolSelector::default()
            },
            style: NameStyle {
                case,
                prefix: "_".into(),
                suffix: String::new(),
            },
        });
    }
    policy
}

/// References inside class bodies must not trigger Python's private-name mangling.
pub(crate) fn reference(name: &str) -> String {
    if name.starts_with("__") && !name.ends_with("__") {
        format!("_intrlay{name}")
    } else {
        name.into()
    }
}

pub const KEYWORDS: &str = "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield";

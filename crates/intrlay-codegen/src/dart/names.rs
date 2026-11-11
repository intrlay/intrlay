//! Default naming for generated Dart declarations.

use intrlay_schema::NameCase;

use crate::{NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility};

pub(super) fn enum_from_tag(plan: &crate::plan::TypeGraph) -> String {
    plan.names.method("from_tag", true)
}

pub fn policy() -> NamingPolicy {
    let mut policy = NamingPolicy::conventional(NameCase::CamelCase);
    policy.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Field),
            visibility: Some(Visibility::Private),
            ..SymbolSelector::default()
        },
        style: NameStyle {
            case: NameCase::CamelCase,
            prefix: "_".into(),
            suffix: String::new(),
        },
    });
    policy.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Method),
            visibility: Some(Visibility::Private),
            ..SymbolSelector::default()
        },
        style: NameStyle {
            case: NameCase::CamelCase,
            prefix: "_".into(),
            suffix: String::new(),
        },
    });
    for (kind, case) in [
        (SymbolKind::Type, NameCase::PascalCase),
        (SymbolKind::Function, NameCase::CamelCase),
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

pub const KEYWORDS: &str = "abstract as assert async await break case catch class const continue covariant default deferred do dynamic else enum export extends extension external factory false final finally for Function get hide if implements import in interface is late library mixin new null on operator part required rethrow return sealed set show static super switch sync this throw true try typedef var void when while with yield";

//! Default naming for generated TypeScript declarations.

use intrlay_schema::NameCase;

use crate::{NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility};

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

pub const KEYWORDS: &str = "break case catch class const continue debugger default delete do else enum export extends false finally for function if import in instanceof new null return super switch this throw true try typeof var void while with implements interface let package private protected public static yield await any boolean number string symbol type unknown never bigint object undefined intrinsic readonly infer keyof unique";

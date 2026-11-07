//! Default naming for generated C# declarations.

use intrlay_schema::NameCase;

use crate::{NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility};

pub fn policy() -> NamingPolicy {
    let mut policy = NamingPolicy::conventional(NameCase::PascalCase);
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
    policy
}

pub const KEYWORDS: &str = "abstract as base bool break byte case catch char checked class const continue decimal default delegate do double else enum event explicit extern false finally fixed float for foreach goto if implicit in int interface internal is lock long namespace new null object operator out override params private protected public readonly ref return sbyte sealed short sizeof stackalloc static string struct switch this throw true try typeof uint ulong unchecked unsafe ushort using virtual void volatile while";

// Schema properties are emitted with @, but type and namespace names are not.
pub const INTRINSIC_KEYWORDS: &[&str] = &["__arglist", "__makeref", "__reftype", "__refvalue"];

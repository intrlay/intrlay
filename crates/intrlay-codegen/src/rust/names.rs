//! Default naming for generated Rust declarations.

use intrlay_schema::NameCase;

use crate::{Modifier, NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector};

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
    policy
}

pub const KEYWORDS: &str = "as break const continue crate else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while async await dyn abstract become box do final macro override priv typeof unsized virtual yield try gen";

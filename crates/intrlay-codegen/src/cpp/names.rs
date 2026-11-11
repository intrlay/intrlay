//! Default naming for generated C++ declarations.

use intrlay_schema::NameCase;

use crate::{NameStyle, NamingPolicy, NamingRule, SymbolKind, SymbolSelector, Visibility};

/// Object-like runtime macros expand in every identifier position.
pub const OBJECT_MACROS: &[&str] = &["INTRLAY_RESTRICT", "INTRLAY_DETAIL_PARENS"];

/// Function-like runtime macros cannot name generated methods or constructors.
pub const FUNCTION_MACROS: &[&str] = &[
    "INTRLAY_MEMBER",
    "INTRLAY_ADAPT_STRUCT",
    "INTRLAY_DETAIL_EXPAND1",
    "INTRLAY_DETAIL_EXPAND2",
    "INTRLAY_DETAIL_EXPAND4",
    "INTRLAY_DETAIL_EXPAND8",
    "INTRLAY_DETAIL_EXPAND16",
    "INTRLAY_DETAIL_EXPAND32",
    "INTRLAY_DETAIL_EXPAND64",
    "INTRLAY_DETAIL_FIELDS_AGAIN",
    "INTRLAY_DETAIL_FIELDS",
];

/// Both generated declarations and original source-member mappings must survive
/// the preprocessor and use ordinary C++ identifiers.
pub fn is_reserved_identifier(name: &str) -> bool {
    OBJECT_MACROS.contains(&name) || crate::name_validation::is_keyword(KEYWORDS, name)
}

pub fn policy() -> NamingPolicy {
    let mut policy = NamingPolicy::conventional(NameCase::SnakeCase);
    policy.rules.push(NamingRule {
        selector: SymbolSelector {
            kind: Some(SymbolKind::Field),
            visibility: Some(Visibility::Private),
            ..SymbolSelector::default()
        },
        style: NameStyle {
            case: NameCase::SnakeCase,
            prefix: String::new(),
            suffix: "_".into(),
        },
    });
    policy
}

pub const KEYWORDS: &str = "alignas alignof and and_eq asm auto bitand bitor bool break case catch char char8_t char16_t char32_t class compl concept const consteval constexpr constinit const_cast continue co_await co_return co_yield decltype default delete do double dynamic_cast else enum explicit export extern false float for friend goto if inline int long mutable namespace new noexcept not not_eq nullptr operator or or_eq private protected public register reinterpret_cast requires return short signed sizeof static static_assert static_cast struct switch template this thread_local throw true try typedef typeid typename union unsigned using virtual void volatile wchar_t while xor xor_eq";

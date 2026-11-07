use intrlay_schema::{Language, NameCase};
use serde::{Deserialize, Serialize};

/// Semantic role of a generated identifier, independent of language syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// A record, class, struct, enum, interface, or type alias.
    Type,
    /// A stored field, including generated implementation state.
    Field,
    /// Buffer-backed value access; also covers accessor methods in Rust/C++.
    Property,
    /// An operation attached to a type.
    Method,
    /// A free-standing function.
    Function,
    /// An enum's named alternative.
    Variant,
    /// A function or constructor parameter.
    Parameter,
    /// A local binding inside a generated function.
    Local,
}

/// Visibility categories used for generated-symbol naming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Publicly exported API.
    Public,
    /// Accessible to derived implementations.
    Protected,
    /// Restricted to the generated module, package, crate, or assembly.
    Internal,
    /// Private implementation detail of its owner.
    Private,
}

/// Language-independent qualifiers relevant to generated symbol names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    /// Associated with the type rather than an instance.
    Static,
    /// Storage cannot be reassigned through this declaration.
    Readonly,
    /// A compile-time constant; also matches Readonly selectors.
    Const,
    /// Asynchronous operation.
    Async,
    /// Declaration without a concrete implementation.
    Abstract,
    /// Implementation may be overridden.
    Virtual,
    /// Replaces an inherited implementation.
    Override,
    /// Prevents inheritance or further overriding.
    Final,
}

/// Metadata for one generated identifier. This is used only during code generation.
#[derive(Clone, Copy, Debug)]
pub struct Symbol<'a> {
    /// Unmodified semantic name supplied by the emitter.
    pub name: &'a str,
    /// Identifier's role in the generated API.
    pub kind: SymbolKind,
    /// Member visibility; parameters and locals can leave this unspecified.
    pub visibility: Option<Visibility>,
    /// Qualifiers present on the generated declaration.
    pub modifiers: &'a [Modifier],
}

impl Symbol<'_> {
    fn has(self, modifier: Modifier) -> bool {
        self.modifiers.contains(&modifier)
            || (modifier == Modifier::Readonly && self.modifiers.contains(&Modifier::Const))
    }
}

/// Conditions for a naming rule. Unspecified conditions match any value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolSelector {
    /// Match one symbol category, or all categories when absent.
    pub kind: Option<SymbolKind>,
    /// Match one visibility level, or any level when absent.
    pub visibility: Option<Visibility>,
    /// Every listed qualifier must be present.
    pub require: Vec<Modifier>,
    /// None of the listed qualifiers may be present.
    pub exclude: Vec<Modifier>,
}

impl SymbolSelector {
    fn matches(&self, symbol: Symbol<'_>) -> bool {
        self.kind.is_none_or(|kind| kind == symbol.kind)
            && self
                .visibility
                .is_none_or(|visibility| Some(visibility) == symbol.visibility)
            && self.require.iter().all(|modifier| symbol.has(*modifier))
            && self.exclude.iter().all(|modifier| !symbol.has(*modifier))
    }
}

/// Built-in casing plus literal affixes, without executable user callbacks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameStyle {
    /// Casing preset applied before affixes.
    pub case: NameCase,
    /// Literal prefix, such as `_` for private fields.
    #[serde(default)]
    pub prefix: String,
    /// Literal suffix, such as `_` for private C++ fields.
    #[serde(default)]
    pub suffix: String,
}

impl NameStyle {
    /// Creates a style with no prefix or suffix.
    #[must_use]
    pub fn new(case: NameCase) -> Self {
        Self {
            case,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    fn apply(&self, name: &str) -> String {
        format!("{}{}{}", self.prefix, self.case.apply(name), self.suffix)
    }
}

/// A selector and the spelling to use for matching identifiers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamingRule {
    /// Conditions evaluated against normalized generated-symbol metadata.
    pub selector: SymbolSelector,
    /// Complete style used when the selector matches.
    pub style: NameStyle,
}

/// Module-wide naming rules. The last matching rule wins.
///
/// Append overrides to a language's defaults. An explicit export name in a source
/// annotation bypasses the entire policy, including prefixes and suffixes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamingPolicy {
    /// Style used when no rule matches.
    pub fallback: NameStyle,
    /// Rules in increasing precedence order.
    pub rules: Vec<NamingRule>,
}

impl NamingPolicy {
    /// Conventional defaults for a generated language module.
    #[must_use]
    pub fn for_language(language: Language) -> Self {
        (crate::backend::for_language(language).naming_policy)()
    }

    /// Common baseline: `PascalCase` types and variants, with caller-selected members.
    pub(crate) fn conventional(member: NameCase) -> Self {
        Self {
            fallback: NameStyle::new(member),
            rules: [SymbolKind::Type, SymbolKind::Variant]
                .into_iter()
                .map(|kind| NamingRule {
                    selector: SymbolSelector {
                        kind: Some(kind),
                        ..SymbolSelector::default()
                    },
                    style: NameStyle::new(NameCase::PascalCase),
                })
                .collect(),
        }
    }

    /// Resolves one automatically generated name according to the ordered rules.
    #[must_use]
    pub fn resolve(&self, symbol: Symbol<'_>) -> String {
        self.rules
            .iter()
            .rev()
            .find(|rule| rule.selector.matches(symbol))
            .map_or(&self.fallback, |rule| &rule.style)
            .apply(symbol.name)
    }
}

/// Settings applied to the entire output module.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerateOptions {
    /// Stable name of this generation unit, independent of its exported types.
    /// Defaults to `Schema`. C++/C# place declarations in a namespace derived
    /// from this name; file-scoped languages already have their own module.
    pub module_name: Option<String>,
    /// Override the output language's conventional naming policy.
    pub naming: Option<NamingPolicy>,
    /// Full namespace for C++ declarations, such as `app::protocol`.
    /// Overrides the default `intrlay_generated::<module_name>` namespace.
    pub cpp_namespace: Option<String>,
    /// Full namespace for C# declarations, such as `App.Protocol`.
    /// Overrides the default `IntrlayGenerated.<ModuleName>` namespace.
    pub csharp_namespace: Option<String>,
}

#[derive(Clone)]
pub struct Names {
    pub policy: NamingPolicy,
}

impl Names {
    pub fn symbol(
        &self,
        source: &str,
        kind: SymbolKind,
        visibility: Option<Visibility>,
        modifiers: &[Modifier],
    ) -> String {
        self.policy.resolve(Symbol {
            name: source,
            kind,
            visibility,
            modifiers,
        })
    }
    pub fn type_name(&self, source: &str) -> String {
        self.policy.resolve(Symbol {
            name: source,
            kind: SymbolKind::Type,
            visibility: Some(Visibility::Public),
            modifiers: &[],
        })
    }
    pub fn member(&self, source: &str) -> String {
        self.policy.resolve(Symbol {
            name: source,
            kind: SymbolKind::Field,
            visibility: Some(Visibility::Public),
            modifiers: &[],
        })
    }
    pub fn method(&self, source: &str, static_: bool) -> String {
        self.policy.resolve(Symbol {
            name: source,
            kind: SymbolKind::Method,
            visibility: Some(Visibility::Public),
            modifiers: if static_ { &[Modifier::Static] } else { &[] },
        })
    }

    pub fn property(&self, source: &str, readonly: bool) -> String {
        self.symbol(
            source,
            SymbolKind::Property,
            Some(Visibility::Public),
            if readonly { &[Modifier::Readonly] } else { &[] },
        )
    }

    pub fn constant(&self, source: &str) -> String {
        self.symbol(
            source,
            SymbolKind::Field,
            Some(Visibility::Public),
            &[Modifier::Static, Modifier::Const],
        )
    }
}

use heck::{ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use serde::{Deserialize, Serialize};

use crate::Language;

/// Built-in field naming policies. Names never participate in buffer layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameCase {
    /// Retain the source spelling, including capitalization and underscores.
    #[serde(rename = "preserve")]
    Preserve,
    /// Lowercase words separated by underscores.
    #[serde(rename = "snake_case")]
    SnakeCase,
    /// An initial lowercase word followed by capitalized words.
    #[serde(rename = "camelCase")]
    CamelCase,
    /// Capitalize the first letter of every word.
    #[serde(rename = "PascalCase")]
    PascalCase,
    /// Uppercase words separated by underscores.
    #[serde(rename = "SCREAMING_SNAKE_CASE")]
    ScreamingSnakeCase,
}

impl NameCase {
    /// Conventional field/property casing for each supported target language.
    #[must_use]
    pub const fn for_language(language: Language) -> Self {
        match language {
            Language::Rust | Language::Cpp | Language::Python => Self::SnakeCase,
            Language::TypeScript | Language::Dart => Self::CamelCase,
            Language::CSharp => Self::PascalCase,
        }
    }

    /// Converts an identifier using a built-in policy, preserving word boundaries.
    #[must_use]
    pub fn apply(self, name: &str) -> String {
        match self {
            Self::Preserve => name.into(),
            Self::SnakeCase => name.to_snake_case(),
            Self::CamelCase => name.to_lower_camel_case(),
            Self::PascalCase => name.to_upper_camel_case(),
            Self::ScreamingSnakeCase => name.to_shouty_snake_case(),
        }
    }

    /// Resolves an annotation's preset name without accepting executable transforms.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "preserve" => Some(Self::Preserve),
            "snake_case" => Some(Self::SnakeCase),
            "camelCase" => Some(Self::CamelCase),
            "PascalCase" => Some(Self::PascalCase),
            "SCREAMING_SNAKE_CASE" => Some(Self::ScreamingSnakeCase),
            _ => None,
        }
    }
}

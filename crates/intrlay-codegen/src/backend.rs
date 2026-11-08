//! Backend registration and the API choices needed by the language-neutral plan.

use intrlay_schema::Language;

use crate::{
    GenerateError, GenerateOptions, Modifier, NamingPolicy, SymbolKind, plan::GenerationPlan,
};

/// The schema always retains array extents; only the target API may erase them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FixedArrayApi {
    /// Preserve the source language's structural array projection.
    Structural,
    /// Expose primitive lanes through a dynamically sized numeric view.
    Numeric,
}

/// Each backend owns its syntax, default naming, and public API representation.
pub struct Backend {
    /// Selects this backend's overrides in source annotations.
    pub language: Language,
    /// Language-owned spelling and accessor declarations shared with validation.
    pub api: &'static dyn crate::api::ApiPolicy,
    /// Conventional names before a caller supplies a custom policy.
    pub naming_policy: fn() -> NamingPolicy,
    /// Module-level import aliases and implementation namespaces that source
    /// declarations must not hide.
    pub reserved_declarations: &'static [&'static str],
    /// Target lexical rules, including keywords permitted in member positions.
    pub is_reserved_name: fn(&str, crate::name_validation::NameKind) -> bool,
    /// Resolves target settings, validates declaration scopes, and emits source
    /// containing semantic name markers for the final naming pass.
    pub generate: fn(&GenerationPlan, &GenerateOptions) -> Result<String, GenerateError>,
    /// Inputs may expose a field or a property depending on the target API.
    pub input_field_kind: SymbolKind,
    /// Qualifiers used when naming the input's members.
    pub input_field_modifiers: &'static [Modifier],
    /// Encoding helpers may be free functions or associated methods.
    pub codec_function_kind: SymbolKind,
    /// Qualifiers used when naming the encoding helpers.
    pub codec_function_modifiers: &'static [Modifier],
    /// Numeric array projections can replace generated collection wrappers.
    pub native_numeric_array_views: bool,
    /// Numeric collection writes use runtime functions without scalar helper calls.
    pub runtime_numeric_array_writers: bool,
    /// Default representation of fixed arrays of primitive numeric elements.
    pub fixed_array_api: FixedArrayApi,
    /// Scalar-compatible field assignments use this target's property or method syntax.
    pub field_setter: crate::api::SetterSyntax,
    /// Root scalar/optional exports can have different property requirements.
    pub root_setter: crate::api::SetterSyntax,
}

/// The only dispatch from a requested language to its implementation.
pub fn for_language(language: Language) -> &'static Backend {
    match language {
        Language::Rust => &crate::rust::BACKEND,
        Language::Cpp => &crate::cpp::BACKEND,
        Language::TypeScript => &crate::typescript::BACKEND,
        Language::CSharp => &crate::csharp::BACKEND,
        Language::Python => &crate::python::BACKEND,
        Language::Dart => &crate::dart::BACKEND,
    }
}

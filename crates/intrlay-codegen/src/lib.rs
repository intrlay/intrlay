//! Static view generation from source-independent, fully resolved layouts.
//!
//! Frontends describe types. This crate resolves header offsets, array strides,
//! payload alignment, and API projections once before emitting target source.

#![forbid(unsafe_code)]

mod api;
mod backend;
mod cpp;
mod csharp;
mod dart;
mod documentation;
mod name_validation;
mod names;
mod plan;
mod python;
mod rust;
mod symbols;
mod typescript;

use std::{error::Error, fmt};

use intrlay_schema::{Language, Schema};
pub use names::{
    GenerateOptions, Modifier, NameStyle, NamingPolicy, NamingRule, Symbol, SymbolKind,
    SymbolSelector, Visibility,
};

/// A schema or target API that cannot produce a valid binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerateError(pub String);

impl fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for GenerateError {}

impl From<fmt::Error> for GenerateError {
    fn from(error: fmt::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<intrlay_schema::LayoutError> for GenerateError {
    fn from(error: intrlay_schema::LayoutError) -> Self {
        Self(error.to_string())
    }
}

/// Generates a standalone binding module, with no runtime schema interpretation.
///
/// # Errors
/// Reports invalid layouts, ambiguous public names, or unsupported API projections.
pub fn generate(schema: &Schema, language: Language) -> Result<String, GenerateError> {
    generate_with_options(schema, language, GenerateOptions::default())
}

/// Generates bindings using one module-wide naming policy.
///
/// Explicit names in source annotations take precedence and are emitted verbatim.
///
/// # Errors
/// Reports schema, projection, or generated identifier conflicts.
pub fn generate_with_options(
    schema: &Schema,
    language: Language,
    options: GenerateOptions,
) -> Result<String, GenerateError> {
    let backend = backend::for_language(language);
    let plan = plan::GenerationPlan::new(schema, backend, options)?;
    name_validation::validate(&plan)?;
    let source = (backend.generate)(&plan)?;
    symbols::render(&plan.encoding.types, &source)
}

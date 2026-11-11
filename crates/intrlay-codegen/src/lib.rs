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
mod options;
mod plan;
mod python;
mod rust;
mod symbols;
mod typescript;

use std::{error::Error, fmt};

use intrlay_schema::{Language, Schema};
pub use names::{
    Modifier, NameStyle, NamingPolicy, NamingRule, Symbol, SymbolKind, SymbolSelector, Visibility,
};

pub use cpp::CppOptions;
pub use csharp::CSharpOptions;
pub use options::GenerateOptions;

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
/// Explicit names are emitted verbatim for direct declarations. Synthesized enum
/// and flattened accessors apply the naming policy to the combined name segments.
///
/// # Errors
/// Reports schema, projection, or generated identifier conflicts.
pub fn generate_with_options(
    schema: &Schema,
    language: Language,
    mut options: GenerateOptions,
) -> Result<String, GenerateError> {
    let backend = backend::for_language(language);
    plan::identifier(options.module_name())?;
    let plan = plan::GenerationPlan::new(schema, backend, options.naming.take())?;
    let source = (backend.generate)(&plan, &options)?;
    symbols::render(&plan.encoding.types, &source)
}

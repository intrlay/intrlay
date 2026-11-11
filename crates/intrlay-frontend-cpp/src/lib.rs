//! C++20 schema extraction using Clang's semantic AST.
//!
//! Annotations select export roots. Referenced records are discovered transitively;
//! native STL layouts never become wire layouts. Parsing uses the caller's compiler
//! flags, so aliases, conditional compilation, and concrete templates are resolved
//! by the C++ compiler before entering the common schema.

#![forbid(unsafe_code)]

mod adapters;
mod annotations;
mod compiler;
mod documentation;
mod extract;

use std::{collections::BTreeMap, error::Error, fmt, path::Path, sync::Mutex};

use clang::{Clang, Index, TranslationUnit, diagnostic::Severity};
use intrlay_schema::{Layouts, Schema};

/// Compiler settings used when parsing a schema translation unit.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Additional Clang arguments, including include directories and definitions.
    pub arguments: Vec<String>,
    /// Directory containing `compile_commands.json`, when using an existing build.
    pub compilation_database: Option<std::path::PathBuf>,
}

/// A diagnostic from the compiler or an unsupported source declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendError(pub String);

impl fmt::Display for FrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for FrontendError {}

// The safe clang wrapper permits one loaded Clang handle per process.
static CLANG: Mutex<()> = Mutex::new(());

/// Extracts annotated exports and all their reachable definitions.
///
/// `INTRLAY(...)` is provided by `intrlay/annotations.hpp`. It does not change
/// the native C++ type and becomes empty on compilers that do not parse schemas.
///
/// # Errors
/// Reports compiler diagnostics, missing libclang, unsupported fields, or an
/// invalid common schema. No partial schema is returned after a compiler error.
pub fn extract(path: impl AsRef<Path>, options: &Options) -> Result<Schema, FrontendError> {
    // A compilation database can select a different compiler working directory.
    let path =
        std::path::absolute(path.as_ref()).map_err(|error| FrontendError(error.to_string()))?;
    let _guard = CLANG
        .lock()
        .map_err(|error| FrontendError(error.to_string()))?;
    let clang = Clang::new().map_err(FrontendError)?;
    let index = Index::new(&clang, false, false);
    let arguments = compiler::arguments(&path, options)?;
    let mut unit = index
        .parser(&path)
        .arguments(&arguments)
        .parse()
        .map_err(|error| FrontendError(format!("cannot parse {}: {error:?}", path.display())))?;
    check_diagnostics(&unit)?;

    // Reparse only when adapters exist, and only for concrete types reachable
    // from exports. Each pass batches newly discovered schema_type requests.
    let enabled = adapters::has_specializations(unit.get_entity());
    let original = if enabled {
        std::fs::read_to_string(&path).map_err(|error| FrontendError(error.to_string()))?
    } else {
        String::new()
    };
    let mut queries = Vec::new();
    let schema = loop {
        let resolved = if queries.is_empty() {
            BTreeMap::new()
        } else {
            adapters::resolved(unit.get_entity(), &queries)?
        };
        let mut mappings = adapters::Adapters::new(enabled, resolved);
        let result = extract::Extractor::new(&mut mappings).extract(unit.get_entity());
        if mappings.pending.is_empty() {
            break result?;
        }
        for (_, query) in mappings.pending {
            // An expression that resolves a namesake must never silently select
            // its adapter, nor keep appending the same unresolved request.
            if queries.contains(&query) {
                return Err(FrontendError(format!(
                    "adapter probe does not name the exported source type: {}",
                    query.expression
                )));
            }
            queries.push(query);
        }
        unit = adapters::parse(&index, &path, &arguments, &original, &queries)?;
    };
    Layouts::new(&schema)
        .validate()
        .map_err(|error| FrontendError(error.to_string()))?;
    Ok(schema)
}

fn check_diagnostics(unit: &TranslationUnit<'_>) -> Result<(), FrontendError> {
    let errors: Vec<_> = unit
        .get_diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.get_severity() >= Severity::Error)
        .filter(|diagnostic| !compiler::is_parser_option_diagnostic(diagnostic))
        .map(|diagnostic| diagnostic.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(FrontendError(errors.join("\n")));
    }
    Ok(())
}

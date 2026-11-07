//! C++20 schema extraction using Clang's semantic AST.
//!
//! Annotations select export roots. Referenced records are discovered transitively;
//! native STL layouts never become wire layouts. Parsing uses the caller's compiler
//! flags, so aliases, conditional compilation, and concrete templates are resolved
//! by the C++ compiler before entering the common schema.

#![forbid(unsafe_code)]

mod annotations;
mod documentation;
mod extract;

use std::{error::Error, fmt, path::Path, sync::Mutex};

use clang::{Clang, CompilationDatabase, Index, diagnostic::Severity};
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
    let arguments = compiler_arguments(&path, options)?;
    let unit = index
        .parser(&path)
        .arguments(&arguments)
        .parse()
        .map_err(|error| FrontendError(format!("cannot parse {}: {error:?}", path.display())))?;
    let errors: Vec<_> = unit
        .get_diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.get_severity() >= Severity::Error)
        .map(|diagnostic| diagnostic.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(FrontendError(errors.join("\n")));
    }
    let schema = extract::Extractor::default().extract(unit.get_entity())?;
    Layouts::new(&schema)
        .validate()
        .map_err(|error| FrontendError(error.to_string()))?;
    Ok(schema)
}

fn compiler_arguments(path: &Path, options: &Options) -> Result<Vec<String>, FrontendError> {
    let mut result = vec!["-x".into(), "c++".into(), "-std=c++20".into()];
    if let Some(directory) = &options.compilation_database {
        let database = CompilationDatabase::from_directory(directory)
            .map_err(|error| FrontendError(format!("compilation database: {error:?}")))?;
        let commands = database.get_compile_commands(path).map_err(|error| {
            FrontendError(format!("compile command for {}: {error:?}", path.display()))
        })?;
        let commands = commands.get_commands();
        let command = commands
            .first()
            .ok_or_else(|| FrontendError("empty compile command".into()))?;
        let working_directory = command.get_directory();
        let source = resolved_source(&command.get_filename(), &working_directory);
        let input = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        result.push(format!(
            "-working-directory={}",
            working_directory.display()
        ));
        let mut arguments = command.get_arguments().into_iter().skip(1);
        while let Some(argument) = arguments.next() {
            if argument == "-o" {
                arguments.next();
            } else if argument != "-c" && argument != "/c" && !argument.starts_with("/Fo") {
                if !argument.starts_with('-') {
                    let candidate = resolved_source(Path::new(&argument), &working_directory);
                    if candidate == source || candidate == input {
                        continue;
                    }
                }
                result.push(argument);
            }
        }
    }
    result.extend(options.arguments.iter().cloned());
    Ok(result)
}

fn resolved_source(path: &Path, directory: &Path) -> std::path::PathBuf {
    let path = directory.join(path);
    path.canonicalize().unwrap_or(path)
}

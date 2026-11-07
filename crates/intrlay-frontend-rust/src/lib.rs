//! Compiler-resolved schema collection from a Rust library crate.
//!
//! A temporary Cargo package calls the library's `intrlay::export_types!`
//! entry point. Only that library and its dependencies are compiled. Collection
//! runs on the host; neither Cargo nor rustc is bundled with this frontend.

#![forbid(unsafe_code)]

mod collector;
mod manifest;

use std::{error::Error, fmt, fs, path::Path};

use cargo_metadata::{CrateType, Metadata, MetadataCommand, Package};
use intrlay_schema::{Layouts, Schema};

/// Package and feature selection for the schema library.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Workspace package name. Required when a virtual workspace is ambiguous.
    pub package: Option<String>,
    /// Features enabled on the selected library, in addition to its defaults.
    pub features: Vec<String>,
    /// Enables every feature declared by the selected library.
    pub all_features: bool,
    /// Disables the selected library's default features.
    pub no_default_features: bool,
    /// Prevents Cargo from accessing the network during metadata collection/build.
    pub offline: bool,
}

/// A Cargo, compiler, collector, or schema diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendError(pub String);

impl fmt::Display for FrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for FrontendError {}

/// Builds and runs a metadata collector for a crate's declared export roots.
///
/// The library must invoke `intrlay::export_types!` at its crate root. The
/// original source and lockfile are left intact. Collector build artifacts are
/// cached beneath Cargo's target directory, separately from application builds.
///
/// # Errors
/// Reports invalid package selection, missing library exports, unavailable Cargo
/// or Rust toolchains, build failures, and invalid schema metadata.
pub fn extract(path: impl AsRef<Path>, options: &Options) -> Result<Schema, FrontendError> {
    extract_inner(path.as_ref(), options).map_err(|error| FrontendError(error.to_string()))
}

fn extract_inner(path: &Path, options: &Options) -> Result<Schema, Box<dyn Error>> {
    let path = fs::canonicalize(path)?;
    let directory = path.parent().ok_or("manifest has no parent directory")?;
    let mut command = MetadataCommand::new();
    command
        .manifest_path(&path)
        .current_dir(directory)
        .no_deps();
    if options.offline {
        command.other_options(vec!["--offline".into()]);
    }
    let metadata = command.exec().map_err(|error| {
        FrontendError(format!(
            "cannot read Cargo metadata for {}: {error}",
            path.display()
        ))
    })?;
    let package = select_package(&metadata, &path, options.package.as_deref())?;
    if !package.targets.iter().any(|target| {
        target
            .crate_types
            .iter()
            .any(|kind| matches!(kind, CrateType::Lib | CrateType::RLib))
    }) {
        return Err(FrontendError(format!(
            "package {} needs a library target with crate-type lib or rlib for schema collection",
            package.name
        ))
        .into());
    }
    let temporary = tempfile::Builder::new().prefix("intrlay-rust-").tempdir()?;
    manifest::write(temporary.path(), &metadata, package, options)?;
    let schema = collector::run(temporary.path(), &metadata, package, options)?;
    Layouts::new(&schema).validate()?;
    Ok(schema)
}

fn select_package<'a>(
    metadata: &'a Metadata,
    manifest: &Path,
    name: Option<&str>,
) -> Result<&'a Package, Box<dyn Error>> {
    let members = metadata.workspace_packages();
    if let Some(name) = name {
        return members
            .into_iter()
            .find(|package| package.name.as_str() == name)
            .ok_or_else(|| FrontendError(format!("workspace has no package named {name}")).into());
    }
    for package in &members {
        if fs::canonicalize(&package.manifest_path)? == manifest {
            return Ok(package);
        }
    }
    let defaults = metadata.workspace_default_packages();
    if let [package] = defaults.as_slice() {
        return Ok(package);
    }
    let names = members
        .iter()
        .map(|package| package.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(FrontendError(format!(
        "workspace selects multiple packages; use --package to choose one schema library ({names})"
    ))
    .into())
}

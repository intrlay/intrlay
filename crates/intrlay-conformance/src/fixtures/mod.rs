//! Shared schemas and expected bytes for executable language consumers.
//!
//! Test execution builds artifacts in memory. Only the conformance command writes
//! them to disk, before any consumer is compiled.

#![allow(
    dead_code,
    reason = "fixture types and helpers are also used by Rust unit tests"
)]

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

mod annotations;
mod baseline;
mod compositions;
mod construction_extents;
mod cpp_collections;
mod cpp_metadata;
mod csharp_recursive_storage;
mod dart_writes;
mod documentation;
mod edge_cases;
mod emission;
mod hygiene;
mod initialization;
mod modules;
mod name_scopes;
mod names;
mod native_union_depth;
mod native_views;
mod python_private_names;
mod python_setters;
mod reference_loads;
mod regressions;
mod replacement;
mod rust_alias_names;
mod rust_primitive_names;

#[derive(Default)]
pub(crate) struct Fixtures {
    files: BTreeMap<PathBuf, Vec<u8>>,
}

impl Fixtures {
    fn insert(
        &mut self,
        path: impl AsRef<Path>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<(), Box<dyn Error>> {
        let path = path.as_ref();
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(format!("fixture path must be relative: {}", path.display()).into());
        }
        if let Some(previous) = self.files.insert(path.to_owned(), bytes.as_ref().to_vec())
            && previous != bytes.as_ref()
        {
            return Err(format!("conflicting fixture {}", path.display()).into());
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, directory: &Path) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(directory)?;
        let manifest = directory.join("intrlay-fixtures.json");
        if manifest.exists() {
            let previous: Vec<PathBuf> = serde_json::from_slice(&fs::read(&manifest)?)?;
            for path in previous {
                if !self.files.contains_key(&path)
                    && path
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                {
                    let stale = directory.join(path);
                    if stale.is_file() {
                        fs::remove_file(stale)?;
                    }
                }
            }
        }
        for (path, bytes) in &self.files {
            let target = directory.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(target, bytes)?;
        }
        fs::write(
            manifest,
            serde_json::to_vec_pretty(&self.files.keys().collect::<Vec<_>>())?,
        )?;
        Ok(())
    }
}

pub(crate) fn collect() -> Result<Fixtures, Box<dyn Error>> {
    let mut fixtures = Fixtures::default();
    baseline::collect(&mut fixtures)?;
    annotations::collect(&mut fixtures)?;
    compositions::collect(&mut fixtures)?;
    construction_extents::collect(&mut fixtures)?;
    cpp_collections::collect(&mut fixtures)?;
    cpp_metadata::collect(&mut fixtures)?;
    csharp_recursive_storage::collect(&mut fixtures)?;
    dart_writes::collect(&mut fixtures)?;
    documentation::collect(&mut fixtures)?;
    edge_cases::collect(&mut fixtures)?;
    emission::collect(&mut fixtures)?;
    hygiene::collect(&mut fixtures)?;
    initialization::collect(&mut fixtures)?;
    modules::collect(&mut fixtures)?;
    name_scopes::collect(&mut fixtures)?;
    names::collect(&mut fixtures)?;
    native_union_depth::collect(&mut fixtures)?;
    native_views::collect(&mut fixtures)?;
    python_private_names::collect(&mut fixtures)?;
    python_setters::collect(&mut fixtures)?;
    regressions::collect(&mut fixtures)?;
    replacement::collect(&mut fixtures)?;
    reference_loads::collect(&mut fixtures)?;
    rust_alias_names::collect(&mut fixtures)?;
    rust_primitive_names::collect(&mut fixtures)?;
    Ok(fixtures)
}

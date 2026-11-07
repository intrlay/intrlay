use std::{error::Error, fs, path::Path};

use cargo_metadata::{Metadata, Package};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::Options;

pub fn write(
    directory: &Path,
    metadata: &Metadata,
    package: &Package,
    options: &Options,
) -> Result<(), Box<dyn Error>> {
    let root = metadata.workspace_root.as_std_path();
    let source: DocumentMut = fs::read_to_string(root.join("Cargo.toml"))?.parse()?;
    let mut manifest = DocumentMut::new();
    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("collector directory has no UTF-8 name")?;
    manifest["package"]["name"] = value(name);
    manifest["package"]["version"] = value("0.0.0");
    manifest["package"]["edition"] = value("2024");
    manifest["workspace"] = Item::Table(Table::new());
    // The dependency remains part of its own workspace. Only the temporary root
    // needs copies of resolution settings that Cargo ignores in dependencies.
    let resolver = source
        .get("workspace")
        .and_then(|item| item.get("resolver"))
        .or_else(|| source.get("package").and_then(|item| item.get("resolver")))
        .and_then(Item::as_str)
        .unwrap_or(
            match metadata
                .root_package()
                .map(|package| package.edition.as_str())
            {
                Some("2024") => "3",
                Some("2021") => "2",
                _ => "1",
            },
        );
    manifest["workspace"]["resolver"] = value(resolver);
    for key in ["patch", "replace", "profile"] {
        if let Some(item) = source.get(key) {
            manifest[key] = item.clone();
        }
    }
    rebase_overrides(&mut manifest, root);

    let dependency = &mut manifest["dependencies"]["intrlay_input"];
    dependency["package"] = value(package.name.as_str());
    dependency["path"] = value(
        package
            .manifest_path
            .parent()
            .ok_or("package has no directory")?
            .as_str(),
    );
    dependency["default-features"] = value(!options.no_default_features);
    let mut features = options.features.clone();
    if options.all_features {
        features.extend(package.features.keys().cloned());
    }
    dependency["features"] = value(features.iter().map(String::as_str).collect::<Array>());
    fs::write(directory.join("Cargo.toml"), manifest.to_string())?;
    let lockfile = root.join("Cargo.lock");
    if lockfile.exists() {
        fs::copy(lockfile, directory.join("Cargo.lock"))?;
    }
    fs::create_dir(directory.join("src"))?;
    fs::write(
        directory.join("src/main.rs"),
        include_str!("collector.rs.txt"),
    )?;
    Ok(())
}

fn rebase_overrides(manifest: &mut DocumentMut, root: &Path) {
    if let Some(patches) = manifest.get_mut("patch").and_then(Item::as_table_like_mut) {
        for (_, entries) in patches.iter_mut() {
            rebase_entries(entries, root);
        }
    }
    if let Some(entries) = manifest.get_mut("replace") {
        rebase_entries(entries, root);
    }
}

fn rebase_entries(entries: &mut Item, root: &Path) {
    if let Some(entries) = entries.as_table_like_mut() {
        for (_, dependency) in entries.iter_mut() {
            if let Some(dependency) = dependency.as_table_like_mut()
                && let Some(path) = dependency.get("path").and_then(Item::as_str)
                && Path::new(path).is_relative()
            {
                let path = root.join(path);
                dependency.insert("path", value(path.to_string_lossy().as_ref()));
            }
        }
    }
}

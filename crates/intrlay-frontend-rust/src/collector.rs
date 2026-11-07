use std::{error::Error, fs, path::Path, process::Command};

use cargo_metadata::{Metadata, Package};
use intrlay_schema::Schema;

use crate::{FrontendError, Options};

pub fn run(
    directory: &Path,
    metadata: &Metadata,
    package: &Package,
    options: &Options,
) -> Result<Schema, Box<dyn Error>> {
    let schema_path = directory.join("schema.json");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(
            package
                .manifest_path
                .parent()
                .ok_or("package has no directory")?,
        )
        .args(["run", "--quiet", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(metadata.target_directory.join("intrlay/rust"))
        .args(["--target", "host-tuple"]);
    if options.offline {
        command.arg("--offline");
    }
    let output = command.arg("--").arg(&schema_path).output()?;
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let hint = if diagnostics.contains("cannot find function `__intrlay_export_schema`") {
            "\nDeclare concrete export roots with intrlay::export_types! at the library crate root."
        } else {
            ""
        };
        return Err(FrontendError(format!(
            "schema collection for {} failed ({}):\n{diagnostics}{hint}",
            package.name, output.status
        ))
        .into());
    }
    if !diagnostics.is_empty() {
        eprint!("{diagnostics}");
    }
    Ok(Schema::from_json(&fs::read_to_string(schema_path)?)?)
}

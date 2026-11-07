//! Build and verify distribution artifacts without uploading them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use clap::Args;
use serde::Deserialize;
use serde_json::Value;

use crate::process::{CommandExt, Workspace, sources};
use crate::release::{self, SourcePackage};

#[derive(Args)]
pub(crate) struct Options {
    /// Empty directory for the verified distribution files.
    #[arg(long, default_value = "target/artifacts")]
    output: PathBuf,
    /// GitHub owner/repository expected by npm trusted publishing.
    #[arg(long)]
    repository: Option<String>,
}

fn check_repository(workspace: &Workspace, repository: &str) -> Result<()> {
    let package: Value = serde_json::from_str(&fs::read_to_string(
        workspace.path("packages/typescript/package.json"),
    )?)?;
    let actual = package["repository"]
        .as_str()
        .or_else(|| package["repository"]["url"].as_str())
        .unwrap_or("");
    let actual = actual
        .strip_prefix("git+")
        .unwrap_or(actual)
        .trim_end_matches('/');
    let actual = actual.strip_suffix(".git").unwrap_or(actual);
    let expected = format!("https://github.com/{repository}");
    ensure!(
        actual == expected,
        "packages/typescript/package.json repository.url must be {expected}"
    );
    Ok(())
}

fn copy_workspace_crates(workspace: &Workspace, output: &Path) -> Result<()> {
    #[derive(Deserialize)]
    struct Package {
        name: String,
        version: String,
        publish: Option<Vec<String>>,
    }
    #[derive(Deserialize)]
    struct Metadata {
        packages: Vec<Package>,
    }
    let metadata: Metadata = serde_json::from_str(
        &workspace
            .command("cargo")?
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .read()?,
    )?;
    for package in metadata.packages {
        if package
            .publish
            .is_some_and(|registries| registries.is_empty())
        {
            continue;
        }
        // Ignore archives left by earlier versions or renamed crates.
        let name = format!("{}-{}.crate", package.name, package.version);
        fs::copy(
            workspace.path("target/package").join(&name),
            output.join(name),
        )?;
    }
    Ok(())
}

pub(crate) fn run(workspace: &Workspace, options: &Options) -> Result<()> {
    let output = workspace.path(&options.output);
    fs::create_dir_all(&output)?;
    ensure!(
        fs::read_dir(&output)?.next().is_none(),
        "choose an empty output directory: {}",
        output.display()
    );
    let version = release::workspace_version(workspace)?;
    release::check_versions(workspace, &version)?;
    if let Some(repository) = &options.repository {
        check_repository(workspace, repository)?;
    }
    release::package_sources(workspace)?;
    release::require_clean_worktree(workspace)?;

    // Development-only members must never enter a published package set.
    workspace
        .command("cargo")?
        .args([
            "package",
            "--workspace",
            "--exclude",
            "intrlay-conformance",
            "--exclude",
            "xtask",
            "--locked",
            // package_sources creates ignored license files intentionally.
            "--allow-dirty",
        ])
        .run()?;
    copy_workspace_crates(workspace, &output)?;
    workspace
        .command("npm")?
        .arg("ci")
        .current_dir(workspace.path("packages/typescript"))
        .run()?;
    workspace
        .command("npm")?
        .args(["pack", "--pack-destination"])
        .arg(&output)
        .current_dir(workspace.path("packages/typescript"))
        .run()?;
    workspace
        .python()?
        .args(["-m", "build", "packages/python", "--outdir"])
        .arg(&output)
        .run()?;
    workspace
        .python()?
        .args(["-m", "twine", "check"])
        .args(sources(&output, &["*.whl"])?)
        .arg(output.join(format!(
            "intrlay-{}.tar.gz",
            release::python_version(&version)?
        )))
        .run()?;
    workspace
        .command("dotnet")?
        .args([
            "pack",
            "packages/csharp/Intrlay.csproj",
            "-c",
            "Release",
            "-o",
        ])
        .arg(&output)
        .run()?;
    for arguments in [
        &["pub", "get"][..],
        &["analyze"],
        &["pub", "publish", "--dry-run"],
    ] {
        workspace
            .command("dart")?
            .args(arguments)
            .current_dir(workspace.path("packages/dart"))
            .run()?;
    }
    release::archive(workspace, &output, None, SourcePackage::Cpp)?;
    release::archive(workspace, &output, None, SourcePackage::Dart)
}

//! Release preparation and explicit GitHub publication operations.

mod archive;
mod metadata;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::process::{CommandExt, Workspace};
pub(crate) use archive::{SourcePackage, archive, checksums};
pub(crate) use metadata::{check_versions, python_version, workspace_version};

const LICENSE_FILES: [&str; 3] = ["LICENSE", "LICENSE-APACHE", "LICENSE-MIT"];

#[derive(Subcommand)]
pub(crate) enum Task {
    /// Copy the latest numbered changelog section and verify package versions.
    Prepare {
        #[arg(long, default_value = "target/release-metadata")]
        output: PathBuf,
        #[arg(long)]
        tag: Option<String>,
    },
    /// Copy shared license and changelog files into publishable packages.
    PackageSources,
    /// Archive a native generator or a C++/Dart source package.
    Archive {
        #[arg(long, default_value = "target/artifacts")]
        output: PathBuf,
        #[arg(long)]
        platform: Option<String>,
        #[arg(long, value_enum, default_value = "cpp")]
        package: SourcePackage,
    },
    /// Write SHA256SUMS for the artifact files in a directory.
    Checksums { directory: PathBuf },
    /// Create or resume a draft GitHub Release and upload verified artifacts.
    Draft {
        #[arg(long, default_value = "target/release-metadata")]
        metadata: PathBuf,
        #[arg(long, default_value = "target/artifacts")]
        artifacts: PathBuf,
    },
}

pub(crate) fn run(workspace: &Workspace, task: Task) -> Result<()> {
    match task {
        Task::Prepare { output, tag } => {
            let github_output = std::env::var_os("GITHUB_OUTPUT").map(PathBuf::from);
            prepare(
                workspace,
                &workspace.path(output),
                tag.as_deref(),
                github_output.as_deref(),
            )
        }
        Task::PackageSources => package_sources(workspace),
        Task::Archive {
            output,
            platform,
            package,
        } => archive(
            workspace,
            &workspace.path(output),
            platform.as_deref(),
            package,
        ),
        Task::Checksums { directory } => checksums(&workspace.path(directory)),
        Task::Draft {
            metadata,
            artifacts,
        } => draft(
            workspace,
            &workspace.path(metadata),
            &workspace.path(artifacts),
        ),
    }
}

#[derive(Serialize, Deserialize)]
struct ReleaseMetadata {
    version: String,
    tag: String,
    prerelease: String,
}

fn prepare(
    workspace: &Workspace,
    output: &Path,
    tag: Option<&str>,
    github_output: Option<&Path>,
) -> Result<()> {
    let text = fs::read_to_string(workspace.path("CHANGELOG.md"))?;
    let (version, notes) = metadata::latest_release(&text)?;
    check_versions(workspace, version)?;
    let expected_tag = format!("v{version}");
    if let Some(tag) = tag {
        ensure!(
            tag == expected_tag,
            "tag {tag} does not match latest changelog entry {expected_tag}"
        );
    }
    let metadata = ReleaseMetadata {
        version: version.to_owned(),
        tag: expected_tag,
        prerelease: version.contains('-').to_string(),
    };
    fs::create_dir_all(output)?;
    fs::write(output.join("release-notes.md"), notes)?;
    fs::write(
        output.join("release.json"),
        serde_json::to_string_pretty(&metadata)? + "\n",
    )?;
    if let Some(destination) = github_output {
        let mut output = OpenOptions::new()
            .create(true)
            .append(true)
            .open(destination)?;
        writeln!(
            output,
            "version={}\ntag={}\nprerelease={}",
            metadata.version, metadata.tag, metadata.prerelease
        )?;
    }
    Ok(())
}

pub(crate) fn package_sources(workspace: &Workspace) -> Result<()> {
    for name in LICENSE_FILES {
        ensure!(
            workspace.path(name).is_file(),
            "missing release license text: {name}"
        );
    }
    for parent in ["packages", "crates"] {
        for directory in fs::read_dir(workspace.path(parent))? {
            let directory = directory?;
            if directory.file_type()?.is_dir() {
                for name in LICENSE_FILES {
                    fs::copy(workspace.path(name), directory.path().join(name))?;
                }
                if parent == "packages" {
                    fs::copy(
                        workspace.path("CHANGELOG.md"),
                        directory.path().join("CHANGELOG.md"),
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Generated license copies are ignored by Git but included in Cargo packages.
/// Check source changes before allowing Cargo to include those generated files.
pub(crate) fn require_clean_worktree(workspace: &Workspace) -> Result<()> {
    let changes = workspace
        .command("git")?
        .args(["status", "--porcelain", "--untracked-files=all"])
        .read()?;
    ensure!(
        changes.is_empty(),
        "commit or remove source changes before packaging a release:\n{changes}"
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum DraftAction {
    Create,
    Edit,
}

fn draft_action(success: bool, stdout: &[u8], stderr: &str) -> Result<DraftAction> {
    if success {
        #[derive(Deserialize)]
        struct ExistingRelease {
            draft: bool,
        }
        let existing: ExistingRelease = serde_json::from_slice(stdout)?;
        ensure!(
            existing.draft,
            "release is already public; do not replace released artifacts"
        );
        Ok(DraftAction::Edit)
    } else if stderr.contains("HTTP 404") {
        Ok(DraftAction::Create)
    } else {
        bail!("cannot read existing GitHub Release: {stderr}")
    }
}

fn draft(workspace: &Workspace, metadata: &Path, artifacts: &Path) -> Result<()> {
    let metadata_file: ReleaseMetadata =
        serde_json::from_str(&fs::read_to_string(metadata.join("release.json"))?)?;
    let tag = &metadata_file.tag;
    let repository = std::env::var("GITHUB_REPOSITORY").context("GITHUB_REPOSITORY must be set")?;
    let query = workspace
        .command("gh")?
        .args(["api", &format!("repos/{repository}/releases/tags/{tag}")])
        .capture()?;
    let action = draft_action(
        query.status.success(),
        &query.stdout,
        &String::from_utf8_lossy(&query.stderr),
    )?;
    let mut command = workspace.command("gh")?;
    command.arg("release");
    match action {
        DraftAction::Edit => {
            command.args(["edit", tag]);
        }
        DraftAction::Create => {
            command.args(["create", tag, "--verify-tag", "--draft"]);
        }
    }
    command
        .args(["--repo", &repository, "--title", tag, "--notes-file"])
        .arg(metadata.join("release-notes.md"))
        .arg(format!("--prerelease={}", metadata_file.prerelease))
        .run()?;
    let mut files = Vec::new();
    for entry in fs::read_dir(artifacts)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    workspace
        .command("gh")?
        .args(["release", "upload", tag, "--repo", &repository, "--clobber"])
        .args(files)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaging_allows_generated_licenses_but_rejects_source_changes() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let workspace = Workspace {
            root: temporary.path().to_path_buf(),
        };
        fs::create_dir_all(workspace.path("packages/runtime"))?;
        fs::write(workspace.path(".gitignore"), "/packages/*/LICENSE*\n")?;
        fs::write(workspace.path("packages/runtime/source"), "original\n")?;
        workspace.command("git")?.args(["init", "--quiet"]).run()?;
        workspace.command("git")?.args(["add", "."]).run()?;
        workspace
            .command("git")?
            .args([
                "-c",
                "user.name=Intrlay tests",
                "-c",
                "user.email=tests@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ])
            .run()?;

        fs::write(workspace.path("packages/runtime/LICENSE"), "generated\n")?;
        require_clean_worktree(&workspace)?;
        fs::write(workspace.path("packages/runtime/source"), "modified\n")?;
        assert!(require_clean_worktree(&workspace).is_err());
        fs::write(workspace.path("packages/runtime/source"), "original\n")?;
        fs::write(workspace.path("packages/runtime/new-source"), "new\n")?;
        assert!(require_clean_worktree(&workspace).is_err());
        Ok(())
    }

    #[test]
    fn prepare_copies_notes_and_appends_workflow_outputs_after_validating_the_tag() -> Result<()> {
        let repository = Workspace::current()?;
        let temporary = tempfile::tempdir()?;
        let workspace = Workspace {
            root: temporary.path().to_path_buf(),
        };
        for path in [
            "Cargo.toml",
            "packages/typescript/package.json",
            "packages/python/pyproject.toml",
            "packages/dart/pubspec.yaml",
            "packages/csharp/Intrlay.csproj",
            "packages/cpp/CMakeLists.txt",
        ] {
            let destination = workspace.path(path);
            fs::create_dir_all(destination.parent().context("missing fixture parent")?)?;
            fs::copy(repository.path(path), destination)?;
        }
        let version = workspace_version(&workspace)?;
        let notes = "\n### Added\n\n- A `value`.\n  Continued text.\n\n";
        fs::write(
            workspace.path("CHANGELOG.md"),
            format!("## [Unreleased]\n\n## [{version}]\n{notes}"),
        )?;
        let output = workspace.path("metadata");
        let workflow_output = workspace.path("outputs.txt");
        fs::write(&workflow_output, "existing=value\n")?;
        assert!(
            prepare(
                &workspace,
                &output,
                Some("v999.0.0"),
                Some(&workflow_output)
            )
            .is_err()
        );
        assert!(!output.exists());
        assert_eq!(fs::read_to_string(&workflow_output)?, "existing=value\n");
        prepare(
            &workspace,
            &output,
            Some(&format!("v{version}")),
            Some(&workflow_output),
        )?;
        assert_eq!(fs::read_to_string(output.join("release-notes.md"))?, notes);
        let metadata: ReleaseMetadata =
            serde_json::from_str(&fs::read_to_string(output.join("release.json"))?)?;
        assert_eq!(metadata.version, version);
        assert_eq!(
            fs::read_to_string(workflow_output)?,
            format!(
                "existing=value\nversion={version}\ntag=v{version}\nprerelease={}\n",
                version.contains('-')
            )
        );
        Ok(())
    }

    #[test]
    fn draft_retries_cannot_replace_public_releases_or_hide_api_errors() -> Result<()> {
        assert_eq!(
            draft_action(true, br#"{"draft":true}"#, "")?,
            DraftAction::Edit
        );
        assert_eq!(
            draft_action(false, b"", "gh: Not Found (HTTP 404)")?,
            DraftAction::Create
        );
        assert!(draft_action(true, br#"{"draft":false}"#, "").is_err());
        assert!(draft_action(false, b"", "HTTP 503").is_err());
        Ok(())
    }
}

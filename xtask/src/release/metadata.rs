//! Shared version policy and verbatim changelog section extraction.

use std::fs;

use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde_json::Value;

use crate::process::Workspace;

pub(crate) fn workspace_version(workspace: &Workspace) -> Result<String> {
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(workspace.path("Cargo.toml"))?)?;
    Ok(manifest["workspace"]["package"]["version"]
        .as_str()
        .context("workspace.package.version must be a string")?
        .to_owned())
}

pub(crate) fn latest_release(text: &str) -> Result<(&str, &str)> {
    let pattern = Regex::new(r"(?m)^## \[([^\]]+)\][^\n]*\n")?;
    let headings = pattern.captures_iter(text).collect::<Vec<_>>();
    for (index, heading) in headings.iter().enumerate() {
        let version = heading.get(1).context("missing version capture")?.as_str();
        if version == "Unreleased" {
            continue;
        }
        semver::Version::parse(version)
            .with_context(|| format!("invalid changelog version: {version}"))?;
        let start = heading.get(0).context("missing heading capture")?.end();
        let end = headings
            .get(index + 1)
            .and_then(|heading| heading.get(0))
            .map_or(text.len(), |heading| heading.start());
        let body = &text[start..end];
        ensure!(
            !body.trim().is_empty(),
            "empty changelog section: {version}"
        );
        return Ok((version, body));
    }
    bail!("promote CHANGELOG.md [Unreleased] to a numbered version before releasing")
}

pub(crate) fn python_version(version: &str) -> Result<String> {
    if !version.contains('-') {
        return Ok(version.to_owned());
    }
    let pattern = Regex::new(r"^(\d+\.\d+\.\d+)-(alpha|beta|rc)\.(\d+)$")?;
    let parts = pattern
        .captures(version)
        .context("prereleases must use alpha.N, beta.N, or rc.N across registries")?;
    let label = match &parts[2] {
        "alpha" => "a",
        "beta" => "b",
        _ => "rc",
    };
    Ok(format!("{}{label}{}", &parts[1], &parts[3]))
}

pub(crate) fn check_versions(workspace: &Workspace, version: &str) -> Result<()> {
    let cargo: toml::Value = toml::from_str(&fs::read_to_string(workspace.path("Cargo.toml"))?)?;
    let npm: Value = serde_json::from_str(&fs::read_to_string(
        workspace.path("packages/typescript/package.json"),
    )?)?;
    let python: toml::Value = toml::from_str(&fs::read_to_string(
        workspace.path("packages/python/pyproject.toml"),
    )?)?;
    let pep440 = python_version(version)?;
    let base = version.split('-').next().context("missing version")?;

    for (path, actual, expected) in [
        (
            "Cargo.toml",
            cargo["workspace"]["package"]["version"].as_str(),
            version,
        ),
        (
            "packages/typescript/package.json",
            npm["version"].as_str(),
            version,
        ),
        (
            "packages/python/pyproject.toml",
            python["project"]["version"].as_str(),
            &pep440,
        ),
    ] {
        ensure!(
            actual == Some(expected),
            "{path}: expected {expected}, found {actual:?}"
        );
    }
    for (path, pattern, expected) in [
        (
            "packages/dart/pubspec.yaml",
            r"(?m)^version:\s*(\S+)",
            version,
        ),
        (
            "packages/csharp/Intrlay.csproj",
            r"<Version>([^<]+)</Version>",
            version,
        ),
        (
            "packages/cpp/CMakeLists.txt",
            r"project\(intrlay VERSION (\S+)",
            base,
        ),
    ] {
        let source = fs::read_to_string(workspace.path(path))?;
        let pattern = Regex::new(pattern)?;
        let actual = pattern
            .captures(&source)
            .with_context(|| format!("cannot find version in {path}"))?;
        ensure!(
            &actual[1] == expected,
            "{path}: expected {expected}, found {}",
            &actual[1]
        );
    }
    let dependencies = cargo["workspace"]["dependencies"]
        .as_table()
        .context("missing workspace dependencies")?;
    for (name, dependency) in dependencies {
        if dependency.get("path").is_some() {
            ensure!(
                dependency.get("version").and_then(toml::Value::as_str) == Some(version),
                "workspace dependency {name} must use version {version}"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_preserves_the_latest_numbered_body() -> Result<()> {
        let body = "\n### Added\n\n- A `value`.\n  Continued text.\n\n```text\nexample\n```\n\n";
        let text = format!(
            "# Changelog\n\n## [Unreleased]\n\n- Future.\n\n## [1.2.3] - 2026-11-04\n{body}## [1.2.2] - 2026-10-01\n\n- Previous.\n"
        );
        assert_eq!(latest_release(&text)?, ("1.2.3", body));
        assert_eq!(
            latest_release("## [0.2.0-beta.1]\n\n- Preview.\n")?.0,
            "0.2.0-beta.1"
        );
        for invalid in [
            "# Changelog\n\n## [Unreleased]\n\n- Work.\n",
            "## [invalid]\n\n- Work.\n",
            "## [1.2.3]\n\n",
        ] {
            assert!(latest_release(invalid).is_err());
        }
        Ok(())
    }

    #[test]
    fn versions_agree_across_packages_and_prerelease_syntaxes() -> Result<()> {
        let workspace = Workspace::current()?;
        check_versions(&workspace, &workspace_version(&workspace)?)?;
        assert!(check_versions(&workspace, "999.0.0").is_err());
        for (source, expected) in [
            ("1.2.3", "1.2.3"),
            ("1.2.3-alpha.1", "1.2.3a1"),
            ("1.2.3-beta.4", "1.2.3b4"),
            ("1.2.3-rc.2", "1.2.3rc2"),
        ] {
            assert_eq!(python_version(source)?, expected);
        }
        assert!(python_version("1.2.3-preview.1").is_err());
        Ok(())
    }
}

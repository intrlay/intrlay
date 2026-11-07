//! Registry publication used only by explicit tag-triggered release jobs.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use serde::Deserialize;

use crate::process::{CommandExt, Workspace};
use crate::release;

#[derive(Args)]
pub(crate) struct Options {
    #[arg(value_enum)]
    registry: Registry,
    #[arg(long, default_value = "target/artifacts")]
    artifacts: PathBuf,
}

#[derive(Clone, Copy, ValueEnum)]
enum Registry {
    Crates,
    Npm,
    Dart,
}

struct RegistryClient {
    agent: ureq::Agent,
}

impl RegistryClient {
    fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
        }
    }

    fn exists(&self, url: &str) -> Result<bool> {
        let response = self
            .agent
            .get(url)
            .header("User-Agent", "intrlay-release")
            .header("Accept", "application/json")
            .call()
            .with_context(|| format!("querying {url}"))?;
        match response.status().as_u16() {
            200 => Ok(true),
            404 => Ok(false),
            status => bail!("registry returned HTTP {status} for {url}"),
        }
    }
}

/// Encode one registry path segment, including npm's scoped package separator.
fn segment(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            result.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            result.push('%');
            result.push(char::from(HEX[usize::from(byte >> 4)]));
            result.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    result
}

pub(crate) fn run(workspace: &Workspace, options: &Options) -> Result<()> {
    let registry = RegistryClient::new();
    match options.registry {
        Registry::Crates => crates(workspace, &registry),
        Registry::Npm => npm(workspace, &registry, &workspace.path(&options.artifacts)),
        Registry::Dart => dart(workspace, &registry, &workspace.path(&options.artifacts)),
    }
}

fn crates(workspace: &Workspace, registry: &RegistryClient) -> Result<()> {
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

    release::package_sources(workspace)?;
    let metadata: Metadata = serde_json::from_str(
        &workspace
            .command("cargo")?
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .read()?,
    )?;
    let mut excluded = Vec::new();
    let mut pending = false;
    for package in metadata.packages {
        if package
            .publish
            .is_some_and(|registries| registries.is_empty())
        {
            excluded.push(package.name);
        } else if registry.exists(&format!(
            "https://crates.io/api/v1/crates/{}/{}",
            segment(&package.name),
            segment(&package.version)
        ))? {
            println!("{}@{} is already published", package.name, package.version);
            excluded.push(package.name);
        } else {
            pending = true;
        }
    }
    if pending {
        release::require_clean_worktree(workspace)?;
        let mut command = workspace.command("cargo")?;
        // Source changes were checked above; only generated copies are ignored.
        command.args(["publish", "--workspace", "--locked", "--allow-dirty"]);
        for name in excluded {
            command.args(["--exclude", &name]);
        }
        command.run()?;
    }
    Ok(())
}

fn npm(workspace: &Workspace, registry: &RegistryClient, artifacts: &Path) -> Result<()> {
    #[derive(Deserialize)]
    struct Package {
        name: String,
        version: String,
    }
    let package: Package = serde_json::from_str(&fs::read_to_string(
        workspace.path("packages/typescript/package.json"),
    )?)?;
    let Package { name, version } = package;
    if registry.exists(&format!(
        "https://registry.npmjs.org/{}/{}",
        segment(&name),
        segment(&version)
    ))? {
        println!("{name}@{version} is already published");
        return Ok(());
    }
    let archive = format!(
        "{}-{version}.tgz",
        name.trim_start_matches('@').replace('/', "-")
    );
    workspace
        .command("npm")?
        .arg("publish")
        .arg(artifacts.join(archive))
        .args([
            "--access",
            "public",
            "--provenance",
            "--tag",
            if version.contains('-') {
                "next"
            } else {
                "latest"
            },
        ])
        .run()
}

fn dart(workspace: &Workspace, registry: &RegistryClient, artifacts: &Path) -> Result<()> {
    let version = release::workspace_version(workspace)?;
    if registry.exists(&format!(
        "https://pub.dev/api/packages/intrlay/versions/{}",
        segment(&version)
    ))? {
        println!("intrlay@{version} is already published");
        return Ok(());
    }
    let prefix = format!("intrlay-dart-{version}");
    let temporary = release::unpack_source_package(&artifacts.join(format!("{prefix}.tar.gz")))?;
    let directory = temporary.path().join(prefix);
    workspace
        .command("dart")?
        .args(["pub", "get"])
        .current_dir(&directory)
        .run()?;
    workspace
        .command("dart")?
        .args(["pub", "publish", "--force"])
        .current_dir(&directory)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn registry_retries_distinguish_missing_versions_from_server_failures() -> Result<()> {
        for status in [200, 404, 503] {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let address = listener.local_addr()?;
            let server = std::thread::spawn(move || -> std::io::Result<()> {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte)?;
                    request.push(byte[0]);
                }
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
            });
            let result = RegistryClient::new().exists(&format!("http://{address}/package"));
            server
                .join()
                .map_err(|_| anyhow::anyhow!("test server panicked"))??;
            match status {
                200 => assert!(result?),
                404 => assert!(!result?),
                _ => assert!(result.is_err()),
            }
        }
        assert_eq!(segment("@scope/intrlay"), "%40scope%2Fintrlay");
        Ok(())
    }
}

//! Exercise the published installation layout before uploading release artifacts.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, ensure};

use crate::process::{CommandExt, Workspace};

pub(super) fn verify(workspace: &Workspace, platform: &str, artifacts: &Path) -> Result<()> {
    let version = super::workspace_version(workspace)?;
    let manifest = workspace.path("crates/intrlay-cli/Cargo.toml");
    let cargo: toml::Value = toml::from_str(&fs::read_to_string(&manifest)?)?;
    let metadata = &cargo["package"]["metadata"]["binstall"];
    let template = metadata
        .get("overrides")
        .and_then(|overrides| overrides.get(platform))
        .and_then(|entry| entry.get("pkg-url"))
        .or_else(|| metadata.get("pkg-url"))
        .and_then(toml::Value::as_str)
        .context("missing binstall pkg-url")?;
    let relative_url = template
        .strip_prefix("{ repo }")
        .context("binstall pkg-url must refer to the package repository")?;

    let suffix = if platform.contains("windows") {
        "zip"
    } else {
        "tar.gz"
    };
    let name = format!("intrlay-{version}-{platform}.{suffix}");
    let archive = artifacts.join(&name);
    ensure!(archive.is_file(), "missing archive: {}", archive.display());
    let route = format!("/releases/download/v{version}/{name}");
    let temporary = tempfile::tempdir()?;
    let installed = temporary.path().join("bin");

    // Preserve the real URL path, format, and bin-dir. Only replace the repository
    // host so the exact archive can be tested before its version is public.
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}{relative_url}");
    let stopped = AtomicBool::new(false);
    std::thread::scope(|scope| -> Result<()> {
        let server = scope.spawn(|| serve_archive(listener, &stopped, &route, &archive));
        let result = (|| -> Result<()> {
            workspace
                .command("cargo-binstall")?
                .arg("intrlay-cli")
                .arg("--manifest-path")
                .arg(&manifest)
                .args(["--version", &version, "--targets", platform])
                .args(["--pkg-url", &url, "--strategies", "crate-meta-data"])
                .args([
                    "--allow-insecure-http",
                    "--no-confirm",
                    "--no-discover-github-token",
                ])
                .arg("--install-path")
                .arg(&installed)
                .run()
        })();
        stopped.store(true, Ordering::Release);
        // Wake accept even when the installer failed before making a request.
        let wake = TcpStream::connect(address);
        let server_result = server
            .join()
            .map_err(|_| anyhow::anyhow!("binstall archive server panicked"))?;
        result?;
        wake?;
        server_result?;
        Ok(())
    })?;

    let executable = installed.join(if platform.contains("windows") {
        "intrlay.exe"
    } else {
        "intrlay"
    });
    let actual = workspace.command(&executable)?.arg("--version").read()?;
    ensure!(
        actual.trim() == format!("intrlay-cli {version}"),
        "installed executable reported unexpected version: {actual}"
    );
    workspace
        .command(&executable)?
        .args([
            "generate",
            "tests/cli/scalar.json",
            "--language",
            "typescript",
            "--output",
        ])
        .arg(temporary.path().join("smoke.ts"))
        .run()?;
    Ok(())
}

fn serve_archive(
    listener: TcpListener,
    stopped: &AtomicBool,
    route: &str,
    archive: &Path,
) -> std::io::Result<()> {
    let result = std::thread::scope(|requests| {
        for connection in listener.incoming() {
            let connection = connection?;
            if stopped.load(Ordering::Acquire) {
                break;
            }
            // Binstall can probe with GET without consuming the body, then open
            // its download while that connection is still alive.
            requests.spawn(move || {
                if let Err(error) = serve(connection, route, archive)
                    && !matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::ConnectionReset
                    )
                {
                    eprintln!("archive request failed: {error}");
                }
            });
        }
        Ok(())
    });
    // Close the listening socket on server failure so the installer cannot hang
    // waiting for a connection that will never be accepted.
    drop(listener);
    result
}

/// Serve only the expected release asset over a short-lived loopback connection.
fn serve(mut connection: TcpStream, route: &str, archive: &Path) -> std::io::Result<()> {
    connection.set_read_timeout(Some(Duration::from_secs(10)))?;
    connection.set_write_timeout(Some(Duration::from_secs(30)))?;
    let mut request = String::new();
    let mut reader = BufReader::new(&mut connection);
    reader.read_line(&mut request)?;
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        if line == "\r\n" {
            break;
        }
        line.clear();
    }
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    if !matches!(method, "HEAD" | "GET") || path != route {
        connection.write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )?;
        return Ok(());
    }
    let mut file = File::open(archive)?;
    write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        file.metadata()?.len()
    )?;
    if method == "GET" {
        std::io::copy(&mut file, &mut connection)?;
    }
    Ok(())
}

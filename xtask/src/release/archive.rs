//! Portable package archives and artifact checksums.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use zip::{ZipWriter, write::SimpleFileOptions};

use super::{LICENSE_FILES, workspace_version};
use crate::process::Workspace;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SourcePackage {
    Cpp,
    Dart,
}

impl SourcePackage {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Cpp => "cpp",
            Self::Dart => "dart",
        }
    }
}

/// Collect regular files without following directory symlinks.
fn files_under(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            files.extend(files_under(&entry.path())?);
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) fn archive(
    workspace: &Workspace,
    output: &Path,
    platform: Option<&str>,
    package: SourcePackage,
) -> Result<()> {
    let version = workspace_version(workspace)?;
    fs::create_dir_all(output)?;
    let (prefix, files) = if let Some(platform) = platform {
        ensure!(
            !platform.is_empty()
                && platform.bytes().all(|byte| byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || b"_-".contains(&byte)),
            "invalid platform identifier"
        );
        let executable = if platform.contains("windows") {
            "intrlay.exe"
        } else {
            "intrlay"
        };
        let binary = workspace.path(format!("target/{platform}/release/{executable}"));
        ensure!(binary.is_file(), "missing executable: {}", binary.display());
        let mut files = vec![
            (binary, PathBuf::from(format!("bin/{executable}"))),
            (
                workspace.path("docs/docs/reference/code-generation.mdx"),
                PathBuf::from("README.md"),
            ),
        ];
        files.extend(
            LICENSE_FILES
                .iter()
                .map(|name| (workspace.path(name), PathBuf::from(name))),
        );
        let include = workspace.path("packages/cpp/include");
        for file in files_under(&include)? {
            if file.extension().is_some_and(|extension| extension == "hpp") {
                let name = Path::new("include").join(file.strip_prefix(&include)?);
                files.push((file, name));
            }
        }
        (format!("intrlay-{version}-{platform}"), files)
    } else {
        let name = package.name();
        let directory = workspace.path(format!("packages/{name}"));
        let mut files = ["README.md", "CHANGELOG.md"]
            .into_iter()
            .chain(LICENSE_FILES)
            .map(|name| directory.join(name))
            .collect::<Vec<_>>();
        let (manifest, folders): (_, &[&str]) = match package {
            SourcePackage::Cpp => ("CMakeLists.txt", &["include", "cmake"]),
            SourcePackage::Dart => ("pubspec.yaml", &["lib"]),
        };
        files.push(directory.join(manifest));
        for folder in folders {
            files.extend(files_under(&directory.join(folder))?);
        }
        files.sort();
        let files = files
            .into_iter()
            .map(|file| {
                let name = file.strip_prefix(&directory)?.to_path_buf();
                Ok((file, name))
            })
            .collect::<Result<Vec<_>>>()?;
        (format!("intrlay-{name}-{version}"), files)
    };

    if platform.is_some_and(|platform| platform.contains("windows")) {
        let mut archive = ZipWriter::new(File::create(output.join(format!("{prefix}.zip")))?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (file, name) in files {
            let name = Path::new(&prefix)
                .join(name)
                .to_string_lossy()
                .replace('\\', "/");
            archive.start_file(name, options)?;
            std::io::copy(&mut File::open(file)?, &mut archive)?;
        }
        archive.finish()?;
    } else {
        let gzip = GzEncoder::new(
            File::create(output.join(format!("{prefix}.tar.gz")))?,
            Compression::default(),
        );
        let mut archive = tar::Builder::new(gzip);
        for (file, name) in files {
            archive.append_path_with_name(file, Path::new(&prefix).join(name))?;
        }
        archive.into_inner()?.finish()?;
    }
    Ok(())
}

pub(crate) fn checksums(directory: &Path) -> Result<()> {
    let mut files = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(std::fs::DirEntry::file_name);
    let mut sums = String::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    for entry in files {
        if !entry.file_type()?.is_file() || entry.file_name() == "SHA256SUMS" {
            continue;
        }
        let mut source = BufReader::new(File::open(entry.path())?);
        let mut digest = Sha256::new();
        loop {
            let size = source.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            digest.update(&buffer[..size]);
        }
        let name = entry.file_name();
        let name = name.to_str().context("artifact filename is not UTF-8")?;
        writeln!(sums, "{:x}  {name}", digest.finalize())?;
    }
    File::create(directory.join("SHA256SUMS"))?.write_all(sums.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_are_stable_and_exclude_themselves() -> Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(directory.path().join("a.bin"), b"abc")?;
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  a.bin\n";
        for _ in 0..2 {
            checksums(directory.path())?;
            assert_eq!(
                fs::read_to_string(directory.path().join("SHA256SUMS"))?,
                expected
            );
        }
        Ok(())
    }

    #[test]
    fn source_and_binary_archives_preserve_members() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let workspace = Workspace {
            root: temporary.path().to_path_buf(),
        };
        fs::write(
            workspace.path("Cargo.toml"),
            "[workspace.package]\nversion = '1.2.3'\n",
        )?;
        for path in [
            "LICENSE",
            "LICENSE-APACHE",
            "LICENSE-MIT",
            "docs/docs/reference/code-generation.mdx",
            "packages/cpp/include/intrlay/view.hpp",
            "packages/cpp/cmake/intrlay.cmake",
            "packages/cpp/CMakeLists.txt",
            "packages/cpp/README.md",
            "packages/cpp/CHANGELOG.md",
            "packages/cpp/LICENSE",
            "packages/cpp/LICENSE-APACHE",
            "packages/cpp/LICENSE-MIT",
            "target/x86_64-pc-windows-msvc/release/intrlay.exe",
            "target/x86_64-unknown-linux-gnu/release/intrlay",
        ] {
            let file = workspace.path(path);
            fs::create_dir_all(file.parent().context("missing fixture parent")?)?;
            fs::write(file, path)?;
        }
        let output = workspace.path("artifacts");
        archive(&workspace, &output, None, SourcePackage::Cpp)?;
        for platform in ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"] {
            archive(&workspace, &output, Some(platform), SourcePackage::Cpp)?;
        }
        let mut zip = zip::ZipArchive::new(File::open(
            output.join("intrlay-1.2.3-x86_64-pc-windows-msvc.zip"),
        )?)?;
        let mut bytes = String::new();
        zip.by_name("intrlay-1.2.3-x86_64-pc-windows-msvc/include/intrlay/view.hpp")?
            .read_to_string(&mut bytes)?;
        assert_eq!(bytes, "packages/cpp/include/intrlay/view.hpp");
        for (file, expected) in [
            (
                "intrlay-cpp-1.2.3.tar.gz",
                "intrlay-cpp-1.2.3/cmake/intrlay.cmake",
            ),
            (
                "intrlay-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
                "intrlay-1.2.3-x86_64-unknown-linux-gnu/bin/intrlay",
            ),
        ] {
            let gzip = flate2::read::GzDecoder::new(File::open(output.join(file))?);
            let mut archive = tar::Archive::new(gzip);
            let mut names = Vec::new();
            for entry in archive.entries()? {
                names.push(entry?.path()?.into_owned());
            }
            assert!(names.contains(&PathBuf::from(expected)));
        }
        assert!(archive(&workspace, &output, Some("../outside"), SourcePackage::Cpp).is_err());
        Ok(())
    }
}

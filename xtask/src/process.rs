//! Shared workspace paths and subprocess execution, including Windows shims.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, ensure};

pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
}

impl Workspace {
    pub(crate) fn current() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("xtask must be a workspace member")?
            .to_path_buf();
        Ok(Self { root })
    }

    pub(crate) fn path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(path)
    }

    pub(crate) fn command(&self, program: impl AsRef<OsStr>) -> Result<Command> {
        // Resolve .cmd/.bat shims too: npm and Dart use them on Windows.
        let program = program.as_ref();
        let executable = which::which(program)
            .with_context(|| format!("cannot find executable {}", program.to_string_lossy()))?;
        let mut command = Command::new(executable);
        command.current_dir(&self.root);
        Ok(command)
    }

    pub(crate) fn python(&self) -> Result<Command> {
        if let Some(program) = std::env::var_os("PYTHON") {
            return self.command(program);
        }
        if which::which("python").is_ok() {
            self.command("python")
        } else {
            self.command("python3")
        }
    }
}

pub(crate) trait CommandExt {
    /// Inherit output and fail when the child exits unsuccessfully.
    fn run(&mut self) -> Result<()>;
    /// Capture output without requiring success, for expected failures.
    fn capture(&mut self) -> Result<Output>;
    /// Capture stdout and require success.
    fn read(&mut self) -> Result<String>;
}

impl CommandExt for Command {
    fn run(&mut self) -> Result<()> {
        eprintln!("$ {self:?}");
        let status = self
            .status()
            .with_context(|| format!("starting {self:?}"))?;
        ensure!(status.success(), "{self:?} exited with {status}");
        Ok(())
    }

    fn capture(&mut self) -> Result<Output> {
        eprintln!("$ {self:?}");
        self.output().with_context(|| format!("starting {self:?}"))
    }

    fn read(&mut self) -> Result<String> {
        let output = self.capture()?;
        ensure!(
            output.status.success(),
            "{self:?} exited with {}\n{}",
            output.status,
            diagnostics(&output)
        );
        String::from_utf8(output.stdout).context("command stdout is not UTF-8")
    }
}

pub(crate) fn diagnostics(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Expand source patterns in stable order and fail if a requested input is absent.
pub(crate) fn sources(directory: &Path, patterns: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for pattern in patterns {
        let pattern = directory.join(pattern);
        let pattern = pattern.to_str().context("source pattern is not UTF-8")?;
        let mut matches = glob::glob(pattern)?.collect::<Result<Vec<_>, _>>()?;
        ensure!(
            !matches.is_empty(),
            "missing sources: {pattern}; generate fixtures first"
        );
        matches.sort();
        files.extend(matches);
    }
    Ok(files)
}

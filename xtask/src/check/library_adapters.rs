//! Compile the optional GLM headers against pinned upstream sources.

use std::fs;

use anyhow::Result;

use crate::process::{CommandExt, Workspace};

pub(super) fn run(workspace: &Workspace) -> Result<()> {
    let build = "target/glm-adapters";
    workspace
        .command("cmake")?
        .args(["-S", "tests/library-adapters/cpp", "-B", build])
        .arg(format!(
            "-DINTRLAY_FIXTURES={}",
            workspace.path(build).display()
        ))
        .run()?;
    let source = fs::read_to_string(workspace.path(format!("{build}/glm-source.txt")))?;
    let schema = format!("{build}/schema.json");
    workspace
        .command("cargo")?
        .args([
            "run",
            "-p",
            "intrlay-cli",
            "--",
            "extract-cpp",
            "tests/library-adapters/cpp/schema.hpp",
            "--output",
            &schema,
            "--",
            "-Ipackages/cpp/include",
            "-DGLM_FORCE_INTRINSICS",
        ])
        .arg(format!("-I{source}"))
        .run()?;
    workspace
        .command("cargo")?
        .args([
            "run",
            "-p",
            "intrlay-cli",
            "--",
            "generate",
            &schema,
            "--language",
            "cpp",
            "--module-name",
            "GlmAdapters",
            "--output",
            &format!("{build}/bindings.hpp"),
        ])
        .run()?;
    workspace
        .command("cmake")?
        .args(["--build", build, "--config", "Release"])
        .run()?;
    workspace
        .command("ctest")?
        .args(["--test-dir", build, "-C", "Release", "--output-on-failure"])
        .run()
}

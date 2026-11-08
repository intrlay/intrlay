//! Workspace checks and executable consumers for every generated language.

mod conformance;
mod csharp_unions;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::process::{CommandExt, Workspace, diagnostics, sources};
use crate::release;

const DART_PACKAGES: &str = "--packages=tests/dart/.dart_tool/package_config.json";

pub(crate) fn run(workspace: &Workspace) -> Result<()> {
    release::package_sources(workspace)?;
    check_rust(workspace)?;
    generate_frontend_fixtures(workspace)?;
    check_rust_consumers(workspace)?;
    check_typescript(workspace)?;
    check_python(workspace)?;
    check_dart(workspace)?;
    check_csharp(workspace)?;
    check_cpp(workspace)?;
    conformance::run(workspace)?;
    for fixture in ["cpp-metadata", "cpp-collections"] {
        cpp(
            workspace,
            &format!("tests/{fixture}/main.cpp"),
            &format!("target/{fixture}"),
            &[&format!("target/{fixture}")],
        )?;
    }
    Ok(())
}

fn check_rust(workspace: &Workspace) -> Result<()> {
    workspace
        .command("cargo")?
        .args(["fmt", "--all", "--check"])
        .run()?;
    for consumer in ["rust-generated", "compositions/rust", "edge-cases/rust"] {
        workspace
            .command("rustfmt")?
            .args(["--check", "--config", "skip_children=true"])
            .args(sources(
                &workspace.path(format!("tests/{consumer}/src")),
                &["*.rs"],
            )?)
            .run()?;
    }
    workspace.command("cargo")?.arg("lint").run()?;
    workspace
        .command("cargo")?
        .args(["test", "--workspace"])
        .run()?;
    workspace
        .command("cargo")?
        .args(["run", "-p", "intrlay-conformance", "--", "target"])
        .run()
}

fn generate_frontend_fixtures(workspace: &Workspace) -> Result<()> {
    workspace
        .command("cargo")?
        .args([
            "run",
            "-p",
            "intrlay-cli",
            "--",
            "generate",
            "tests/cli/scalar.json",
            "--language",
            "typescript",
            "--output",
            "target/cli-scalar.ts",
        ])
        .run()?;
    for (fixture, schema, language, module, output) in [
        ("schema", "cpp-schema", "rust", None, "cpp-source.rs"),
        (
            "annotations",
            "cpp-annotations",
            "cpp",
            Some("Annotations"),
            "cpp-annotations.hpp",
        ),
        (
            "codec_cases",
            "cpp-codecs",
            "cpp",
            Some("Codecs"),
            "cpp-codecs.hpp",
        ),
    ] {
        let schema = format!("target/{schema}.json");
        workspace
            .command("cargo")?
            .args([
                "run",
                "-p",
                "intrlay-cli",
                "--",
                "extract-cpp",
                &format!("crates/intrlay-frontend-cpp/tests/fixtures/{fixture}.hpp"),
                "--output",
                &schema,
                "--",
                "-Ipackages/cpp/include",
            ])
            .run()?;
        let mut command = workspace.command("cargo")?;
        command.args([
            "run",
            "-p",
            "intrlay-cli",
            "--",
            "generate",
            &schema,
            "--language",
            language,
            "--output",
            &format!("target/regressions/{output}"),
        ]);
        if let Some(module) = module {
            command.args(["--module-name", module]);
        }
        command.run()?;
    }
    Ok(())
}

fn check_rust_consumers(workspace: &Workspace) -> Result<()> {
    workspace
        .command("cargo")?
        .args(["run", "--manifest-path", "tests/rust-generated/Cargo.toml"])
        .run()?;
    for (feature, command, diagnostic) in [
        ("forbidden-create", "check", "create"),
        (
            "forbidden-replace",
            "build",
            "in-place replacement requires a fixed encoding",
        ),
    ] {
        let result = workspace
            .command("cargo")?
            .args([
                command,
                "--manifest-path",
                "tests/rust-generated/Cargo.toml",
                "--features",
                feature,
            ])
            .capture()?;
        let messages = diagnostics(&result);
        ensure!(
            !result.status.success() && messages.contains(diagnostic),
            "expected unavailable API compile failure: {feature}\n{messages}"
        );
    }
    Ok(())
}

fn check_typescript(workspace: &Workspace) -> Result<()> {
    let package = workspace.path("packages/typescript");
    workspace
        .command("npm")?
        .arg("ci")
        .current_dir(&package)
        .run()?;
    workspace
        .command("npm")?
        .args(["run", "build"])
        .current_dir(&package)
        .run()?;
    workspace
        .command("npm")?
        .args([
            "install",
            "--prefix",
            "target",
            "--no-package-lock",
            "--no-save",
            "./packages/typescript",
        ])
        .run()?;
    for arguments in [&["ci"][..], &["run", "check"]] {
        workspace
            .command("npm")?
            .args(arguments)
            .current_dir(workspace.path("tests/typescript"))
            .run()?;
    }
    compile_typescript_fixtures(workspace)?;
    workspace
        .command("npm")?
        .arg("test")
        .current_dir(workspace.path("tests/typescript"))
        .run()?;
    for fixture in ["naming", "regressions", "annotations"] {
        workspace
            .command("node")?
            .arg(format!("tests/typescript/{fixture}.mjs"))
            .run()?;
    }
    Ok(())
}

fn generated_sources(workspace: &Workspace, extension: &str) -> Result<Vec<std::path::PathBuf>> {
    let directory = workspace.path("target");
    let manifest: Vec<std::path::PathBuf> =
        serde_json::from_slice(&fs::read(directory.join("intrlay-fixtures.json"))?)?;
    Ok(manifest
        .into_iter()
        .filter(|path| path.extension().is_some_and(|value| value == extension))
        .map(|path| directory.join(path))
        .collect())
}

fn compile_typescript_fixtures(workspace: &Workspace) -> Result<()> {
    // Compile const enum declarations before executing JavaScript consumers;
    // Node's type stripping cannot transform these TypeScript declarations.
    workspace
        .command("node")?
        .args([
            "tests/typescript/node_modules/typescript/bin/tsc",
            "--strict",
            "--noUnusedLocals",
            "--noUnusedParameters",
            "--target",
            "ES2022",
            "--module",
            "ESNext",
            "--moduleResolution",
            "Bundler",
            "--noEmitOnError",
        ])
        .args(generated_sources(workspace, "ts")?)
        .run()
}

fn check_python(workspace: &Workspace) -> Result<()> {
    workspace
        .python()?
        .args([
            "-m",
            "pip",
            "install",
            "-e",
            "packages/python",
            "-r",
            "tests/python-types/requirements.txt",
        ])
        .run()?;
    check_python_arguments(workspace)?;
    workspace
        .python()?
        .args(["tests/python.py", "target/conformance"])
        .run()?;
    for fixture in [
        "tests/python-types/check.py",
        "tests/python-regressions.py",
        "tests/construction-extents.py",
    ] {
        workspace.python()?.arg(fixture).run()?;
    }
    Ok(())
}

fn check_python_arguments(workspace: &Workspace) -> Result<()> {
    let target = workspace.path("target");
    let mut sources = Vec::new();
    for source in generated_sources(workspace, "py")? {
        let destination = target
            .join("python-unused-arguments")
            .join(source.strip_prefix(&target)?);
        fs::create_dir_all(destination.parent().context("generated source directory")?)?;
        // Keep consumer-facing lint suppression in generated files. These test
        // copies retain only narrow exceptions for public keyword parameters.
        let code = fs::read_to_string(source)?.replacen("# flake8: noqa", "", 1);
        fs::write(&destination, code)?;
        sources.push(destination);
    }
    workspace
        .python()?
        .args([
            "-m",
            "ruff",
            "check",
            "--isolated",
            "--select",
            "ARG,RUF100",
            "--cache-dir",
            "target/ruff-cache",
        ])
        .args(sources)
        .run()
}

fn check_dart(workspace: &Workspace) -> Result<()> {
    let directory = workspace.path("tests/dart");
    workspace
        .command("dart")?
        .args(["pub", "get"])
        .current_dir(&directory)
        .run()?;
    workspace
        .command("dart")?
        .args([DART_PACKAGES, "analyze"])
        .args(generated_sources(workspace, "dart")?)
        .run()?;
    workspace
        .command("dart")?
        .arg("analyze")
        .current_dir(&directory)
        .run()?;
    workspace
        .command("dart")?
        .args(["run", "conformance.dart", "../../target/conformance"])
        .current_dir(&directory)
        .run()?;
    for fixture in [
        "regressions",
        "construction-extents",
        "annotations",
        "runtime",
        "writes",
        "private-fields",
        "tagged-scalars",
    ] {
        workspace
            .command("dart")?
            .args(["run", &format!("{fixture}.dart")])
            .current_dir(&directory)
            .run()?;
    }
    for fixture in [
        "construction-extents",
        "regressions",
        "private-fields",
        "tagged-scalars",
        "runtime",
        "writes",
    ] {
        workspace
            .command("dart")?
            .args([
                "compile",
                "js",
                "-O2",
                &format!("{fixture}.dart"),
                "-o",
                &format!("../../target/dart-{fixture}.cjs"),
            ])
            .current_dir(&directory)
            .run()?;
        workspace
            .command("node")?
            .args([
                "tests/dart/run-web.mjs",
                &format!("target/dart-{fixture}.cjs"),
            ])
            .run()?;
    }
    Ok(())
}

fn check_csharp(workspace: &Workspace) -> Result<()> {
    for (project, arguments) in [
        (
            "tests/csharp/Conformance.csproj",
            &["target/conformance"][..],
        ),
        ("tests/csharp-naming/Naming.csproj", &[]),
        ("tests/csharp-regressions/Regressions.csproj", &[]),
        ("tests/csharp-native-views/NativeViews.csproj", &[]),
        ("tests/csharp-initialization/Initialization.csproj", &[]),
        ("tests/native-union-depth/NativeUnionDepth.csproj", &[]),
    ] {
        workspace
            .command("dotnet")?
            .args(["run", "--project", project, "-c", "Release", "--"])
            .args(arguments)
            .run()?;
    }

    // The packaged analyzer must work without a separate generator reference.
    let version = release::workspace_version(workspace)?;
    release::check_versions(workspace, &version)?;
    workspace
        .command("dotnet")?
        .args([
            "pack",
            "packages/csharp/Intrlay.csproj",
            "-c",
            "Release",
            "-o",
            "target/nuget",
        ])
        .run()?;
    let cache = tempfile::Builder::new()
        .prefix("nuget-consumer-")
        .tempdir_in(workspace.path("target"))?;
    workspace
        .command("dotnet")?
        .args([
            "run",
            "--project",
            "tests/csharp-package/Package.csproj",
            "-c",
            "Release",
        ])
        .arg(format!("-p:IntrlayVersion={version}"))
        .arg(format!(
            "-p:RestoreSources={}",
            workspace.path("target/nuget").display()
        ))
        .arg(format!("-p:RestorePackagesPath={}", cache.path().display()))
        .args(["--", "target/conformance"])
        .run()?;
    csharp_unions::run(workspace)
}

fn check_cpp(workspace: &Workspace) -> Result<()> {
    for fixture in ["conformance", "regressions"] {
        cpp(
            workspace,
            &format!("tests/cpp/{fixture}.cpp"),
            &format!("target/{fixture}"),
            &[&format!("target/{fixture}")],
        )?;
    }
    check_cpp_source_diagnostics(workspace)?;
    workspace
        .command("cmake")?
        .args(["-S", "packages/cpp", "-B", "target/cmake-runtime"])
        .run()?;
    workspace
        .command("cmake")?
        .args([
            "--install",
            "target/cmake-runtime",
            "--prefix",
            "target/cpp-install",
        ])
        .run()?;
    for fixture in ["package", "collections", "metadata"] {
        let build = if fixture == "package" {
            "target/cmake-consumer".to_owned()
        } else {
            format!("target/cmake-{fixture}")
        };
        let mut command = workspace.command("cmake")?;
        command
            .args(["-S", &format!("tests/cpp-{fixture}"), "-B", &build])
            .arg(format!(
                "-DCMAKE_PREFIX_PATH={}",
                workspace.path("target/cpp-install").display()
            ));
        if fixture != "package" {
            command.arg(format!(
                "-DINTRLAY_FIXTURES={}",
                workspace.path(format!("target/cpp-{fixture}")).display()
            ));
        }
        command.run()?;
        workspace
            .command("cmake")?
            .args(["--build", &build, "--config", "Release"])
            .run()?;
        workspace
            .command("ctest")?
            .args(["--test-dir", &build, "-C", "Release", "--output-on-failure"])
            .run()?;
    }
    Ok(())
}

fn check_cpp_source_diagnostics(workspace: &Workspace) -> Result<()> {
    let compiler = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    for (fixture, source, definition, expected) in [
        (
            "replacement",
            "tests/replacement/cpp.cpp",
            "INTRLAY_TEST_INVALID_MUTATIONS",
            &[
                "constraints not satisfied",
                "SetSourceFor",
                "FixedEncodeSourceFor",
                "set_number",
                "set_numbers_values",
            ][..],
        ),
        (
            "cpp-metadata",
            "tests/cpp-metadata/main.cpp",
            "INTRLAY_TEST_INVALID_SOURCES",
            &[
                "constraints not satisfied",
                "EncodeSourceFor",
                "PolymorphicBitfieldChild",
            ][..],
        ),
    ] {
        let result = workspace
            .command(&compiler)?
            .args(["-std=c++20", "-fsyntax-only", "-Ipackages/cpp/include"])
            .arg(format!("-D{definition}"))
            .arg(format!("-Itarget/{fixture}"))
            .arg(source)
            .capture()?;
        let messages = diagnostics(&result);
        ensure!(
            !result.status.success() && expected.iter().all(|expected| messages.contains(expected)),
            "expected {fixture} source constraint diagnostics:\n{messages}"
        );
    }
    Ok(())
}

fn typescript(workspace: &Workspace, sources: &[impl AsRef<Path>]) -> Result<()> {
    workspace
        .command("node")?
        .args([
            "tests/typescript/node_modules/typescript/bin/tsc",
            "--strict",
            "--noUnusedLocals",
            "--noUnusedParameters",
            "--noEmit",
            "--target",
            "ES2022",
            "--module",
            "nodenext",
            "--moduleResolution",
            "nodenext",
        ])
        .args(sources.iter().map(AsRef::as_ref))
        .run()
}

fn dart(
    workspace: &Workspace,
    sources: &[impl AsRef<Path>],
    consumer: &str,
    web: bool,
    output: &str,
) -> Result<()> {
    workspace
        .command("dart")?
        .args([DART_PACKAGES, "analyze"])
        .args(sources.iter().map(AsRef::as_ref))
        .arg(consumer)
        .run()?;
    workspace
        .command("dart")?
        .args([DART_PACKAGES, "run", consumer])
        .run()?;
    if web {
        workspace
            .command("dart")?
            .args([
                "compile",
                "js",
                DART_PACKAGES,
                "-O2",
                consumer,
                "-o",
                output,
            ])
            .run()?;
        workspace
            .command("node")?
            .args(["tests/dart/run-web.mjs", output])
            .run()?;
    }
    Ok(())
}

fn cpp(workspace: &Workspace, source: &str, directory: &str, arguments: &[&str]) -> Result<()> {
    fs::create_dir_all(workspace.path(directory))?;
    let suffix = std::env::consts::EXE_SUFFIX;
    let executable = format!("{directory}/cpp{suffix}");
    let compiler = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    workspace
        .command(compiler)?
        .args(["-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-I{directory}"))
        .args(["-Ipackages/cpp/include", source, "-o", &executable])
        .run()?;
    workspace
        .command(workspace.path(executable))?
        .args(arguments)
        .run()
}

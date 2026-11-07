//! Shared feature cases with each language's own consumer and diagnostic checks.

use std::fs;

use anyhow::{Context, Result, ensure};
use regex::Regex;

use crate::process::{CommandExt, Workspace, diagnostics, sources};

struct Case {
    name: &'static str,
    typescript_sources: &'static [&'static str],
    python_sources: &'static [&'static str],
    dart_sources: &'static [&'static str],
    typecheck_python_consumer: bool,
    dart_web: bool,
    compiled_consumers: &'static [Consumer],
}

enum Consumer {
    Csharp(&'static str),
    Cpp,
    Rust,
}

impl Case {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            typescript_sources: &["bindings.ts"],
            python_sources: &["bindings.py"],
            dart_sources: &["bindings.dart"],
            typecheck_python_consumer: false,
            dart_web: false,
            compiled_consumers: &[],
        }
    }
}

const CASES: &[Case] = &[
    Case {
        typescript_sources: &["alternatives.ts"],
        python_sources: &["alternatives.py"],
        dart_sources: &["alternatives.dart"],
        typecheck_python_consumer: true,
        dart_web: true,
        compiled_consumers: &[Consumer::Csharp("CppMetadata")],
        ..Case::new("cpp-metadata")
    },
    Case {
        typescript_sources: &["bindings.ts", "../../tests/compositions/typescript.ts"],
        dart_sources: &["bindings.dart", "setter-names.dart"],
        typecheck_python_consumer: true,
        dart_web: true,
        compiled_consumers: &[
            Consumer::Csharp("Compositions"),
            Consumer::Cpp,
            Consumer::Rust,
        ],
        ..Case::new("compositions")
    },
    Case {
        typescript_sources: &["bindings.ts", "scalar.ts"],
        python_sources: &["bindings.py", "scalar.py"],
        dart_sources: &["bindings.dart", "scalar.dart"],
        compiled_consumers: &[Consumer::Csharp("EdgeCases"), Consumer::Cpp, Consumer::Rust],
        ..Case::new("edge-cases")
    },
    Case {
        dart_web: true,
        compiled_consumers: &[Consumer::Csharp("Replacement"), Consumer::Cpp],
        ..Case::new("replacement")
    },
    Case {
        typecheck_python_consumer: true,
        ..Case::new("reference-fixes")
    },
    Case {
        typescript_sources: &[],
        python_sources: &["*.py"],
        dart_sources: &["*.dart"],
        typecheck_python_consumer: true,
        dart_web: true,
        ..Case::new("reference-scopes")
    },
];

pub(super) fn run(workspace: &Workspace) -> Result<()> {
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
            "csharp",
            "--options",
            "tests/cli/csharp-options.json",
            "--output",
            "target/compositions/cli.cs",
        ])
        .run()?;
    for case in CASES {
        check_case(workspace, case)?;
    }
    check_python_errors(workspace)?;
    check_csharp_storage(workspace)?;
    check_expanded_source(workspace)
}

fn check_case(workspace: &Workspace, case: &Case) -> Result<()> {
    let consumer = format!("tests/{}", case.name);
    let generated = format!("target/{}", case.name);
    let directory = workspace.path(&generated);
    if !case.typescript_sources.is_empty() {
        super::typescript(workspace, &sources(&directory, case.typescript_sources)?)?;
        workspace
            .command("node")?
            .arg(format!("{consumer}/typescript.mjs"))
            .run()?;
    }
    let mut python_sources = sources(&directory, case.python_sources)?;
    if case.typecheck_python_consumer {
        python_sources.push(workspace.path(format!("{consumer}/python.py")));
    }
    workspace
        .python()?
        .args(["-m", "mypy", "--strict", "--cache-dir"])
        .arg(workspace.path("target/mypy-cache"))
        .args(python_sources)
        .env("MYPYPATH", &directory)
        .run()?;
    workspace
        .python()?
        .arg(format!("{consumer}/python.py"))
        .run()?;
    super::dart(
        workspace,
        &sources(&directory, case.dart_sources)?,
        &format!("{consumer}/dart.dart"),
        case.dart_web,
        &format!("{generated}/dart-web.cjs"),
    )?;
    for target in case.compiled_consumers {
        match target {
            Consumer::Csharp(project) => workspace
                .command("dotnet")?
                .args([
                    "run",
                    "--project",
                    &format!("{consumer}/{project}.csproj"),
                    "-c",
                    "Release",
                ])
                .run()?,
            Consumer::Cpp => {
                super::cpp(workspace, &format!("{consumer}/cpp.cpp"), &generated, &[])?;
            }
            Consumer::Rust => workspace
                .command("cargo")?
                .args([
                    "run",
                    "--manifest-path",
                    &format!("{consumer}/rust/Cargo.toml"),
                ])
                .run()?,
        }
    }
    Ok(())
}

fn check_python_errors(workspace: &Workspace) -> Result<()> {
    let path = workspace.path("tests/compositions/invalid.py");
    let result = workspace
        .python()?
        .args(["-m", "mypy", "--strict", "--cache-dir"])
        .arg(workspace.path("target/mypy-cache"))
        .arg(&path)
        .env("MYPYPATH", workspace.path("target/compositions"))
        .capture()?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    let actual_pattern = Regex::new(r"error: .*\[([^]]+)\]")?;
    let actual = actual_pattern
        .captures_iter(&stdout)
        .map(|capture| capture[1].to_owned())
        .collect::<Vec<_>>();
    let expected_pattern = Regex::new(r"# EXPECT: ([a-z-]+)")?;
    let source = fs::read_to_string(path)?;
    let expected = expected_pattern
        .captures_iter(&source)
        .map(|capture| capture[1].to_owned())
        .collect::<Vec<_>>();
    ensure!(
        result.status.code() == Some(1) && actual == expected,
        "unexpected mypy diagnostics:\n{}",
        diagnostics(&result)
    );
    Ok(())
}

fn check_csharp_storage(workspace: &Workspace) -> Result<()> {
    let project = "tests/csharp-recursive-storage/RecursiveStorage.csproj";
    for framework in ["net8.0", "net10.0"] {
        workspace
            .command("dotnet")?
            .args([
                "run",
                "--project",
                project,
                "-c",
                "Release",
                "--framework",
                framework,
            ])
            .run()?;
    }
    let result = workspace
        .command("dotnet")?
        .args([
            "build",
            project,
            "-c",
            "Release",
            "--framework",
            "net10.0",
            "-p:TestInvalidSchema=true",
        ])
        .capture()?;
    let messages = diagnostics(&result);
    ensure!(
        !result.status.success()
            && messages.contains("error ILAY001:")
            && messages.contains("recursive collection storage name"),
        "{messages}"
    );
    Ok(())
}

fn check_expanded_source(workspace: &Workspace) -> Result<()> {
    for path in sources(&workspace.path("target/edge-cases/expanded"), &["**/*.cs"])? {
        for line in fs::read_to_string(&path)?
            .trim_start_matches('\u{feff}')
            .lines()
        {
            if line.trim_start().starts_with("return ") {
                ensure!(
                    line.chars().count() < 240,
                    "unsplit nested return expression in {}",
                    path.display()
                );
            }
        }
    }
    let source = fs::read_to_string(workspace.path("target/conformance/expanded/Intrlay.Generator/Intrlay.Generator.BindingGenerator/IntrlayGenerated.Schema.Bindings.g.cs"))
        .context("expected expanded Roslyn source")?;
    for text in ["Position along the X axis, in metres.", "An output value."] {
        ensure!(
            source.matches(&format!("/// {text}")).count() >= 2,
            "native properties and views must both preserve field documentation: {text}"
        );
    }
    Ok(())
}

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
        dart_web: true,
        ..Case::new("field-helpers")
    },
    Case {
        typecheck_python_consumer: true,
        dart_web: true,
        compiled_consumers: &[
            Consumer::Csharp("LibraryAdapters"),
            Consumer::Cpp,
            Consumer::Rust,
        ],
        ..Case::new("library-adapters")
    },
    Case {
        typescript_sources: &["alternatives.ts", "enum_getters.ts"],
        python_sources: &["alternatives.py", "enum_getters.py"],
        dart_sources: &["alternatives.dart", "enum_getters.dart"],
        typecheck_python_consumer: true,
        dart_web: true,
        compiled_consumers: &[Consumer::Csharp("CppMetadata")],
        ..Case::new("cpp-metadata")
    },
    Case {
        typescript_sources: &["bindings.ts", "../../tests/compositions/typescript.ts"],
        python_sources: &["bindings.py", "value_enums.py"],
        dart_sources: &["bindings.dart", "setter-names.dart", "value-enums.dart"],
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
    check_value_enum_typescript(workspace)?;
    check_python_errors(workspace)?;
    check_csharp_storage(workspace)?;
    check_csharp_sources(workspace)?;
    check_expanded_source(workspace)
}

fn check_csharp_sources(workspace: &Workspace) -> Result<()> {
    let result = workspace
        .command("dotnet")?
        .args([
            "build",
            "tests/csharp-native-views/NativeViews.csproj",
            "-c",
            "Release",
            "-p:TestInvalidSources=true",
        ])
        .capture()?;
    let messages = diagnostics(&result);
    let pattern = Regex::new(r"InvalidSourceTests\.cs\((\d+),\d+\): error (CS\d+):")?;
    let errors = pattern
        .captures_iter(&messages)
        .map(|capture| (capture[1].to_owned(), capture[2].to_owned()))
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(
        !result.status.success()
            && errors.len() == 5
            && errors.iter().filter(|(_, code)| code == "CS0315").count() == 3
            && errors.iter().filter(|(_, code)| code == "CS1503").count() == 1
            && errors.iter().filter(|(_, code)| code == "CS9244").count() == 1
            && !messages.contains("error ILAY001:"),
        "unexpected generic input diagnostics:\n{messages}"
    );
    Ok(())
}

fn check_value_enum_typescript(workspace: &Workspace) -> Result<()> {
    // Exercise both inlined enum constants and isolated modules' runtime enum
    // objects, including member access, numeric literals, and re-exports.
    for isolated in [false, true] {
        let mut compiler = workspace.command("node")?;
        compiler.args([
            "tests/typescript/node_modules/typescript/bin/tsc",
            "--strict",
            "--target",
            "ES2022",
            "--module",
            "ESNext",
            "--moduleResolution",
            "Bundler",
            "--rootDir",
            ".",
            "--outDir",
            "target/compositions/compiled",
            "tests/compositions/value-enums.ts",
        ]);
        if isolated {
            compiler.arg("--isolatedModules");
        }
        compiler.run()?;
        workspace
            .command("node")?
            .arg("tests/compositions/value-enums.mjs")
            .run()?;
    }
    Ok(())
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
    workspace
        .command("dotnet")?
        .args([
            "run",
            "--project",
            project,
            "-c",
            "Release",
            "--framework",
            "net10.0",
        ])
        .run()?;
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

//! Compile both the C# union protocol and its intended consumer diagnostics.

use anyhow::{Result, ensure};

use crate::process::{CommandExt, Workspace, diagnostics};

pub(super) fn run(workspace: &Workspace) -> Result<()> {
    let dotnet = std::env::var_os("DOTNET11").unwrap_or_else(|| "dotnet".into());
    for project in [
        "tests/csharp-native-views/NativeViews.csproj",
        "tests/native-union-depth/NativeUnionDepth.csproj",
    ] {
        workspace
            .command(&dotnet)?
            .args([
                "run",
                "--project",
                project,
                "-c",
                "Release",
                "-p:TestCSharp15=true",
            ])
            .run()?;
    }

    for (property, expected) in [
        ("TestIncompleteUnion", &["CS8509"][..]),
        (
            "TestWrongUnionLanguage",
            &["ILAY001", "require C# 15", "compilation uses C# 14"],
        ),
        (
            "TestMissingUnionRuntime",
            &["ILAY001", ".NET 11 UnionAttribute and IUnion"],
        ),
    ] {
        let result = workspace
            .command(&dotnet)?
            .args([
                "build",
                "tests/csharp-native-views/NativeViews.csproj",
                "-c",
                "Release",
                "-p:TestCSharp15=true",
            ])
            .arg(format!("-p:{property}=true"))
            .capture()?;
        let messages = diagnostics(&result);
        ensure!(
            !result.status.success() && expected.iter().all(|message| messages.contains(message)),
            "expected C# union diagnostic for {property}:\n{messages}"
        );
    }
    Ok(())
}

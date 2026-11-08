//! The crate-input CLI must collect real Rust types without building the application.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

type TestResult = Result<(), Box<dyn Error>>;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_intrlay"));
    command.env(
        "CARGO_TARGET_DIR",
        repository().join("target/cli-rust-input"),
    );
    command
}

fn generate(manifest: &Path, output: &Path, arguments: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(cli()
        .args(["generate", "--manifest-path"])
        .arg(manifest)
        .args(["--language", "typescript", "--offline", "--output"])
        .arg(output)
        .args(arguments)
        .output()?)
}

fn succeeds(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fails_with(output: &Output, diagnostic: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unexpected success");
    assert!(
        stderr.contains(diagnostic),
        "expected {diagnostic}: {stderr}"
    );
}

fn write(root: &Path, path: &str, contents: &str) -> TestResult {
    let path = root.join(path);
    fs::create_dir_all(path.parent().ok_or("fixture path has no parent")?)?;
    fs::write(path, contents)?;
    Ok(())
}

fn fixture(root: &Path) -> TestResult {
    let runtime = repository().join("packages/rust").canonicalize()?;
    let runtime = serde_json::to_string(&runtime.to_string_lossy())?;
    write(
        root,
        "Cargo.toml",
        &format!(
            r#"
[workspace]
members = ["schema", "application"]
exclude = ["local-types"]
resolver = "3"

[workspace.dependencies]
wire = {{ package = "intrlay", path = {runtime} }}

[patch.crates-io]
intrlay-cli-test-types = {{ path = "local-types" }}
"#
        ),
    )?;
    write(
        root,
        ".cargo/config.toml",
        "[build]\ntarget = 'wasm32-unknown-unknown'\n",
    )?;
    write(
        root,
        "schema/Cargo.toml",
        r#"
[package]
name = "protocol-schema"
version = "0.0.0"
edition = "2024"

[lib]
name = "custom_library_name"

[features]
default = ["baseline"]
baseline = []
extra = []

[dependencies]
wire.workspace = true
intrlay-cli-test-types = "0.0.0"
"#,
    )?;
    write(
        root,
        "schema/src/lib.rs",
        r#"
#![deny(warnings)]
#[cfg(target_arch = "wasm32")]
compile_error!("schema collector must run on the host");

mod internal {
    #[derive(wire::Intrlay)]
    pub struct Message<T> {
        value: T,
        label: String,
    }
}

type IntegerMessage = internal::Message<intrlay_cli_test_types::Value>;

wire::export_types! {
    r#IntegerMessage,
    #[cfg(feature = "baseline")]
    r#Baseline = u8,
    #[cfg(feature = "extra")]
    Extra = u64,
}
"#,
    )?;
    write(
        root,
        "application/Cargo.toml",
        "[package]\nname = 'application'\nversion = '0.0.0'\nedition = '2024'\n",
    )?;
    write(
        root,
        "application/src/main.rs",
        "compile_error!(\"application must not compile during codegen\");\nfn main() {}\n",
    )?;
    write(
        root,
        "local-types/Cargo.toml",
        "[package]\nname = 'intrlay-cli-test-types'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )?;
    write(root, "local-types/src/lib.rs", "pub type Value = u32;\n")?;
    Ok(())
}

#[test]
fn workspace_collection_respects_features_overrides_and_private_types() -> TestResult {
    let temporary = tempfile::Builder::new()
        .prefix("intrlay cli workspace ")
        .tempdir()?;
    let root = temporary.path();
    fixture(root)?;
    let workspace = root.join("Cargo.toml");
    let member = root.join("schema/Cargo.toml");
    let output = root.join("generated/bindings.ts");
    let mut cargo = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    succeeds(
        &cargo
            .current_dir(root)
            .args(["generate-lockfile", "--offline"])
            .output()?,
    );
    let original_lock = fs::read(root.join("Cargo.lock"))?;
    let original_source = fs::read(root.join("schema/src/lib.rs"))?;

    fails_with(&generate(&workspace, &output, &[])?, "use --package");
    fails_with(
        &generate(&workspace, &output, &["-p", "absent"])?,
        "no package named absent",
    );
    succeeds(&generate(&workspace, &output, &["-p", "protocol-schema"])?);
    let source = fs::read_to_string(&output)?;
    assert!(source.contains("IntegerMessage"));
    assert!(source.contains("BaselineView"));
    assert!(!source.contains("ExtraView"));

    succeeds(&generate(
        &member,
        &output,
        &["--no-default-features", "--features", "extra"],
    )?);
    let source = fs::read_to_string(&output)?;
    assert!(!source.contains("BaselineView"));
    assert!(source.contains("ExtraView"));
    succeeds(&generate(&member, &output, &["--all-features"])?);
    let source = fs::read_to_string(&output)?;
    assert!(source.contains("BaselineView") && source.contains("ExtraView"));

    let manifest = fs::read_to_string(&workspace)?;
    fs::write(
        &workspace,
        manifest.replacen(
            "[workspace]",
            "[workspace]\ndefault-members = [\"schema\"]",
            1,
        ),
    )?;
    succeeds(&generate(&workspace, &output, &[])?);
    assert_eq!(fs::read(root.join("Cargo.lock"))?, original_lock);
    assert_eq!(fs::read(root.join("schema/src/lib.rs"))?, original_source);

    write(root, "schema/src/lib.rs", "pub struct WithoutExports;\n")?;
    fs::write(&output, "previous output")?;
    fails_with(&generate(&member, &output, &[])?, "intrlay::export_types!");
    assert_eq!(fs::read_to_string(&output)?, "previous output");
    write(
        root,
        "schema/src/lib.rs",
        "compile_error!(\"schema build failure\");\n",
    )?;
    fails_with(&generate(&member, &output, &[])?, "schema build failure");
    fails_with(
        &generate(&workspace, &output, &["-p", "application"])?,
        "needs a library target",
    );
    Ok(())
}

#[test]
fn standalone_crate_and_existing_json_input_both_generate() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    let runtime = repository().join("packages/rust").canonicalize()?;
    let runtime = serde_json::to_string(&runtime.to_string_lossy())?;
    write(
        root,
        "Cargo.toml",
        &format!(
            r#"
[package]
name = "standalone-schema"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
intrlay = {{ path = {runtime} }}
"#
        ),
    )?;
    write(
        root,
        "src/lib.rs",
        "intrlay::export_types! { Counter = u32 }\n",
    )?;
    let output = root.join("bindings.cs");
    let result = cli()
        .args(["generate", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args([
            "-l",
            "csharp",
            "--module-name",
            "Example",
            "--offline",
            "-o",
        ])
        .arg(&output)
        .output()?;
    succeeds(&result);
    assert!(fs::read_to_string(&output)?.contains("namespace IntrlayGenerated.Example;"));

    let scalar = repository().join("tests/cli/scalar.json");
    let result = cli()
        .arg("generate")
        .arg(&scalar)
        .args(["-l", "typescript", "-o"])
        .arg(&output)
        .output()?;
    succeeds(&result);
    for arguments in [
        &["--features", "extra"][..],
        &["--package", "schema"][..],
        &["--all-features"][..],
        &["--no-default-features"][..],
        &["--offline"][..],
    ] {
        let result = cli()
            .arg("generate")
            .arg(&scalar)
            .args(["-l", "typescript", "-o"])
            .arg(&output)
            .args(arguments)
            .output()?;
        fails_with(&result, "cannot be used with");
    }
    let result = cli()
        .arg("generate")
        .arg(&scalar)
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .args(["-l", "typescript", "-o"])
        .arg(&output)
        .output()?;
    fails_with(&result, "cannot be used with");
    let result = cli()
        .args(["generate", "-l", "typescript", "-o"])
        .arg(&output)
        .output()?;
    fails_with(&result, "required arguments were not provided");
    Ok(())
}

#[test]
fn csharp_language_flag_overrides_the_options_file() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let options = temporary.path().join("options.json");
    let output = temporary.path().join("bindings.cs");
    fs::write(&options, r#"{"csharp": {"language_version": 15}}"#)?;
    for version in [None, Some("14"), Some("15"), Some("16"), Some("13")] {
        let mut command = cli();
        command
            .arg("generate")
            .arg(repository().join("tests/cli/scalar.json"))
            .args(["--language", "csharp", "--options"])
            .arg(&options)
            .arg("--output")
            .arg(&output);
        if let Some(version) = version {
            command.args(["--csharp-language-version", version]);
        }
        let result = command.output()?;
        if version == Some("13") {
            fails_with(&result, "C# 14 or later");
        } else {
            succeeds(&result);
            let source = fs::read_to_string(&output)?;
            if version == Some("14") {
                assert!(source.contains("[global::Intrlay.GenerateBindings]"));
            } else {
                let selected = version.unwrap_or("15");
                assert!(source.contains(&format!("CSharpLanguageVersion = {selected}")));
            }
        }
    }
    Ok(())
}

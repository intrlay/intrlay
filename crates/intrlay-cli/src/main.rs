//! Standalone code generation from compiler-resolved Rust or C++ schema metadata.

use std::{error::Error, fs, path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand, ValueEnum};
use intrlay_codegen::{GenerateOptions, generate_with_options};
use intrlay_schema::{Language, Schema};

#[derive(Parser)]
#[command(version, about = "Typed memory views for trusted FFI buffers")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a binding from a Rust crate or exported schema metadata.
    #[command(group(clap::ArgGroup::new("source").required(true).args(["schema", "manifest_path"])))]
    Generate {
        #[command(flatten)]
        source: Source,
        #[arg(short, long, value_enum)]
        language: Target,
        #[arg(short, long)]
        output: PathBuf,
        /// JSON `GenerateOptions`, including module-wide naming rules.
        #[arg(long)]
        options: Option<PathBuf>,
        /// Stable generation unit name, used by the default C++/C# namespaces.
        #[arg(long)]
        module_name: Option<String>,
        /// Minimum C# language version (default 14). Version 15 enables union patterns.
        #[arg(long)]
        csharp_language_version: Option<u16>,
    },
    /// Resolve annotated C++20 types through libclang and write common metadata.
    ExtractCpp {
        header: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        compilation_database: Option<PathBuf>,
        /// Clang flags after --, such as -Ipackages/cpp/include.
        #[arg(last = true)]
        arguments: Vec<String>,
    },
}

#[derive(Args)]
#[group(skip)]
struct Source {
    /// JSON schema written by `Registry::finish().to_json()` or extract-cpp.
    schema: Option<PathBuf>,
    /// Cargo.toml of a Rust schema library declaring `intrlay::export_types!`.
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Select one package when the manifest describes a Cargo workspace.
    #[arg(short, long, requires = "manifest_path", conflicts_with = "schema")]
    package: Option<String>,
    /// Enable features on the selected schema library (comma-separated).
    #[arg(
        long,
        value_delimiter = ',',
        requires = "manifest_path",
        conflicts_with = "schema"
    )]
    features: Vec<String>,
    /// Enable every feature of the selected schema library.
    #[arg(long, requires = "manifest_path", conflicts_with = "schema")]
    all_features: bool,
    /// Disable the selected schema library's default features.
    #[arg(long, requires = "manifest_path", conflicts_with = "schema")]
    no_default_features: bool,
    /// Prevent Cargo network access during schema collection.
    #[arg(long, requires = "manifest_path", conflicts_with = "schema")]
    offline: bool,
}

impl Source {
    fn load(self) -> Result<Schema, Box<dyn Error>> {
        if let Some(manifest) = self.manifest_path {
            let options = intrlay_frontend_rust::Options {
                package: self.package,
                features: self.features,
                all_features: self.all_features,
                no_default_features: self.no_default_features,
                offline: self.offline,
            };
            Ok(intrlay_frontend_rust::extract(manifest, &options)?)
        } else {
            let path = self
                .schema
                .ok_or("expected schema JSON or --manifest-path")?;
            Ok(Schema::from_json(&fs::read_to_string(path)?)?)
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Target {
    Rust,
    Cpp,
    #[value(name = "typescript")]
    TypeScript,
    #[value(name = "csharp")]
    CSharp,
    Python,
    Dart,
}

impl From<Target> for Language {
    fn from(value: Target) -> Self {
        match value {
            Target::Rust => Self::Rust,
            Target::Cpp => Self::Cpp,
            Target::TypeScript => Self::TypeScript,
            Target::CSharp => Self::CSharp,
            Target::Python => Self::Python,
            Target::Dart => Self::Dart,
        }
    }
}

fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    let (path, text) = match command {
        Command::Generate {
            source,
            language,
            output,
            options,
            module_name,
            csharp_language_version,
        } => {
            let schema = source.load()?;
            let mut options: GenerateOptions = options
                .map(|path| {
                    fs::read_to_string(path)
                        .map_err(Into::into)
                        .and_then(|json| {
                            serde_json::from_str(&json).map_err(Box::<dyn Error>::from)
                        })
                })
                .transpose()?
                .unwrap_or_default();
            if let Some(module_name) = module_name {
                options.module_name = Some(module_name);
            }
            if let Some(version) = csharp_language_version {
                options.csharp.language_version = Some(version);
            }
            (
                output,
                generate_with_options(&schema, language.into(), options)?,
            )
        }
        Command::ExtractCpp {
            header,
            output,
            compilation_database,
            arguments,
        } => {
            let options = intrlay_frontend_cpp::Options {
                arguments,
                compilation_database,
            };
            (
                output,
                intrlay_frontend_cpp::extract(header, &options)?.to_json()?,
            )
        }
    };
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}

fn main() -> ExitCode {
    match execute(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("intrlay: {error}");
            ExitCode::FAILURE
        }
    }
}

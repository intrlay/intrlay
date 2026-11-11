//! Repository development tasks invoked with `cargo xtask`.

mod check;
mod package;
mod process;
mod publish;
mod release;

use anyhow::Result;
use clap::{Parser, Subcommand};
use process::Workspace;

#[derive(Parser)]
#[command(about = "Intrlay development and release tasks")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Check the workspace and generated consumers in all six languages.
    Check,
    /// Build and verify release packages without uploading them.
    Package(package::Options),
    /// Prepare release notes, archives, checksums, or a draft GitHub Release.
    #[command(subcommand)]
    Release(release::Task),
    /// Publish verified artifacts, skipping versions already in the registry.
    Publish(publish::Options),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace = Workspace::current()?;
    match cli.task {
        Task::Check => check::run(&workspace),
        Task::Package(options) => package::run(&workspace, &options),
        Task::Release(task) => release::run(&workspace, task),
        Task::Publish(options) => publish::run(&workspace, &options),
    }
}

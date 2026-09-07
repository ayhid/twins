//! Command-line surface of `twins`, declared with clap derive.

use clap::{Parser, Subcommand};

/// Find and remove duplicate files on macOS.
#[derive(Debug, Parser)]
#[command(name = "twins", version, about, disable_version_flag = true)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the version and exit.
    Version,
}

/// Version string printed by `twins version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

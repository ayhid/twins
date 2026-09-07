//! Command-line surface of `twins`, declared with clap derive.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
    /// Find duplicates under the given paths (default: your home folder).
    Scan(ScanArgs),
    /// Scan and print a JSON report (never modifies anything).
    Report(ScanArgs),
    /// Print the version and exit.
    Version,
}

/// Options shared by `scan` and `report`.
#[derive(Debug, Args, Clone)]
#[allow(clippy::struct_excessive_bools, clippy::doc_markdown)] // opt-in switches; docs are --help text
pub struct ScanArgs {
    /// Directories to scan.
    pub paths: Vec<PathBuf>,

    /// Print the report as JSON instead of text.
    #[arg(long)]
    pub json: bool,

    /// Ignore files smaller than this size (e.g. 512, 10K, 1.5MiB).
    #[arg(long, default_value = "1MiB", value_name = "SIZE")]
    pub min_size: String,

    /// Glob to skip; without `/` it matches the file name, with `/` the full
    /// path. Repeatable.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Parallelism for stat and hashing (default: one per CPU, capped at 8).
    #[arg(long, default_value_t = 0, value_name = "N")]
    pub jobs: usize,

    /// Consider empty files.
    #[arg(long)]
    pub include_empty: bool,

    /// Descend into ~/Library.
    #[arg(long)]
    pub include_library: bool,

    /// Descend into node_modules directories.
    #[arg(long)]
    pub include_node_modules: bool,

    /// Accept roots and directories on network volumes.
    #[arg(long)]
    pub include_remote: bool,

    /// Compare candidates byte by byte after hashing.
    #[arg(long)]
    pub verify: bool,

    /// List every file that could not be read.
    #[arg(short, long)]
    pub verbose: bool,
}

/// Version string printed by `twins version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

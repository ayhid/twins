//! `twins` command-line entry point.

mod cli;

use clap::Parser;

use crate::cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => println!("twins {}", cli::VERSION),
    }
}

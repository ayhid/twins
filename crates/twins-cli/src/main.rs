//! `twins` command-line entry point.

mod cli;
mod run;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Scan(args) => run::scan(&args, false),
        Command::Report(args) => run::scan(&args, true),
        Command::Version => {
            println!("twins {}", cli::VERSION);
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("twins: {}", run::describe(&err));
            ExitCode::from(u8::try_from(run::exit_code(&err)).unwrap_or(1))
        }
    }
}

//! `twins` command-line entry point.

mod cli;
mod progress;
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
        // The terminal observer already cleared the progress line on
        // `Finished`, and nothing reached stdout: the report is written only
        // after the pipeline returns Ok.
        Err(err) if run::is_cancelled(&err) => {
            eprintln!("scan cancelled");
            ExitCode::from(130)
        }
        Err(err) => {
            eprintln!("twins: {}", run::describe(&err));
            ExitCode::from(u8::try_from(run::exit_code(&err)).unwrap_or(1))
        }
    }
}

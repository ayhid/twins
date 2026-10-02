//! Runs a scan from parsed arguments and prints the report.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Result, anyhow};
use twins_core::human::parse_size;
use twins_core::observe::{CancelToken, Throttle};
use twins_core::pipeline::{self, PipelineError, ScanSpec};
use twins_core::report;
use twins_core::scan;

use crate::cli::ScanArgs;
use crate::progress::TerminalObserver;

/// Runs `scan` or `report`. `force_json` is set by `report`.
pub fn scan(args: &ScanArgs, force_json: bool) -> Result<()> {
    let json = args.json || force_json;
    let roots = roots(&args.paths)?;
    let min_size = parse_size(&args.min_size)?;
    let opts = scan::Options::new(roots)
        .min_size(min_size)
        .include_empty(args.include_empty)
        .include_library(args.include_library)
        .include_node_modules(args.include_node_modules)
        .include_remote(args.include_remote)
        .exclude(args.exclude.clone())
        .workers(args.jobs);
    // `--jobs` drives both the stat pool and the hashing pool.
    let spec = ScanSpec::new(opts)
        .hash_workers(args.jobs)
        .verify(args.verify);
    let cancel = CancelToken::new();
    // Progress depends only on stderr being a terminal, never on --json (D-09).
    let observer = Throttle::new(
        TerminalObserver::stderr(std::io::stderr().is_terminal(), args.verbose),
        Duration::from_millis(80),
    );
    let outcome = pipeline::scan(&spec, &observer, &cancel)?;

    let r = outcome.report(SystemTime::now());
    let mut out = std::io::stdout().lock();
    if json {
        report::write_json(&mut out, &r)?;
    } else {
        report::write_text(&mut out, &r)?;
        let n = outcome.errors();
        if n > 0 && !args.verbose {
            eprintln!("{n} files could not be read (use --verbose to list them)");
        }
    }
    Ok(())
}

fn roots(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if !paths.is_empty() {
        return Ok(paths.to_vec());
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set; pass a directory to scan"))?;
    Ok(vec![home])
}

/// Exit code for a fatal error: 130 when cancelled, 2 for usage errors
/// (bad roots, globs or sizes), 1 otherwise.
pub fn exit_code(err: &anyhow::Error) -> i32 {
    if let Some(e) = err.downcast_ref::<PipelineError>() {
        return match e {
            PipelineError::Cancelled => 130,
            PipelineError::Scan(_) => 2,
            PipelineError::Find(_) => 1,
        };
    }
    if err
        .downcast_ref::<twins_core::human::ParseSizeError>()
        .is_some()
    {
        2
    } else {
        1
    }
}

/// Whether `err` is a cancelled pipeline run (the user pressed Ctrl+C).
pub fn is_cancelled(_err: &anyhow::Error) -> bool {
    false
}

/// Adds context to a fatal error before printing it.
pub fn describe(err: &anyhow::Error) -> String {
    err.chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_maps_cancel_to_130() {
        assert_eq!(
            exit_code(&anyhow::Error::from(PipelineError::Cancelled)),
            130
        );
    }

    #[test]
    fn exit_code_maps_scan_errors_to_2() {
        let err = PipelineError::from(scan::ScanError::NoRoots);
        assert_eq!(exit_code(&anyhow::Error::from(err)), 2);
    }

    #[test]
    fn exit_code_maps_other_errors_to_1() {
        assert_eq!(exit_code(&anyhow::anyhow!("x")), 1);
    }

    #[test]
    fn is_cancelled_detects_pipeline_cancellation() {
        assert!(is_cancelled(&anyhow::Error::from(PipelineError::Cancelled)));
        assert!(!is_cancelled(&anyhow::Error::from(PipelineError::from(
            scan::ScanError::NoRoots
        ))));
        assert!(!is_cancelled(&anyhow::anyhow!("x")));
    }
}

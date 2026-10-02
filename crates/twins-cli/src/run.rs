//! Runs a scan from parsed arguments and prints the report.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, anyhow};
use signal_hook::consts::signal::SIGINT;
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
    install_sigint(&cancel)?;
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

/// Turns Ctrl+C into a cancel of `cancel`.
///
/// Both actions share the token's flag, and the order matters: the
/// conditional shutdown is registered first. On the first SIGINT it finds
/// the flag still false (so it does nothing), then the second action raises
/// the flag and the pipeline stops at its next check. On a second SIGINT the
/// flag is already true, so the shutdown action exits 130 at once without
/// running at-exit hooks. That is the escape hatch for a read stuck on a slow
/// volume, and it is safe because a scan writes nothing.
fn install_sigint(cancel: &CancelToken) -> Result<()> {
    signal_hook::flag::register_conditional_shutdown(SIGINT, 130, cancel.flag())
        .context("cannot install the Ctrl+C handler")?;
    signal_hook::flag::register(SIGINT, cancel.flag())
        .context("cannot install the Ctrl+C handler")?;
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
/// (bad roots, globs or sizes, or `in-dir` without a keep directory), 1
/// otherwise.
pub fn exit_code(err: &anyhow::Error) -> i32 {
    if let Some(e) = err.downcast_ref::<PipelineError>() {
        return match e {
            PipelineError::Cancelled => 130,
            PipelineError::Scan(_) | PipelineError::MissingKeepDir => 2,
            PipelineError::KeepDir(_) | PipelineError::Find(_) => 1,
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
///
/// `main` checks this before the generic error path so a cancel prints
/// `scan cancelled` and exits 130 instead of a `twins: ` error line.
pub fn is_cancelled(err: &anyhow::Error) -> bool {
    err.downcast_ref::<PipelineError>()
        .is_some_and(PipelineError::is_cancelled)
}

/// Adds context to a fatal error before printing it: every message of the
/// chain, joined by `: `. Core errors such as `ScanError::Glob` and
/// `FsError::Io` print their source and also expose it through `source()`,
/// so a cause the message already ends with is skipped rather than printed
/// twice.
pub fn describe(err: &anyhow::Error) -> String {
    let mut out = String::new();
    for cause in err.chain() {
        let text = cause.to_string();
        if out.ends_with(&text) {
            continue;
        }
        if !out.is_empty() {
            out.push_str(": ");
        }
        out.push_str(&text);
    }
    out
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
    fn exit_code_maps_missing_keep_dir_to_2() {
        let err = anyhow::Error::from(PipelineError::MissingKeepDir);
        assert_eq!(exit_code(&err), 2);
    }

    #[test]
    fn exit_code_maps_other_errors_to_1() {
        assert_eq!(exit_code(&anyhow::anyhow!("x")), 1);
    }

    #[test]
    fn describe_prints_a_source_shown_by_its_parent_once() {
        let io = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let expected = format!("lstat /x: {io}");
        let err = PipelineError::from(scan::ScanError::Io(twins_core::fsutil::FsError::Io {
            op: "lstat",
            path: PathBuf::from("/x"),
            source: io,
        }));
        assert_eq!(describe(&anyhow::Error::from(err)), expected);
    }

    #[test]
    fn describe_joins_context_and_cause() {
        let err = anyhow::anyhow!("inner").context("outer");
        assert_eq!(describe(&err), "outer: inner");
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

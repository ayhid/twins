//! Runs a scan from parsed arguments and prints the report.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use anyhow::{Result, anyhow};
use twins_core::group::{self, Index, Keeper, Strategy};
use twins_core::human::parse_size;
use twins_core::report::{self, Meta};
use twins_core::scan;

use crate::cli::ScanArgs;

/// Runs `scan` or `report`. `force_json` is set by `report`.
pub fn scan(args: &ScanArgs, force_json: bool) -> Result<()> {
    let json = args.json || force_json;
    let roots = roots(&args.paths)?;
    let min_size = parse_size(&args.min_size)?;
    let opts = scan::Options::new(roots.clone())
        .min_size(min_size)
        .include_empty(args.include_empty)
        .include_library(args.include_library)
        .include_node_modules(args.include_node_modules)
        .include_remote(args.include_remote)
        .exclude(args.exclude.clone())
        .workers(args.jobs);

    let stderr = Mutex::new(std::io::stderr());
    let progress = Progress::new(!json && std::io::stderr().is_terminal());
    let idx = Index::new();
    let stats = scan::walk(
        &opts,
        |m| {
            idx.add(m);
            progress.tick("walk", None);
        },
        |path, err| skip(&stderr, args.verbose, path, err),
    )?;

    let errors = AtomicU64::new(stats.errors);
    let find_opts = group::Options::default()
        .workers(args.jobs)
        .verify(args.verify)
        .on_progress(Box::new(|p| progress.stage(p)))
        .on_error(Box::new(|path, err| {
            errors.fetch_add(1, Ordering::Relaxed);
            skip(&stderr, args.verbose, path, err);
        }));
    let groups = group::find(&idx, &find_opts)?;
    drop(find_opts);
    progress.finish();

    let keeper = Keeper::new(Strategy::default(), None);
    let actions = group::plan(&groups, &keeper);
    let meta = Meta {
        roots,
        files: stats.files,
        candidates: stats.candidates,
        strategy: keeper.strategy(),
        dry_run: false,
    };
    let r = report::build(&actions, &meta, SystemTime::now());
    let mut out = std::io::stdout().lock();
    if json {
        report::write_json(&mut out, &r)?;
    } else {
        report::write_text(&mut out, &r)?;
        let n = errors.load(Ordering::Relaxed);
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

fn skip(stderr: &Mutex<std::io::Stderr>, verbose: bool, path: &Path, err: &dyn std::fmt::Display) {
    if !verbose {
        return;
    }
    if let Ok(mut w) = stderr.lock() {
        let _ = writeln!(w, "skip {}: {err}", path.display());
    }
}

/// Single-line progress on stderr, only when it is a terminal.
struct Progress {
    enabled: bool,
    walked: AtomicU64,
}

impl Progress {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            walked: AtomicU64::new(0),
        }
    }

    fn tick(&self, phase: &str, total: Option<u64>) {
        let n = self.walked.fetch_add(1, Ordering::Relaxed) + 1;
        if self.enabled && n % 256 == 0 {
            Self::line(phase, n, total);
        }
    }

    fn stage(&self, p: group::Progress) {
        if self.enabled && (p.done % 16 == 0 || p.done == p.total) {
            Self::line(&p.stage.to_string(), p.done, Some(p.total));
        }
    }

    fn line(phase: &str, done: u64, total: Option<u64>) {
        let mut w = std::io::stderr().lock();
        let _ = match total {
            Some(t) => write!(w, "\r\x1b[K{phase}: {done}/{t}"),
            None => write!(w, "\r\x1b[K{phase}: {done} files"),
        };
        let _ = w.flush();
    }

    fn finish(&self) {
        if self.enabled {
            let mut w = std::io::stderr().lock();
            let _ = write!(w, "\r\x1b[K");
            let _ = w.flush();
        }
    }
}

/// Maps core errors to a usage-style message; kept for symmetry with the
/// Go implementation's exit codes.
pub fn exit_code(err: &anyhow::Error) -> i32 {
    if err.downcast_ref::<scan::ScanError>().is_some()
        || err
            .downcast_ref::<twins_core::human::ParseSizeError>()
            .is_some()
    {
        2
    } else {
        1
    }
}

/// Adds context to a fatal error before printing it.
pub fn describe(err: &anyhow::Error) -> String {
    err.chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

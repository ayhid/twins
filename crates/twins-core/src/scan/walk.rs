//! The walk itself: root validation, directory traversal, candidate
//! filtering.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;
use walkdir::WalkDir;

use super::rules::Rules;
use super::{Options, ScanError};
use crate::fsutil::{self, FileMeta, FsError};
use crate::safety;

/// Counters of one walk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Directories entered.
    pub dirs: u64,
    /// Regular files seen.
    pub files: u64,
    /// Files handed to the visitor.
    pub candidates: u64,
    /// Entries skipped by a rule or a filter.
    pub skipped: u64,
    /// Non-fatal errors reported through `on_error`.
    pub errors: u64,
}

#[derive(Default)]
struct Counters {
    dirs: AtomicU64,
    files: AtomicU64,
    candidates: AtomicU64,
    skipped: AtomicU64,
    errors: AtomicU64,
}

impl Counters {
    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> Stats {
        Stats {
            dirs: self.dirs.load(Ordering::Relaxed),
            files: self.files.load(Ordering::Relaxed),
            candidates: self.candidates.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

/// Error passed to `on_error` for one entry.
#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    /// The directory reader failed (permissions, vanished entry...).
    #[error(transparent)]
    Walk(#[from] walkdir::Error),
    /// `lstat` on a file failed.
    #[error(transparent)]
    Stat(#[from] FsError),
}

/// Walks every root and calls `visit` for each candidate regular file.
/// `visit` runs on worker threads and may be called concurrently;
/// `on_error` receives non-fatal per-entry problems.
///
/// # Errors
/// [`ScanError`] for invalid roots, bad globs or cancellation. Unreadable
/// entries are reported through `on_error` and counted in
/// [`Stats::errors`].
pub fn walk<V, E>(opts: &Options, visit: V, on_error: E) -> Result<Stats, ScanError>
where
    V: Fn(FileMeta) + Sync,
    E: Fn(&Path, &EntryError) + Sync,
{
    walk_observed(opts, visit, on_error, |_: &Stats| {})
}

/// Same as [`walk`], plus `on_progress`, which receives a snapshot of the
/// counters each time a regular file is counted while directories are
/// listed. It runs on the listing thread, never concurrently with itself.
///
/// # Errors
/// [`ScanError`] for invalid roots, bad globs or cancellation. Unreadable
/// entries are reported through `on_error` and counted in
/// [`Stats::errors`].
pub(crate) fn walk_observed<V, E, P>(
    opts: &Options,
    visit: V,
    on_error: E,
    on_progress: P,
) -> Result<Stats, ScanError>
where
    V: Fn(FileMeta) + Sync,
    E: Fn(&Path, &EntryError) + Sync,
    P: Fn(&Stats) + Sync,
{
    let roots = normalise_roots(opts)?;
    let rules = Rules::compile(opts)?;
    let counters = Counters::default();
    let report = |path: &Path, err: EntryError| {
        Counters::bump(&counters.errors);
        on_error(path, &err);
    };
    // Zero means one thread per CPU: the walk is stat-bound and does not
    // saturate an SSD, unlike hashing, which `group::find` caps at 8.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(opts.workers)
        .build()
        .map_err(|e| {
            ScanError::Io(FsError::Io {
                op: "thread pool",
                path: PathBuf::new(),
                source: std::io::Error::other(e),
            })
        })?;

    for root in &roots {
        if cancelled(opts) {
            return Err(ScanError::Cancelled);
        }
        let files = collect_files(root, opts, &rules, &counters, &report, &on_progress)?;
        pool.install(|| {
            files.par_iter().for_each(|path| {
                // Without this poll a cancel raised after the listing would
                // wait for every remaining file to be statted.
                if cancelled(opts) {
                    return;
                }
                handle_file(path, opts, &counters, &visit, &report);
            });
        });
        if cancelled(opts) {
            return Err(ScanError::Cancelled);
        }
    }
    Ok(counters.snapshot())
}

/// Whether the caller raised the walk's cancel flag.
fn cancelled(opts: &Options) -> bool {
    opts.cancel
        .as_ref()
        .is_some_and(|c| c.load(Ordering::Relaxed))
}

/// Lists the regular files of one root, applying directory and file rules.
fn collect_files(
    root: &Path,
    opts: &Options,
    rules: &Rules,
    counters: &Counters,
    report: &(impl Fn(&Path, EntryError) + Sync),
    on_progress: &impl Fn(&Stats),
) -> Result<Vec<PathBuf>, ScanError> {
    let mut files = Vec::new();
    let volumes = VolumeCache::default();
    let iter = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 || !e.file_type().is_dir() {
                return true;
            }
            let enter = !rules.skip_dir(e.path(), e.file_name())
                && (opts.include_remote || volumes.is_local(e));
            if !enter {
                Counters::bump(&counters.skipped);
            }
            enter
        });
    for entry in iter {
        if cancelled(opts) {
            return Err(ScanError::Cancelled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let path = e.path().map(Path::to_path_buf).unwrap_or_default();
                report(&path, e.into());
                continue;
            }
        };
        let ft = entry.file_type();
        if ft.is_dir() {
            Counters::bump(&counters.dirs);
        } else if ft.is_file() {
            Counters::bump(&counters.files);
            on_progress(&counters.snapshot());
            if rules.skip_file(entry.path(), entry.file_name()) {
                Counters::bump(&counters.skipped);
            } else {
                files.push(entry.into_path());
            }
        } else {
            Counters::bump(&counters.skipped); // symlink, socket, device...
        }
    }
    Ok(files)
}

/// Remembers whether each device is a local volume. A mount boundary
/// changes the device number, so one `statfs` per device is enough
/// instead of one per directory.
#[derive(Default)]
struct VolumeCache {
    by_dev: Mutex<HashMap<u64, bool>>,
}

impl VolumeCache {
    fn is_local(&self, entry: &walkdir::DirEntry) -> bool {
        let Ok(md) = entry.metadata() else {
            return false;
        };
        let mut map = self
            .by_dev
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *map.entry(md.dev())
            .or_insert_with(|| fsutil::is_local_volume(entry.path()).unwrap_or(false))
    }
}

fn handle_file(
    path: &Path,
    opts: &Options,
    counters: &Counters,
    visit: &(impl Fn(FileMeta) + Sync),
    report: &(impl Fn(&Path, EntryError) + Sync),
) {
    let meta = match fsutil::stat(path) {
        Ok(m) => m,
        Err(e) => {
            report(path, e.into());
            return;
        }
    };
    if !is_candidate(&meta, opts) {
        Counters::bump(&counters.skipped);
        return;
    }
    Counters::bump(&counters.candidates);
    visit(meta);
}

fn is_candidate(m: &FileMeta, opts: &Options) -> bool {
    if m.dataless() {
        return false;
    }
    if m.size() == 0 {
        return opts.include_empty;
    }
    m.size() >= opts.min_size
}

/// Makes roots absolute, validates them and drops any root nested inside
/// another so files are visited once. The result is exactly the list of
/// roots the walk descends, in walk order, each spelled as the walk names
/// its files; `pipeline` maps the in-dir keep directory onto it, so the
/// two must never diverge.
pub(crate) fn normalise_roots(opts: &Options) -> Result<Vec<PathBuf>, ScanError> {
    if opts.roots.is_empty() {
        return Err(ScanError::NoRoots);
    }
    let mut abs = Vec::with_capacity(opts.roots.len());
    for root in &opts.roots {
        let a = safety::absolutize(root).ok_or_else(|| ScanError::NotDirectory(root.clone()))?;
        validate_root(&a, opts.include_remote)?;
        abs.push(a);
    }
    abs.sort();
    let mut out: Vec<PathBuf> = Vec::new();
    for a in abs {
        if out.last().is_some_and(|last| a.starts_with(last)) {
            continue;
        }
        out.push(a);
    }
    Ok(out)
}

fn validate_root(abs: &Path, include_remote: bool) -> Result<(), ScanError> {
    if safety::is_protected(abs) {
        return Err(ScanError::ProtectedRoot(abs.to_path_buf()));
    }
    let md = std::fs::metadata(abs).map_err(|source| FsError::Io {
        op: "stat",
        path: abs.to_path_buf(),
        source,
    })?;
    if !md.is_dir() {
        return Err(ScanError::NotDirectory(abs.to_path_buf()));
    }
    if !include_remote && !fsutil::is_local_volume(abs)? {
        return Err(ScanError::RemoteRoot(abs.to_path_buf()));
    }
    Ok(())
}

//! The refinement pipeline: every bucket is split by a key computed once
//! per physical identity, and only sub-buckets that still hold two or more
//! physical files survive.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;

use super::{DirectHasher, Group, Hasher, Index, physical};
use crate::fsutil::{FileMeta, Identity};
use crate::hash::{self, HashError};

/// Hashing parallelism is capped: beyond this, SSDs saturate.
const MAX_WORKERS: usize = 8;

/// Stand-in cancel flag when none is configured; never raised.
static NEVER: AtomicBool = AtomicBool::new(false);

/// Pipeline step, for progress reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Head and tail fingerprint.
    Partial,
    /// Full content digest.
    Full,
    /// Byte-by-byte comparison.
    Verify,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Partial => "partial hash",
            Self::Full => "full hash",
            Self::Verify => "verify",
        })
    }
}

/// Snapshot of one stage's advancement.
///
/// Each stage begins with exactly one event where `done == 0`, sent from the
/// calling thread before any worker event of that stage, even when the stage
/// has nothing to do. Every later event of the stage has `done >= 1`, so a
/// `done == 0` event unambiguously marks a stage start. Within a stage,
/// `done` rises by exactly one per event, so the last event of a stage that
/// ran to completion is `done == total`. Events are never delivered
/// concurrently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Stage being run.
    pub stage: Stage,
    /// Items processed so far.
    pub done: u64,
    /// Items in the stage.
    pub total: u64,
}

/// Non-fatal problem with one file, reported through `on_error`.
#[derive(Debug, thiserror::Error)]
pub enum GroupError {
    /// The file could not be hashed or compared.
    #[error(transparent)]
    Hash(#[from] HashError),
    /// The file shares a digest with the group but differs byte-wise.
    #[error("{0}: content differs despite identical hash")]
    HashCollision(PathBuf),
}

/// Fatal error of [`find`].
#[derive(Debug, thiserror::Error)]
pub enum FindError {
    /// The run was cancelled through [`Options::cancel`].
    #[error("scan cancelled")]
    Cancelled,
    /// The worker pool could not be created.
    #[error("thread pool: {0}")]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}

type ProgressFn<'a> = Box<dyn Fn(Progress) + Send + Sync + 'a>;
type ErrorFn<'a> = Box<dyn Fn(&Path, &GroupError) + Send + Sync + 'a>;

/// Tunes [`find`]. The default uses [`DirectHasher`] and all CPUs (capped).
pub struct Options<'a> {
    hasher: Arc<dyn Hasher>,
    workers: usize,
    verify: bool,
    on_progress: Option<ProgressFn<'a>>,
    on_error: Option<ErrorFn<'a>>,
    cancel: Option<Arc<AtomicBool>>,
}

impl fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("workers", &self.workers)
            .field("verify", &self.verify)
            .finish_non_exhaustive()
    }
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            hasher: Arc::new(DirectHasher),
            workers: 0,
            verify: false,
            on_progress: None,
            on_error: None,
            cancel: None,
        }
    }
}

impl<'a> Options<'a> {
    /// Fingerprint implementation.
    #[must_use]
    pub fn hasher(self, hasher: Arc<dyn Hasher>) -> Self {
        Self { hasher, ..self }
    }

    /// Hashing parallelism. Zero means `min(cpus, 8)`.
    #[must_use]
    pub fn workers(self, workers: usize) -> Self {
        Self { workers, ..self }
    }

    /// Compare byte by byte after hashing.
    #[must_use]
    pub fn verify(self, verify: bool) -> Self {
        Self { verify, ..self }
    }

    /// Called from worker threads as each stage advances, never concurrently
    /// with itself.
    ///
    /// Each stage begins with exactly one call where `done == 0`, made from the
    /// calling thread before any worker event of that stage, even when the
    /// stage has nothing to do.
    ///
    /// While hashing, only the worker making the call waits for it: the
    /// others keep hashing and leave their counts to that worker, which
    /// reports them in order before it goes back to hashing. A slow
    /// callback therefore delays the progress it reports, not the hashing,
    /// although the stage still waits for its last call, and a callback
    /// that never returns stalls it. Verify reports from the thread doing
    /// the comparisons.
    #[must_use]
    pub fn on_progress(self, f: ProgressFn<'a>) -> Self {
        Self {
            on_progress: Some(f),
            ..self
        }
    }

    /// Called for every file dropped from the pipeline.
    #[must_use]
    pub fn on_error(self, f: ErrorFn<'a>) -> Self {
        Self {
            on_error: Some(f),
            ..self
        }
    }

    /// Flag polled between files and inside each full hash and byte
    /// comparison (before every 256 KiB chunk); setting it aborts with
    /// [`FindError::Cancelled`]. A file interrupted by the cancel is neither
    /// reported through `on_error` nor given a digest.
    #[must_use]
    pub fn cancel(self, cancel: Arc<AtomicBool>) -> Self {
        Self {
            cancel: Some(cancel),
            ..self
        }
    }

    fn effective_workers(&self) -> usize {
        if self.workers > 0 {
            return self.workers;
        }
        std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .min(MAX_WORKERS)
    }

    fn cancelled(&self) -> bool {
        self.cancel_flag().load(Ordering::Relaxed)
    }

    /// The configured cancel flag, or one that is never raised.
    fn cancel_flag(&self) -> &AtomicBool {
        self.cancel.as_deref().unwrap_or(&NEVER)
    }

    fn report(&self, path: &Path, err: &GroupError) {
        if let Some(f) = &self.on_error {
            f(path, err);
        }
    }

    fn progress(&self, stage: Stage, done: u64, total: u64) {
        if let Some(f) = &self.on_progress {
            f(Progress { stage, done, total });
        }
    }
}

/// Runs the pipeline over the index and returns duplicate groups sorted by
/// reclaimable space. Files that cannot be read are reported through
/// `on_error` and dropped.
///
/// # Errors
/// [`FindError::Cancelled`] when the cancel flag is raised.
pub fn find(idx: &Index, opts: &Options<'_>) -> Result<Vec<Group>, FindError> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(opts.effective_workers())
        .build()?;
    pool.install(|| find_in_pool(idx, opts))
}

fn find_in_pool(idx: &Index, opts: &Options<'_>) -> Result<Vec<Group>, FindError> {
    let buckets = idx.candidates();
    let partial = refine(buckets, opts, Stage::Partial, |m| opts.hasher.partial(m))?;
    let full = refine(
        partial.into_iter().map(|b| b.files).collect(),
        opts,
        Stage::Full,
        |m| opts.hasher.full_cancellable(m, opts.cancel_flag()),
    )?;
    let total = full.len() as u64;
    if opts.verify {
        opts.progress(Stage::Verify, 0, total);
    }
    let mut groups = Vec::with_capacity(full.len());
    for (i, b) in full.into_iter().enumerate() {
        if opts.cancelled() {
            return Err(FindError::Cancelled);
        }
        let files = if opts.verify {
            let kept = verify(b.files, opts);
            if opts.cancelled() {
                return Err(FindError::Cancelled);
            }
            opts.progress(Stage::Verify, i as u64 + 1, total);
            if physical(&kept) < 2 {
                continue;
            }
            kept
        } else {
            b.files
        };
        let size = files[0].size();
        groups.push(Group::new(size, b.key, files));
    }
    Ok(sort_groups(groups))
}

/// Files sharing the same key at a given stage.
struct Bucket<K> {
    key: K,
    files: Vec<FileMeta>,
}

fn refine<K, F>(
    buckets: Vec<Vec<FileMeta>>,
    opts: &Options<'_>,
    stage: Stage,
    key: F,
) -> Result<Vec<Bucket<K>>, FindError>
where
    K: Clone + Eq + Hash + Send + Sync,
    F: Fn(&FileMeta) -> Result<K, HashError> + Sync,
{
    let reps = representatives(&buckets);
    let keys = compute_keys(&reps, opts, stage, key)?;
    let mut out = Vec::new();
    for files in buckets {
        let mut by_key: HashMap<K, Vec<FileMeta>> = HashMap::new();
        let mut order = Vec::new();
        for f in files {
            let Some(k) = keys.get(&f.identity()) else {
                continue; // hashing failed, already reported
            };
            let entry = by_key.entry(k.clone()).or_default();
            if entry.is_empty() {
                order.push(k.clone());
            }
            entry.push(f);
        }
        for k in order {
            let sub = by_key.remove(&k).unwrap_or_default();
            if physical(&sub) >= 2 {
                out.push(Bucket { key: k, files: sub });
            }
        }
    }
    Ok(out)
}

/// One file per physical identity across all buckets.
fn representatives(buckets: &[Vec<FileMeta>]) -> Vec<FileMeta> {
    let mut seen = HashSet::new();
    buckets
        .iter()
        .flatten()
        .filter(|f| seen.insert(f.identity()))
        .cloned()
        .collect()
}

/// Hashes representatives in parallel on the installed pool.
fn compute_keys<K, F>(
    reps: &[FileMeta],
    opts: &Options<'_>,
    stage: Stage,
    key: F,
) -> Result<HashMap<Identity, K>, FindError>
where
    K: Send + Sync,
    F: Fn(&FileMeta) -> Result<K, HashError> + Sync,
{
    let total = reps.len() as u64;
    // Stage-start marker, sent before any worker can report.
    opts.progress(stage, 0, total);
    let ticker = Ticker::new(opts, stage, total);
    let results: Vec<Option<(Identity, Result<K, HashError>)>> = reps
        .par_iter()
        .map(|m| {
            if opts.cancelled() {
                return None;
            }
            let r = key(m);
            // A key interrupted by the cancel is dropped here, so it surfaces
            // as `FindError::Cancelled` and is never reported as unreadable.
            if opts.cancelled() {
                return None;
            }
            ticker.tick();
            Some((m.identity(), r))
        })
        .collect();
    let mut keys = HashMap::with_capacity(reps.len());
    for (m, r) in reps.iter().zip(results) {
        match r {
            None => return Err(FindError::Cancelled),
            Some((id, Ok(k))) => {
                keys.insert(id, k);
            }
            Some((_, Err(e))) => opts.report(m.path(), &GroupError::Hash(e)),
        }
    }
    Ok(keys)
}

/// Counts the files a stage has processed and reports every count once, in
/// order, without making workers wait for the progress callback.
///
/// A worker bumps `done`, then tries to claim the reporting slot. The
/// holder reports each count not yet reported, releases the slot and looks
/// at `done` again: a count bumped while it was reporting may have found
/// the slot taken, and that worker relies on the holder to report it. A
/// worker that finds the slot taken goes straight back to hashing. Every
/// access to `done` and `busy` is `SeqCst`, so a holder's look after its
/// release cannot miss a bump whose claim saw the slot taken. When the
/// parallel loop ends, every count up to `total` has been reported.
struct Ticker<'o, 'a> {
    opts: &'o Options<'a>,
    stage: Stage,
    total: u64,
    /// Files processed so far.
    done: AtomicU64,
    /// Last count reported; only touched by the slot holder.
    reported: AtomicU64,
    /// Whether a worker holds the reporting slot.
    busy: AtomicBool,
}

impl<'o, 'a> Ticker<'o, 'a> {
    fn new(opts: &'o Options<'a>, stage: Stage, total: u64) -> Self {
        Self {
            opts,
            stage,
            total,
            done: AtomicU64::new(0),
            reported: AtomicU64::new(0),
            busy: AtomicBool::new(false),
        }
    }

    /// Counts one more file and reports what is pending, unless another
    /// worker is already reporting.
    fn tick(&self) {
        self.done.fetch_add(1, Ordering::SeqCst);
        loop {
            if self
                .busy
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return; // the holder reports this count
            }
            let mut n = self.reported.load(Ordering::Relaxed);
            while n < self.done.load(Ordering::SeqCst) {
                n += 1;
                self.opts.progress(self.stage, n, self.total);
            }
            self.reported.store(n, Ordering::Relaxed);
            self.busy.store(false, Ordering::SeqCst);
            if self.done.load(Ordering::SeqCst) == n {
                return;
            }
        }
    }
}

/// Keeps only files byte-identical to the first one in the bucket.
///
/// Stops early, returning the files kept so far, once the cancel flag is
/// raised; the caller must check [`Options::cancelled`] afterwards.
fn verify(files: Vec<FileMeta>, opts: &Options<'_>) -> Vec<FileMeta> {
    let mut iter = files.into_iter();
    let Some(reference) = iter.next() else {
        return Vec::new();
    };
    let mut out = vec![reference.clone()];
    for f in iter {
        if opts.cancelled() {
            return out;
        }
        if f.identity() == reference.identity() {
            out.push(f);
            continue;
        }
        match hash::equal_cancellable(reference.path(), f.path(), opts.cancel_flag()) {
            Ok(true) => out.push(f),
            Ok(false) => opts.report(f.path(), &GroupError::HashCollision(f.path().to_path_buf())),
            Err(_) if opts.cancelled() => return out,
            Err(e) => opts.report(f.path(), &GroupError::Hash(e)),
        }
    }
    out
}

/// Orders groups by reclaimable space, then size, then first path.
fn sort_groups(mut groups: Vec<Group>) -> Vec<Group> {
    groups.sort_by(|a, b| {
        b.reclaimable()
            .cmp(&a.reclaimable())
            .then(b.size().cmp(&a.size()))
            .then_with(|| a.files()[0].path().cmp(b.files()[0].path()))
    });
    groups
}

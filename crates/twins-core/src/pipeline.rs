//! The scan pipeline as one call: walk, size grouping, hashing, keep plan
//! and report metadata. Shells build a [`ScanSpec`], follow the run through
//! an [`Observer`], stop it with a [`CancelToken`] and render the
//! [`ScanOutcome`]; they never wire the stages themselves.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::group::{self, Action, FindError, Group, GroupError, Index, Keeper, Strategy};
use crate::observe::{CancelToken, Event, Observer, Outcome, Stage};
use crate::report::{self, Meta, Report};
use crate::scan::{self, ScanError, Stats};

/// What kind of run produced a report. The report's dry-run flag is
/// derived from it and from nothing else. Phase 2 adds the real clean
/// modes to this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum RunMode {
    /// Read-only scan, as `twins scan` and `twins report` run.
    #[default]
    Scan,
    /// A clean that only simulates its actions.
    DryRun,
}

impl RunMode {
    /// Whether a report produced in this mode is a dry run.
    #[must_use]
    pub fn is_dry_run(self) -> bool {
        matches!(self, Self::DryRun)
    }
}

/// Everything a scan needs. Every setter returns a new value.
#[derive(Debug, Clone)]
pub struct ScanSpec {
    walk: scan::Options,
    hash_workers: usize,
    verify: bool,
    strategy: Strategy,
    keep_dir: Option<PathBuf>,
    mode: RunMode,
}

impl ScanSpec {
    /// A scan with the given walk options, default hashing parallelism, no
    /// verify, the default keep strategy and [`RunMode::Scan`].
    #[must_use]
    pub fn new(walk: scan::Options) -> Self {
        Self {
            walk,
            hash_workers: 0,
            verify: false,
            strategy: Strategy::default(),
            keep_dir: None,
            mode: RunMode::Scan,
        }
    }

    /// Hashing parallelism. Zero means `min(cpus, 8)`.
    #[must_use]
    pub fn hash_workers(self, hash_workers: usize) -> Self {
        Self {
            hash_workers,
            ..self
        }
    }

    /// Compare byte by byte after hashing.
    #[must_use]
    pub fn verify(self, verify: bool) -> Self {
        Self { verify, ..self }
    }

    /// Which copy of each group survives.
    #[must_use]
    pub fn strategy(self, strategy: Strategy) -> Self {
        Self { strategy, ..self }
    }

    /// Directory preferred by [`Strategy::InDir`].
    #[must_use]
    pub fn keep_dir(self, keep_dir: PathBuf) -> Self {
        Self {
            keep_dir: Some(keep_dir),
            ..self
        }
    }

    /// Kind of run.
    #[must_use]
    pub fn mode(self, mode: RunMode) -> Self {
        Self { mode, ..self }
    }

    /// Walk options, roots included.
    #[must_use]
    pub fn walk_options(&self) -> &scan::Options {
        &self.walk
    }

    /// Kind of run.
    #[must_use]
    pub fn run_mode(&self) -> RunMode {
        self.mode
    }

    /// Stages in this run: 5 with verify, 4 otherwise.
    #[must_use]
    pub fn steps(&self) -> u8 {
        if self.verify { 5 } else { 4 }
    }
}

/// Result of a completed scan.
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    actions: Vec<Action>,
    meta: Meta,
    stats: Stats,
    errors: u64,
}

impl ScanOutcome {
    /// One keep/remove decision per duplicate group.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Scan-level information carried into the report.
    #[must_use]
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Counters of the walk.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Files that could not be read during the walk or the hashing.
    #[must_use]
    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// Builds the schema-v1 report stamped with `now`.
    #[must_use]
    pub fn report(&self, now: SystemTime) -> Report {
        report::build(&self.actions, &self.meta, now)
    }
}

/// Fatal error of [`scan`].
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Invalid roots, bad globs or I/O on a root. Never holds
    /// [`ScanError::Cancelled`].
    #[error(transparent)]
    Scan(ScanError),
    /// The hashing pool could not be built. Never holds
    /// [`FindError::Cancelled`].
    #[error(transparent)]
    Find(FindError),
    /// The cancel token was raised.
    #[error("scan cancelled")]
    Cancelled,
}

impl PipelineError {
    /// Whether the run stopped because it was cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

impl From<ScanError> for PipelineError {
    fn from(err: ScanError) -> Self {
        match err {
            ScanError::Cancelled => Self::Cancelled,
            other => Self::Scan(other),
        }
    }
}

impl From<FindError> for PipelineError {
    fn from(err: FindError) -> Self {
        match err {
            FindError::Cancelled => Self::Cancelled,
            other @ FindError::ThreadPool(_) => Self::Find(other),
        }
    }
}

/// Runs the whole scan pipeline and plans which copy of each duplicate
/// group survives. Events go to `observer`; the last one is always
/// [`Event::Finished`], whatever the outcome. Every stage opens with one
/// [`Event::StageStarted`], in the order walk, size grouping, partial hash,
/// full hash and, with verify, verify, even when a stage has nothing to do;
/// a stage's [`Event::Progress`] events follow its start. Unreadable files are
/// reported as [`Event::FileSkipped`] and counted in
/// [`ScanOutcome::errors`].
///
/// # Errors
/// [`PipelineError::Cancelled`] when `cancel` is raised before or during
/// the run, [`PipelineError::Scan`] for invalid roots or exclusion globs,
/// [`PipelineError::Find`] when the hashing pool cannot be built.
pub fn scan(
    spec: &ScanSpec,
    observer: &dyn Observer,
    cancel: &CancelToken,
) -> Result<ScanOutcome, PipelineError> {
    let result = run(spec, observer, cancel);
    let outcome = match &result {
        Ok(_) => Outcome::Completed,
        Err(e) if e.is_cancelled() => Outcome::Cancelled,
        Err(_) => Outcome::Failed,
    };
    observer.on_event(&Event::Finished { outcome });
    result
}

fn run(
    spec: &ScanSpec,
    observer: &dyn Observer,
    cancel: &CancelToken,
) -> Result<ScanOutcome, PipelineError> {
    check(cancel)?;
    let index = Index::new();
    let stats = walk_stage(spec, observer, cancel, &index)?;
    // `find` returns Ok on an empty index even when cancelled, so a cancel
    // landing between the stages must be caught here.
    check(cancel)?;
    started(observer, spec, Stage::SizeGrouping);
    observer.on_event(&Event::Progress {
        stage: Stage::SizeGrouping,
        done: stats.candidates,
        total: Some(stats.candidates),
    });
    let (groups, find_errors) = hash_stages(spec, observer, cancel, &index)?;
    check(cancel)?;
    Ok(plan(spec, &groups, stats, stats.errors + find_errors))
}

fn check(cancel: &CancelToken) -> Result<(), PipelineError> {
    if cancel.is_cancelled() {
        Err(PipelineError::Cancelled)
    } else {
        Ok(())
    }
}

fn started(observer: &dyn Observer, spec: &ScanSpec, stage: Stage) {
    observer.on_event(&Event::StageStarted {
        stage,
        step: stage.step(),
        steps: spec.steps(),
    });
}

fn skipped(observer: &dyn Observer, path: &Path, reason: &dyn std::fmt::Display) {
    observer.on_event(&Event::FileSkipped {
        path: path.to_string_lossy().into_owned(),
        reason: reason.to_string(),
    });
}

fn walk_stage(
    spec: &ScanSpec,
    observer: &dyn Observer,
    cancel: &CancelToken,
    index: &Index,
) -> Result<Stats, PipelineError> {
    started(observer, spec, Stage::Walk);
    let opts = spec.walk_options().clone().cancel(cancel.flag());
    // One Progress per file seen while listing directories, the slow part
    // of the walk; observers such as `Throttle` rate-limit it.
    let stats = scan::walk_observed(
        &opts,
        |m| index.add(m),
        |path, err| skipped(observer, path, err),
        |s: &Stats| {
            observer.on_event(&Event::Progress {
                stage: Stage::Walk,
                done: s.files,
                total: None,
            });
        },
    )?;
    Ok(stats)
}

/// Partial hash, full hash and optional verify; returns the groups and the
/// number of files dropped along the way.
fn hash_stages(
    spec: &ScanSpec,
    observer: &dyn Observer,
    cancel: &CancelToken,
    index: &Index,
) -> Result<(Vec<Group>, u64), PipelineError> {
    let errors = AtomicU64::new(0);
    let opts = group::Options::default()
        .workers(spec.hash_workers)
        .verify(spec.verify)
        .cancel(cancel.flag())
        .on_progress(Box::new(|p| {
            let stage = Stage::from(p.stage);
            // `find` opens every stage with one `done == 0` marker.
            if p.done == 0 {
                started(observer, spec, stage);
            } else {
                observer.on_event(&Event::Progress {
                    stage,
                    done: p.done,
                    total: Some(p.total),
                });
            }
        }))
        .on_error(Box::new(|path: &Path, err: &GroupError| {
            errors.fetch_add(1, Ordering::Relaxed);
            skipped(observer, path, err);
        }));
    let groups = group::find(index, &opts)?;
    drop(opts);
    Ok((groups, errors.into_inner()))
}

fn plan(spec: &ScanSpec, groups: &[Group], stats: Stats, errors: u64) -> ScanOutcome {
    let keeper = Keeper::new(spec.strategy, spec.keep_dir.clone());
    let actions = group::plan(groups, &keeper);
    let meta = Meta {
        // The roots as the caller gave them, not the walk's absolute ones.
        roots: spec.walk_options().roots().to_vec(),
        files: stats.files,
        candidates: stats.candidates,
        strategy: keeper.strategy(),
        dry_run: spec.mode.is_dry_run(),
    };
    ScanOutcome {
        actions,
        meta,
        stats,
        errors,
    }
}

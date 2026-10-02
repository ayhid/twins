//! Progress and cancellation contract between the pipeline and its shells.
//!
//! The pipeline reports what it is doing through an [`Observer`] and stops
//! when a [`CancelToken`] is raised. Shells (the CLI today, the app and the
//! agent later) render the events however they like; core never prints and
//! never installs signal handlers.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::group;

/// Shared cancellation flag. Clones share the same flag, so raising one
/// raises them all.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that is not raised.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Raises the token. Running stages stop at their next check.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the token has been raised.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// The underlying flag, for signal handlers that can only set an
    /// `AtomicBool` and for the walk and grouping `cancel` options.
    #[must_use]
    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }
}

/// Pipeline stage, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Stage {
    /// Directory traversal and `lstat` of every file.
    Walk,
    /// Bucketing candidates by size.
    SizeGrouping,
    /// Head and tail fingerprint of same-size files.
    PartialHash,
    /// Full content digest.
    FullHash,
    /// Byte-by-byte comparison, only with verify on.
    Verify,
}

impl Stage {
    /// 1-based position of the stage in a run.
    #[must_use]
    pub const fn step(self) -> u8 {
        match self {
            Self::Walk => 1,
            Self::SizeGrouping => 2,
            Self::PartialHash => 3,
            Self::FullHash => 4,
            Self::Verify => 5,
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Walk => "walk",
            Self::SizeGrouping => "size grouping",
            Self::PartialHash => "partial hash",
            Self::FullHash => "full hash",
            Self::Verify => "verify",
        })
    }
}

impl From<group::Stage> for Stage {
    fn from(stage: group::Stage) -> Self {
        match stage {
            group::Stage::Partial => Self::PartialHash,
            group::Stage::Full => Self::FullHash,
            group::Stage::Verify => Self::Verify,
        }
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// The run produced its result.
    Completed,
    /// The cancel token was raised.
    Cancelled,
    /// A fatal error stopped the run.
    Failed,
}

/// Something the pipeline reports while it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[non_exhaustive]
pub enum Event {
    /// A stage begins. `step` is 1-based and `steps` is the number of
    /// stages in this run (4, or 5 with verify). Never throttled.
    StageStarted {
        /// Stage that begins.
        stage: Stage,
        /// Its 1-based position.
        step: u8,
        /// Stages in this run.
        steps: u8,
    },
    /// Advancement inside a stage. `total` is `None` when unknown, as while
    /// walking. The only event an observer may throttle.
    Progress {
        /// Stage being run.
        stage: Stage,
        /// Items processed so far.
        done: u64,
        /// Items in the stage, when known.
        total: Option<u64>,
    },
    /// A file was dropped because it could not be read or compared. Never
    /// throttled.
    FileSkipped {
        /// Path of the file, lossily converted to UTF-8.
        path: String,
        /// Why it was dropped.
        reason: String,
    },
    /// The run ended, whatever the outcome. Always the last event, never
    /// throttled.
    Finished {
        /// How the run ended.
        outcome: Outcome,
    },
}

/// Receives pipeline events.
///
/// `on_event` is called from the pipeline's worker threads. Only
/// [`Event::FileSkipped`] may be delivered concurrently, from several
/// workers at once; every other event is delivered by one thread at a
/// time, and a stage's [`Event::Progress`] events arrive in order, with
/// `done` never going down.
///
/// While hashing, only the worker delivering a `Progress` waits for
/// `on_event`; the other workers keep hashing, so a slow observer delays
/// what it displays rather than the scan. It should still be cheap: each
/// stage waits for its last event, the walk and verify report from the
/// thread doing the work, and an observer that never returns stalls the
/// run.
pub trait Observer: Send + Sync {
    /// Called for every event.
    fn on_event(&self, event: &Event);
}

/// Observer that ignores every event.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopObserver;

impl Observer for NoopObserver {
    fn on_event(&self, _event: &Event) {}
}

/// Rate-limiting wrapper around another observer.
///
/// Forwards a [`Event::Progress`] only when at least `interval` has passed
/// since the last one it forwarded, when it is the first `Progress` after a
/// [`Event::StageStarted`], or when it completes its stage
/// (`total == Some(done)`). Every other `Progress` is dropped, and so is a
/// `Progress` whose `done` is not above that of the last one forwarded for
/// the same stage: the inner observer never sees a stage go backwards, so a
/// late `done == 5` cannot follow the stage's final event.
///
/// It never drops [`Event::StageStarted`], [`Event::FileSkipped`] or
/// [`Event::Finished`]. `Progress` and `StageStarted` are admitted and
/// forwarded under one internal lock, so concurrent callers are
/// serialized and the inner observer receives these events one at a time;
/// a slow inner observer makes other threads' `Progress` wait. The pipeline
/// delivers them one thread at a time anyway (see [`Observer`]), so there
/// the lock is never contended. `FileSkipped` and `Finished` bypass it.
#[derive(Debug)]
pub struct Throttle<O> {
    inner: O,
    interval: Duration,
    gate: Mutex<Gate>,
}

/// What [`Throttle`] remembers of the `Progress` it forwarded.
#[derive(Debug, Default)]
struct Gate {
    /// When the last `Progress` was forwarded, or `None` when the next one
    /// must pass.
    last: Option<Instant>,
    /// Stage and `done` of the last `Progress` forwarded since the last
    /// `StageStarted`.
    newest: Option<(Stage, u64)>,
}

impl Gate {
    /// Whether a `Progress` may pass at `now`; records it when it does.
    fn admit(
        &mut self,
        stage: Stage,
        done: u64,
        total: Option<u64>,
        interval: Duration,
        now: Instant,
    ) -> bool {
        if let Some((s, d)) = self.newest
            && s == stage
            && done <= d
        {
            return false; // stale or repeated
        }
        let due = self
            .last
            .is_none_or(|t| now.saturating_duration_since(t) >= interval);
        if !due && total != Some(done) {
            return false;
        }
        self.last = Some(now);
        self.newest = Some((stage, done));
        true
    }
}

impl<O> Throttle<O> {
    /// Wraps `inner`, forwarding at most one `Progress` per `interval`
    /// besides the first and the final one of each stage. A zero interval
    /// forwards every `Progress` that moves its stage forward.
    #[must_use]
    pub fn new(inner: O, interval: Duration) -> Self {
        Self {
            inner,
            interval,
            gate: Mutex::new(Gate::default()),
        }
    }

    /// The wrapped observer.
    #[must_use]
    pub fn inner(&self) -> &O {
        &self.inner
    }

    /// Unwraps the observer.
    pub fn into_inner(self) -> O {
        self.inner
    }

    /// The gate, even if an inner observer panicked while holding it.
    fn gate(&self) -> MutexGuard<'_, Gate> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<O: Observer> Observer for Throttle<O> {
    fn on_event(&self, event: &Event) {
        match event {
            Event::StageStarted { .. } => {
                let mut gate = self.gate();
                *gate = Gate::default();
                self.inner.on_event(event);
            }
            Event::Progress { stage, done, total } => {
                let mut gate = self.gate();
                if gate.admit(*stage, *done, *total, self.interval, Instant::now()) {
                    self.inner.on_event(event);
                }
            }
            _ => self.inner.on_event(event),
        }
    }
}

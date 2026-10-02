//! Progress and cancellation contract between the pipeline and its shells.
//!
//! The pipeline reports what it is doing through an [`Observer`] and stops
//! when a [`CancelToken`] is raised. Shells (the CLI today, the app and the
//! agent later) render the events however they like; core never prints and
//! never installs signal handlers.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
/// `on_event` is called from worker threads, possibly concurrently, so it
/// must be cheap and must not block for long.
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
/// (`total == Some(done)`). Every other `Progress` is dropped.
///
/// It never drops or delays [`Event::StageStarted`], [`Event::FileSkipped`]
/// or [`Event::Finished`], and never reorders the events it forwards. The
/// check is lock-free (one atomic compare-and-swap), so hashing threads can
/// call it for every file.
#[derive(Debug)]
pub struct Throttle<O> {
    inner: O,
    interval: Duration,
    base: Instant,
    /// Nanoseconds since `base` of the last forwarded `Progress`, or
    /// [`OPEN`] when the next `Progress` must pass.
    last: AtomicU64,
}

/// Sentinel for [`Throttle::last`]: the next `Progress` passes.
const OPEN: u64 = u64::MAX;

impl<O> Throttle<O> {
    /// Wraps `inner`, forwarding at most one `Progress` per `interval`
    /// besides the first and the final one of each stage. A zero interval
    /// forwards everything.
    #[must_use]
    pub fn new(inner: O, interval: Duration) -> Self {
        Self {
            inner,
            interval,
            base: Instant::now(),
            last: AtomicU64::new(OPEN),
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

    /// Nanoseconds elapsed since `base`, saturating at `u64::MAX - 1` so it
    /// never collides with [`OPEN`].
    fn now(&self) -> u64 {
        u64::try_from(self.base.elapsed().as_nanos())
            .unwrap_or(u64::MAX)
            .min(OPEN - 1)
    }

    /// Whether a non-final `Progress` may pass now. Claims the slot with a
    /// compare-and-swap so concurrent callers cannot both pass.
    fn admit(&self) -> bool {
        let now = self.now();
        let prev = self.last.load(Ordering::Relaxed);
        let interval = u64::try_from(self.interval.as_nanos()).unwrap_or(u64::MAX);
        let due = prev == OPEN || now.saturating_sub(prev) >= interval;
        due && self
            .last
            .compare_exchange(prev, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }
}

impl<O: Observer> Observer for Throttle<O> {
    fn on_event(&self, event: &Event) {
        match event {
            Event::StageStarted { .. } => {
                self.last.store(OPEN, Ordering::Relaxed);
            }
            Event::Progress { done, total, .. } => {
                if *total == Some(*done) {
                    self.last.store(self.now(), Ordering::Relaxed);
                } else if !self.admit() {
                    return;
                }
            }
            _ => {}
        }
        self.inner.on_event(event);
    }
}

//! Progress and cancellation contract between the pipeline and its shells.
//!
//! The pipeline reports what it is doing through an [`Observer`] and stops
//! when a [`CancelToken`] is raised. Shells (the CLI today, the app and the
//! agent later) render the events however they like; core never prints and
//! never installs signal handlers.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

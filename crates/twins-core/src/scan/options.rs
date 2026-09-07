//! Walk configuration, built immutably.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Default minimum size of a candidate: 1 MiB.
pub const DEFAULT_MIN_SIZE: u64 = 1024 * 1024;

/// Configuration of one walk. Every setter returns a new value.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // independent opt-in switches
pub struct Options {
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) min_size: u64,
    pub(crate) include_empty: bool,
    pub(crate) include_library: bool,
    pub(crate) include_node_modules: bool,
    pub(crate) include_remote: bool,
    pub(crate) exclude: Vec<String>,
    pub(crate) workers: usize,
    pub(crate) home: Option<PathBuf>,
    pub(crate) cancel: Option<Arc<AtomicBool>>,
}

impl Options {
    /// Walk the given roots with default rules.
    #[must_use]
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            min_size: DEFAULT_MIN_SIZE,
            include_empty: false,
            include_library: false,
            include_node_modules: false,
            include_remote: false,
            exclude: Vec::new(),
            workers: 0,
            home: None,
            cancel: None,
        }
    }

    /// Roots to walk.
    #[must_use]
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Smallest file considered, in bytes. Empty files are governed by
    /// [`Options::include_empty`] regardless of this value.
    #[must_use]
    pub fn min_size(self, min_size: u64) -> Self {
        Self { min_size, ..self }
    }

    /// Consider empty files.
    #[must_use]
    pub fn include_empty(self, include_empty: bool) -> Self {
        Self {
            include_empty,
            ..self
        }
    }

    /// Descend into the user's `~/Library`.
    #[must_use]
    pub fn include_library(self, include_library: bool) -> Self {
        Self {
            include_library,
            ..self
        }
    }

    /// Descend into `node_modules` directories.
    #[must_use]
    pub fn include_node_modules(self, include_node_modules: bool) -> Self {
        Self {
            include_node_modules,
            ..self
        }
    }

    /// Accept roots and directories on network volumes.
    #[must_use]
    pub fn include_remote(self, include_remote: bool) -> Self {
        Self {
            include_remote,
            ..self
        }
    }

    /// Glob patterns to skip. A pattern without `/` is matched against the
    /// base name, one with `/` against the full absolute path.
    #[must_use]
    pub fn exclude(self, exclude: Vec<String>) -> Self {
        Self { exclude, ..self }
    }

    /// Parallelism for stat calls. Zero means one per CPU.
    #[must_use]
    pub fn workers(self, workers: usize) -> Self {
        Self { workers, ..self }
    }

    /// Home directory used to locate `~/Library`. Defaults to `$HOME`.
    #[must_use]
    pub fn home(self, home: PathBuf) -> Self {
        Self {
            home: Some(home),
            ..self
        }
    }

    /// Flag polled during the walk; setting it aborts with
    /// [`super::ScanError::Cancelled`].
    #[must_use]
    pub fn cancel(self, cancel: Arc<AtomicBool>) -> Self {
        Self {
            cancel: Some(cancel),
            ..self
        }
    }

    pub(crate) fn home_dir(&self) -> Option<PathBuf> {
        self.home
            .clone()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    }
}

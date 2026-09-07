//! Candidate files collected by size.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::physical;
use crate::fsutil::FileMeta;

/// Collects candidates by size. The only mutable, shared structure of the
/// pipeline; safe for concurrent [`Index::add`].
#[derive(Debug, Default)]
pub struct Index {
    by_size: Mutex<HashMap<u64, Vec<FileMeta>>>,
}

impl Index {
    /// Empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A poisoned lock only means another thread panicked mid-push; the map
    /// itself is still consistent, so recover it rather than propagate.
    fn map(&self) -> MutexGuard<'_, HashMap<u64, Vec<FileMeta>>> {
        self.by_size.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records a candidate.
    pub fn add(&self, m: FileMeta) {
        self.map().entry(m.size()).or_default().push(m);
    }

    /// Number of files added.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map().values().map(Vec::len).sum()
    }

    /// Whether nothing was added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every size bucket holding at least two distinct physical files.
    /// Buckets are ordered by size, descending, so results are stable.
    #[must_use]
    pub fn candidates(&self) -> Vec<Vec<FileMeta>> {
        let map = self.map();
        let mut sizes: Vec<&u64> = map.keys().collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        sizes
            .into_iter()
            .map(|s| &map[s])
            .filter(|files| physical(files) >= 2)
            .cloned()
            .collect()
    }
}

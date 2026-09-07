//! Turns candidate files into groups of exact duplicates through a
//! size → partial hash → full hash pipeline, then decides which copy to
//! keep.

mod find;
mod hasher;
mod index;
mod keep;

pub use find::{FindError, GroupError, Options, Progress, Stage, find};
pub use hasher::{DirectHasher, Hasher};
pub use index::Index;
pub use keep::{Action, Keeper, Strategy, UnknownStrategy, plan, total_reclaimable};

use std::collections::HashSet;

use crate::fsutil::{FileMeta, Identity};
use crate::hash::Digest;

/// Immutable set of files with identical content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    size: u64,
    digest: Digest,
    files: Vec<FileMeta>,
}

impl Group {
    /// Builds a group; files are sorted by path.
    #[must_use]
    pub fn new(size: u64, digest: Digest, files: Vec<FileMeta>) -> Self {
        let mut files = files;
        files.sort_by(|a, b| a.path().cmp(b.path()));
        Self {
            size,
            digest,
            files,
        }
    }

    /// Size of every member, in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Content digest shared by every member.
    #[must_use]
    pub fn digest(&self) -> Digest {
        self.digest
    }

    /// Members, sorted by path.
    #[must_use]
    pub fn files(&self) -> &[FileMeta] {
        &self.files
    }

    /// Distinct on-disk copies: hardlinks count once.
    #[must_use]
    pub fn physical(&self) -> usize {
        physical(&self.files)
    }

    /// Space freed by keeping a single physical copy.
    #[must_use]
    pub fn reclaimable(&self) -> u64 {
        self.size * (self.physical().saturating_sub(1)) as u64
    }
}

/// Counts distinct `(device, inode)` identities.
fn physical(files: &[FileMeta]) -> usize {
    files
        .iter()
        .map(FileMeta::identity)
        .collect::<HashSet<Identity>>()
        .len()
}

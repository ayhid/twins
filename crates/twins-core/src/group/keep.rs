//! Which copy of a group survives.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::Group;
use crate::fsutil::FileMeta;

/// Decides which copy of a group survives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strategy {
    /// Keep the copy with the earliest modification time.
    #[default]
    Oldest,
    /// Keep the copy with the latest modification time.
    Newest,
    /// Keep the copy with the fewest path components, then the shortest path.
    ShortestPath,
    /// Keep a copy under a chosen directory, falling back to oldest.
    InDir,
}

impl Strategy {
    /// Every strategy, in display order.
    pub const ALL: [Self; 4] = [Self::Oldest, Self::Newest, Self::ShortestPath, Self::InDir];

    /// Name accepted on the command line and in config files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Oldest => "oldest",
            Self::Newest => "newest",
            Self::ShortestPath => "shortest-path",
            Self::InDir => "in-dir",
        }
    }
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Error returned when a strategy name is not recognised.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown keep strategy {0:?} (want one of oldest, newest, shortest-path, in-dir)")]
pub struct UnknownStrategy(String);

impl FromStr for Strategy {
    type Err = UnknownStrategy;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .ok_or_else(|| UnknownStrategy(s.to_owned()))
    }
}

/// Applies a [`Strategy`]. `dir` is only used by [`Strategy::InDir`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keeper {
    strategy: Strategy,
    dir: Option<PathBuf>,
}

impl Keeper {
    /// Keeper for a strategy; `dir` is cleaned lexically.
    #[must_use]
    pub fn new(strategy: Strategy, dir: Option<PathBuf>) -> Self {
        Self {
            strategy,
            dir: dir.map(|d| d.components().collect()),
        }
    }

    /// Strategy applied.
    #[must_use]
    pub fn strategy(&self) -> Strategy {
        self.strategy
    }

    /// Returns the file to keep and the files to remove. Hardlinks of the
    /// kept file are never removed: they already share its data.
    #[must_use]
    pub fn choose<'g>(&self, g: &'g Group) -> (&'g FileMeta, Vec<&'g FileMeta>) {
        let keep = self.pick(g.files());
        let remove = g
            .files()
            .iter()
            .filter(|f| f.identity() != keep.identity())
            .collect();
        (keep, remove)
    }

    /// The file to keep among `files`, which must be non-empty.
    ///
    /// # Panics
    /// When `files` is empty.
    #[must_use]
    pub fn pick<'f>(&self, files: &'f [FileMeta]) -> &'f FileMeta {
        files
            .iter()
            .min_by(|a, b| self.order(a, b))
            .expect("non-empty group")
    }

    /// Total order whose minimum is the file to keep.
    fn order(&self, a: &FileMeta, b: &FileMeta) -> Ordering {
        let primary = match self.strategy {
            Strategy::Oldest => a.mtime().cmp(&b.mtime()),
            Strategy::Newest => b.mtime().cmp(&a.mtime()),
            Strategy::ShortestPath => depth(a.path())
                .cmp(&depth(b.path()))
                .then(path_len(a.path()).cmp(&path_len(b.path()))),
            Strategy::InDir => self
                .in_dir(b.path())
                .cmp(&self.in_dir(a.path()))
                .then(a.mtime().cmp(&b.mtime())),
        };
        primary
            .then_with(|| depth(a.path()).cmp(&depth(b.path())))
            .then_with(|| a.path().cmp(b.path()))
    }

    fn in_dir(&self, path: &Path) -> bool {
        self.dir.as_deref().is_some_and(|d| path.starts_with(d))
    }
}

fn depth(path: &Path) -> usize {
    path.components().count()
}

fn path_len(path: &Path) -> usize {
    path.as_os_str().len()
}

/// Resolved decision for one group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    group: Group,
    keep: FileMeta,
    remove: Vec<FileMeta>,
}

impl Action {
    /// The group decided on.
    #[must_use]
    pub fn group(&self) -> &Group {
        &self.group
    }

    /// The surviving file.
    #[must_use]
    pub fn keep(&self) -> &FileMeta {
        &self.keep
    }

    /// Files to remove.
    #[must_use]
    pub fn remove(&self) -> &[FileMeta] {
        &self.remove
    }

    /// Space actually freed by executing the action: one size per physical
    /// identity removed that is not the kept file's.
    #[must_use]
    pub fn reclaimable(&self) -> u64 {
        let mut seen = HashSet::from([self.keep.identity()]);
        self.remove
            .iter()
            .filter(|f| seen.insert(f.identity()))
            .count() as u64
            * self.group.size()
    }
}

/// Applies a keeper to every group.
#[must_use]
pub fn plan(groups: &[Group], keeper: &Keeper) -> Vec<Action> {
    groups
        .iter()
        .map(|g| {
            let (keep, remove) = keeper.choose(g);
            Action {
                group: g.clone(),
                keep: keep.clone(),
                remove: remove.into_iter().cloned().collect(),
            }
        })
        .collect()
}

/// Sums the space freed by a plan.
#[must_use]
pub fn total_reclaimable(actions: &[Action]) -> u64 {
    actions.iter().map(Action::reclaimable).sum()
}

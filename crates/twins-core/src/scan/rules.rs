//! Pre-compiled exclusion rules for one walk.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use super::{Options, ScanError};
use crate::{fsutil, safety};

/// Directory names skipped at any depth.
const ALWAYS_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".Trash",
    ".Trashes",
    "Caches",
    ".cache",
    "Backups.backupdb",
    ".Spotlight-V100",
    ".fseventsd",
    ".DocumentRevisions-V100",
    ".TemporaryItems",
    ".MobileBackups",
    "__pycache__",
];

const NODE_MODULES: &str = "node_modules";

/// Immutable rule set compiled from [`Options`].
#[derive(Debug)]
pub(crate) struct Rules {
    include_library: bool,
    include_node_modules: bool,
    home: Option<PathBuf>,
    base_globs: GlobSet,
    path_globs: GlobSet,
}

impl Rules {
    pub(crate) fn compile(opts: &Options) -> Result<Self, ScanError> {
        let mut base = GlobSetBuilder::new();
        let mut path = GlobSetBuilder::new();
        for pattern in &opts.exclude {
            let glob = Glob::new(pattern).map_err(|source| ScanError::Glob {
                pattern: pattern.clone(),
                source,
            })?;
            if pattern.contains('/') {
                path.add(glob);
            } else {
                base.add(glob);
            }
        }
        let build = |b: GlobSetBuilder| {
            b.build().map_err(|source| ScanError::Glob {
                pattern: String::new(),
                source,
            })
        };
        Ok(Self {
            include_library: opts.include_library,
            include_node_modules: opts.include_node_modules,
            home: opts.home_dir(),
            base_globs: build(base)?,
            path_globs: build(path)?,
        })
    }

    /// Whether a directory must not be descended into.
    pub(crate) fn skip_dir(&self, path: &Path, name: &OsStr) -> bool {
        if name
            .to_str()
            .is_some_and(|n| ALWAYS_EXCLUDED_DIRS.contains(&n))
        {
            return true;
        }
        if name == NODE_MODULES && !self.include_node_modules {
            return true;
        }
        if fsutil::is_bundle(name) || safety::is_protected(path) {
            return true;
        }
        if !self.include_library
            && self
                .home
                .as_deref()
                .is_some_and(|home| safety::is_user_library(home, path))
        {
            return true;
        }
        self.matches_glob(path, name)
    }

    /// Whether a file is excluded by user globs.
    pub(crate) fn skip_file(&self, path: &Path, name: &OsStr) -> bool {
        self.matches_glob(path, name)
    }

    fn matches_glob(&self, path: &Path, name: &OsStr) -> bool {
        self.base_globs.is_match(Path::new(name)) || self.path_globs.is_match(path)
    }
}

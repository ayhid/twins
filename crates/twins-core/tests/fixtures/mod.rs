//! Shared fixture builder: a temporary tree described by (relative path,
//! content) pairs.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// One mebibyte, the default minimum size.
pub const MIB: usize = 1024 * 1024;

/// Temporary tree rooted at a fresh directory.
pub struct Tree {
    dir: tempfile::TempDir,
}

impl Tree {
    /// Creates every (path, content) pair, making parent directories.
    pub fn build(files: &[(&str, &[u8])]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let p = dir.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
        }
        Self { dir }
    }

    /// Root of the tree.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Absolute path of a relative entry.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Absolute paths of the given relative entries, as a sorted set.
    pub fn paths(&self, rels: &[&str]) -> BTreeSet<PathBuf> {
        rels.iter().map(|r| self.path(r)).collect()
    }

    /// Creates a hardlink `link` pointing at `target` (both relative).
    pub fn hard_link(&self, target: &str, link: &str) {
        fs::hard_link(self.path(target), self.path(link)).unwrap();
    }

    /// Creates a symlink `link` pointing at `target` (both relative).
    pub fn symlink(&self, target: &str, link: &str) {
        std::os::unix::fs::symlink(self.path(target), self.path(link)).unwrap();
    }
}

/// A 1 MiB buffer filled with `byte`.
pub fn mib(byte: u8) -> Vec<u8> {
    vec![byte; MIB]
}

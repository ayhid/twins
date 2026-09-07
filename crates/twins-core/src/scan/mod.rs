//! Directory walk producing candidate regular files. Symlinks are never
//! followed, bundles are never entered, iCloud placeholders are never read
//! and non-local volumes are skipped.

mod options;
mod rules;
mod walk;

pub use options::Options;
pub use walk::{Stats, walk};

use std::path::PathBuf;

use crate::fsutil::FsError;

/// Fatal errors of a walk. Per-file problems are reported through the
/// `on_error` callback instead.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// No root was given.
    #[error("no root to scan")]
    NoRoots,
    /// A root lies in a protected location.
    #[error("{0}: protected location, refusing to scan")]
    ProtectedRoot(PathBuf),
    /// A root is on a network volume and `include_remote` is off.
    #[error("{0}: not a local volume (use --include-remote)")]
    RemoteRoot(PathBuf),
    /// A root is not a directory.
    #[error("{0}: not a directory")]
    NotDirectory(PathBuf),
    /// An exclusion glob does not parse.
    #[error("invalid exclude pattern {pattern:?}: {source}")]
    Glob {
        /// The offending pattern.
        pattern: String,
        /// Parser error.
        #[source]
        source: globset::Error,
    },
    /// A system call on a root failed.
    #[error(transparent)]
    Io(#[from] FsError),
    /// The walk was cancelled through [`Options::cancel`].
    #[error("scan cancelled")]
    Cancelled,
}

//! Content fingerprinting abstraction, so a persistent cache can be layered
//! on top of the raw implementation.

use std::sync::atomic::AtomicBool;

use crate::fsutil::FileMeta;
use crate::hash::{self, Digest, HashError};

/// Computes partial and full fingerprints of a file.
pub trait Hasher: Send + Sync {
    /// Cheap fingerprint of the head and tail of the file.
    ///
    /// # Errors
    /// When the file cannot be read.
    fn partial(&self, m: &FileMeta) -> Result<u64, HashError>;

    /// Full content digest.
    ///
    /// # Errors
    /// When the file cannot be read.
    fn full(&self, m: &FileMeta) -> Result<Digest, HashError>;

    /// Full content digest that may stop early when `cancel` is raised.
    ///
    /// An interrupted hash never yields a digest: a cancelled call returns
    /// an error, so no digest of a file prefix can be produced or cached.
    /// The default ignores the flag and calls [`Hasher::full`], which keeps
    /// existing implementors source-compatible; they are then only stopped
    /// between files.
    ///
    /// # Errors
    /// When the file cannot be read, or, for implementations that poll the
    /// flag, with op `cancelled` and [`std::io::ErrorKind::Interrupted`] once
    /// `cancel` is raised.
    fn full_cancellable(&self, m: &FileMeta, cancel: &AtomicBool) -> Result<Digest, HashError> {
        let _ = cancel;
        self.full(m)
    }
}

/// Reads the file every time.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirectHasher;

impl Hasher for DirectHasher {
    fn partial(&self, m: &FileMeta) -> Result<u64, HashError> {
        hash::partial(m.path(), m.size())
    }

    fn full(&self, m: &FileMeta) -> Result<Digest, HashError> {
        hash::full(m.path())
    }

    fn full_cancellable(&self, m: &FileMeta, cancel: &AtomicBool) -> Result<Digest, HashError> {
        hash::full_cancellable(m.path(), cancel)
    }
}

//! Content fingerprinting abstraction, so a persistent cache can be layered
//! on top of the raw implementation.

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
}

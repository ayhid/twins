//! Cheap partial fingerprints and full content digests.

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use xxhash_rust::xxh64::Xxh64;

/// Bytes read at each end of a file by [`partial`].
pub const SAMPLE_SIZE: u64 = 16 * 1024;
/// Streaming buffer used by [`full`] and [`equal`].
const BUFFER_SIZE: usize = 256 * 1024;
/// [`SAMPLE_SIZE`] as a buffer length.
const SAMPLE_LEN: usize = 16 * 1024;

/// I/O failure while hashing, carrying the offending path.
#[derive(Debug, thiserror::Error)]
#[error("{op} {path}: {source}")]
pub struct HashError {
    /// Operation attempted, e.g. `open` or `read`.
    pub op: &'static str,
    /// Path the operation was attempted on.
    pub path: PathBuf,
    /// Underlying error.
    #[source]
    pub source: io::Error,
}

fn err<'a>(op: &'static str, path: &'a Path) -> impl FnOnce(io::Error) -> HashError + 'a {
    move |source| HashError {
        op,
        path: path.to_path_buf(),
        source,
    }
}

/// 256-bit BLAKE3 content hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Raw bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Parses the lowercase hex form produced by `Display`.
    #[must_use]
    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(Self(out))
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({self})")
    }
}

/// Fingerprints the first and last [`SAMPLE_SIZE`] bytes of the file.
/// Files no larger than two samples are read entirely. Designed to discard
/// non-duplicates with at most two reads.
///
/// # Errors
/// [`HashError`] when the file cannot be opened or read.
pub fn partial(path: &Path, size: u64) -> Result<u64, HashError> {
    let mut f = File::open(path).map_err(err("open", path))?;
    let mut h = Xxh64::new(0);
    if size <= 2 * SAMPLE_SIZE {
        let mut buf = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        f.read_to_end(&mut buf).map_err(err("read", path))?;
        h.update(&buf);
        return Ok(h.digest());
    }
    let mut buf = vec![0u8; SAMPLE_LEN];
    f.read_exact(&mut buf).map_err(err("read head", path))?;
    h.update(&buf);
    f.seek(SeekFrom::Start(size - SAMPLE_SIZE))
        .map_err(err("seek", path))?;
    let n = read_up_to(&mut f, &mut buf).map_err(err("read tail", path))?;
    h.update(&buf[..n]);
    Ok(h.digest())
}

/// Computes the BLAKE3 digest of the whole file.
///
/// # Errors
/// [`HashError`] when the file cannot be opened or read.
pub fn full(path: &Path) -> Result<Digest, HashError> {
    let mut f = File::open(path).map_err(err("open", path))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        let n = f.read(&mut buf).map_err(err("read", path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(Digest(*hasher.finalize().as_bytes()))
}

/// Compares two files byte by byte: the final, hash-free verification.
///
/// # Errors
/// [`HashError`] when either file cannot be opened or read.
pub fn equal(a: &Path, b: &Path) -> Result<bool, HashError> {
    let mut fa = File::open(a).map_err(err("open", a))?;
    let mut fb = File::open(b).map_err(err("open", b))?;
    let mut ba = vec![0u8; BUFFER_SIZE];
    let mut bb = vec![0u8; BUFFER_SIZE];
    loop {
        let na = read_up_to(&mut fa, &mut ba).map_err(err("read", a))?;
        let nb = read_up_to(&mut fb, &mut bb).map_err(err("read", b))?;
        if na != nb || ba[..na] != bb[..nb] {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
    }
}

/// Computes the BLAKE3 digest of the whole file, honouring a cancel flag.
///
/// # Errors
/// [`HashError`] when the file cannot be opened or read.
pub fn full_cancellable(path: &Path, cancel: &AtomicBool) -> Result<Digest, HashError> {
    let _ = cancel; // RED stub: the flag is not polled yet.
    full(path)
}

/// Compares two files byte by byte, honouring a cancel flag.
///
/// # Errors
/// [`HashError`] when either file cannot be opened or read.
pub fn equal_cancellable(a: &Path, b: &Path, cancel: &AtomicBool) -> Result<bool, HashError> {
    let _ = cancel; // RED stub: the flag is not polled yet.
    equal(a, b)
}

/// Fills `buf` as much as possible, stopping early only at end of file.
fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

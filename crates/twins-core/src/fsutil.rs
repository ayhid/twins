//! Low-level, macOS-aware file metadata helpers.

use std::ffi::{CString, OsStr};
use std::os::macos::fs::MetadataExt as MacMetadataExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// BSD `st_flags` bit set on iCloud placeholders whose data is not on disk.
const SF_DATALESS: u32 = 0x4000_0000;

/// Errors raised while inspecting the filesystem.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// The path is not a regular file (symlink, directory, device...).
    #[error("{0}: not a regular file")]
    NotRegular(PathBuf),
    /// An underlying system call failed.
    #[error("{op} {path}: {source}")]
    Io {
        /// Operation attempted, e.g. `lstat`.
        op: &'static str,
        /// Path the operation was attempted on.
        path: PathBuf,
        /// Error reported by the OS.
        #[source]
        source: std::io::Error,
    },
}

/// Uniquely identifies the physical file on disk. Two paths with the same
/// identity are hardlinks of the same data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identity {
    dev: u64,
    inode: u64,
}

impl Identity {
    /// Device number.
    #[must_use]
    pub fn dev(self) -> u64 {
        self.dev
    }

    /// Inode number.
    #[must_use]
    pub fn inode(self) -> u64 {
        self.inode
    }
}

/// Immutable snapshot of a file's identity and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    path: PathBuf,
    size: u64,
    mtime: SystemTime,
    identity: Identity,
    nlink: u64,
    dataless: bool,
}

impl FileMeta {
    /// Path the metadata was read from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Size in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Last modification time.
    #[must_use]
    pub fn mtime(&self) -> SystemTime {
        self.mtime
    }

    /// Physical identity `(dev, inode)`.
    #[must_use]
    pub fn identity(&self) -> Identity {
        self.identity
    }

    /// Number of hard links pointing at the data.
    #[must_use]
    pub fn nlink(&self) -> u64 {
        self.nlink
    }

    /// Whether the file is an iCloud placeholder: reading it would trigger
    /// a download.
    #[must_use]
    pub fn dataless(&self) -> bool {
        self.dataless
    }
}

/// Returns the metadata of a regular file without following symlinks.
///
/// # Errors
/// [`FsError::NotRegular`] for anything but a regular file, [`FsError::Io`]
/// when `lstat` fails.
pub fn stat(path: &Path) -> Result<FileMeta, FsError> {
    let md = std::fs::symlink_metadata(path).map_err(|source| FsError::Io {
        op: "lstat",
        path: path.to_path_buf(),
        source,
    })?;
    if !md.file_type().is_file() {
        return Err(FsError::NotRegular(path.to_path_buf()));
    }
    Ok(FileMeta {
        path: path.to_path_buf(),
        size: md.size(),
        mtime: mtime_of(&md),
        identity: Identity {
            dev: md.dev(),
            inode: md.ino(),
        },
        nlink: md.nlink(),
        dataless: md.st_flags() & SF_DATALESS != 0,
    })
}

fn mtime_of(md: &std::fs::Metadata) -> SystemTime {
    let secs = md.mtime();
    let nanos = u32::try_from(md.mtime_nsec().clamp(0, 999_999_999)).unwrap_or(0);
    let offset = Duration::new(secs.unsigned_abs(), nanos);
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + offset
    } else {
        SystemTime::UNIX_EPOCH - offset
    }
}

/// Reports whether the filesystem holding `path` is a local (non-network)
/// mount.
///
/// # Errors
/// [`FsError::Io`] when `statfs` fails.
#[allow(unsafe_code)] // statfs has no std equivalent
pub fn is_local_volume(path: &Path) -> Result<bool, FsError> {
    let io = |source| FsError::Io {
        op: "statfs",
        path: path.to_path_buf(),
        source,
    };
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: `c_path` is a valid NUL-terminated string and `fs` points at
    // writable memory of the right size; statfs fully initialises it on
    // success, which is the only case we read it.
    let rc = unsafe { libc::statfs(c_path.as_ptr(), fs.as_mut_ptr()) };
    if rc != 0 {
        return Err(io(std::io::Error::last_os_error()));
    }
    // SAFETY: rc == 0 guarantees the struct was initialised.
    let fs = unsafe { fs.assume_init() };
    Ok(fs.f_flags & libc::MNT_LOCAL as u32 != 0)
}

/// macOS package directories treated as atomic units, never descended into.
const BUNDLE_EXTENSIONS: &[&str] = &[
    "app",
    "framework",
    "bundle",
    "plugin",
    "kext",
    "photoslibrary",
    "musiclibrary",
    "tvlibrary",
    "sparsebundle",
    "xcodeproj",
    "xcworkspace",
    "pkg",
    "prefpane",
    "qlgenerator",
    "appex",
    "playground",
];

/// Reports whether a directory name denotes a macOS bundle.
#[must_use]
pub fn is_bundle(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| {
            let ext = ext.to_ascii_lowercase();
            BUNDLE_EXTENSIONS.contains(&ext.as_str())
        })
}

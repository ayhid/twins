//! Guard rails: locations that are never scanned nor modified.

use std::path::{Component, Path, PathBuf};

/// Never scanned nor modified, whatever the flags.
const PROTECTED_PREFIXES: &[&str] = &[
    "/System",
    "/Library",
    "/usr",
    "/bin",
    "/sbin",
    "/private/etc",
    "/private/var",
    "/etc",
    "/var",
    "/dev",
    "/cores",
    "/Applications",
];

/// User-writable temporary areas carved out of the protected tree (macOS
/// puts per-user temp dirs under `/private/var/folders`).
const ALLOWED_PREFIXES: &[&str] = &[
    "/private/var/folders",
    "/private/var/tmp",
    "/private/tmp",
    "/var/folders",
    "/var/tmp",
    "/tmp",
];

/// Reports whether `path` lies inside a hard-coded protected location.
/// The path is normalised lexically; symlinks are not resolved. A relative
/// path is resolved against the current directory and, if that fails,
/// treated as protected.
#[must_use]
pub fn is_protected(path: &Path) -> bool {
    let Some(abs) = absolutize(path) else {
        return true;
    };
    if abs == Path::new("/") {
        return true;
    }
    if has_prefix(&abs, ALLOWED_PREFIXES) {
        return false;
    }
    has_prefix(&abs, PROTECTED_PREFIXES)
}

/// Reports whether `path` is `home/Library` or lies inside it. Excluded
/// from scans unless explicitly opted in.
#[must_use]
pub fn is_user_library(home: &Path, path: &Path) -> bool {
    let lib = home.join("Library");
    absolutize(path).is_some_and(|abs| abs.starts_with(&lib))
}

fn has_prefix(abs: &Path, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| abs.starts_with(p))
}

/// Makes a path absolute and removes `.` / `..` components without
/// touching the filesystem.
#[must_use]
pub fn absolutize(path: &Path) -> Option<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    Some(out)
}

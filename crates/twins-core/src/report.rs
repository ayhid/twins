//! Renders a plan as JSON (for scripts) or text (for humans). The JSON
//! schema is versioned; bump [`VERSION`] on breaking changes.

use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::fsutil::FileMeta;
use crate::group::{Action, Strategy, total_reclaimable};
use crate::human::human_size;

/// Version of the JSON schema.
pub const VERSION: u32 = 1;

/// Scan-level information carried into the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// Roots that were scanned.
    pub roots: Vec<PathBuf>,
    /// Regular files seen by the walk.
    pub files: u64,
    /// Files handed to the grouping pipeline.
    pub candidates: u64,
    /// Keep strategy that produced the plan.
    pub strategy: Strategy,
    /// Whether the plan was only simulated.
    pub dry_run: bool,
}

/// Machine-readable result of a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Schema version, see [`VERSION`].
    pub version: u32,
    /// UTC RFC 3339 instant the report was produced.
    pub scanned_at: String,
    /// Roots that were scanned.
    pub roots: Vec<String>,
    /// Keep strategy name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keep_strategy: String,
    /// Whether the plan was only simulated.
    pub dry_run: bool,
    /// Aggregates of the plan.
    pub summary: Summary,
    /// One entry per duplicate group.
    pub groups: Vec<GroupEntry>,
}

/// Aggregates of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Regular files seen by the walk.
    pub files_scanned: u64,
    /// Files handed to the grouping pipeline.
    pub candidates: u64,
    /// Duplicate groups found.
    pub groups: u64,
    /// Files slated for removal.
    pub duplicates: u64,
    /// Bytes freed by executing the plan.
    pub reclaimable_bytes: u64,
    /// [`Summary::reclaimable_bytes`] in human form.
    pub reclaimable: String,
}

/// One set of identical files with the resolved decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupEntry {
    /// Size of every member.
    pub size: u64,
    /// BLAKE3 digest, lowercase hex.
    pub digest: String,
    /// Bytes freed by executing this group's action.
    pub reclaimable_bytes: u64,
    /// Path of the surviving file.
    pub keep: String,
    /// Paths to remove.
    pub remove: Vec<String>,
    /// Every member, sorted by path.
    pub files: Vec<FileEntry>,
}

/// One member of a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Absolute path.
    pub path: String,
    /// Inode number, so hardlinks can be told apart from copies.
    pub inode: u64,
    /// Modification time, UTC RFC 3339.
    pub mtime: String,
}

/// Converts a plan into a report stamped with `now`.
#[must_use]
pub fn build(actions: &[Action], meta: &Meta, now: SystemTime) -> Report {
    let groups: Vec<GroupEntry> = actions.iter().map(to_entry).collect();
    let duplicates = actions.iter().map(|a| a.remove().len() as u64).sum();
    let reclaimable = total_reclaimable(actions);
    Report {
        version: VERSION,
        scanned_at: rfc3339(now),
        roots: meta.roots.iter().map(display).collect(),
        keep_strategy: meta.strategy.name().to_owned(),
        dry_run: meta.dry_run,
        summary: Summary {
            files_scanned: meta.files,
            candidates: meta.candidates,
            groups: groups.len() as u64,
            duplicates,
            reclaimable_bytes: reclaimable,
            reclaimable: human_size(reclaimable),
        },
        groups,
    }
}

fn to_entry(a: &Action) -> GroupEntry {
    GroupEntry {
        size: a.group().size(),
        digest: a.group().digest().to_string(),
        reclaimable_bytes: a.reclaimable(),
        keep: display(a.keep().path()),
        remove: a.remove().iter().map(|f| display(f.path())).collect(),
        files: a.group().files().iter().map(to_file).collect(),
    }
}

fn to_file(f: &FileMeta) -> FileEntry {
    FileEntry {
        path: display(f.path()),
        inode: f.identity().inode(),
        mtime: rfc3339(f.mtime()),
    }
}

fn display(p: impl AsRef<std::path::Path>) -> String {
    p.as_ref().to_string_lossy().into_owned()
}

/// Writes the report as indented JSON followed by a newline.
///
/// # Errors
/// When the writer fails.
pub fn write_json(w: &mut impl Write, r: &Report) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, r)?;
    w.write_all(b"\n")
}

/// Writes a human-readable listing. Each group prints its shared folder once,
/// on a line ending in `/`, then one row per member with its path relative to
/// that folder. Every row is labelled from the report's `keep` and `remove`
/// fields: `keep` for the kept file, `keep` with a `(hardlink)` suffix for
/// hardlinks of it, and `remove` for files the plan removes.
///
/// When the members share only the filesystem root, there is no folder line
/// and rows show full paths. Groups are separated by one blank line. Control
/// characters in paths are printed escaped, so each file stays on one row.
///
/// # Errors
/// When the writer fails.
pub fn write_text(w: &mut impl Write, r: &Report) -> io::Result<()> {
    if r.groups.is_empty() {
        return writeln!(
            w,
            "No duplicates found ({} files scanned).",
            r.summary.files_scanned
        );
    }
    for (i, g) in r.groups.iter().enumerate() {
        if i > 0 {
            writeln!(w)?;
        }
        writeln!(
            w,
            "[{}] {} × {}  ({} reclaimable)",
            i + 1,
            human_size(g.size),
            g.files.len(),
            human_size(g.reclaimable_bytes)
        )?;
        let folder = shared_folder(g);
        if let Some(folder) = &folder {
            writeln!(w, "  {}/", printable(folder))?;
        }
        let mut rows: Vec<(Role, &FileEntry)> =
            g.files.iter().map(|f| (Role::of(&f.path, g), f)).collect();
        rows.sort_by_key(|(role, _)| *role);
        for (role, f) in rows {
            let full = Path::new(&f.path);
            let shown = folder
                .as_deref()
                .and_then(|d| full.strip_prefix(d).ok())
                .unwrap_or(full);
            let (label, suffix) = match role {
                Role::Keep => ("keep", ""),
                Role::Hardlink => ("keep", "  (hardlink)"),
                Role::Remove => ("remove", ""),
            };
            writeln!(w, "    {label:<6}  {}{suffix}", printable(shown))?;
        }
    }
    writeln!(
        w,
        "\n{} group{}, {} duplicate{}, {} reclaimable ({} files scanned)",
        r.summary.groups,
        plural(r.summary.groups),
        r.summary.duplicates,
        plural(r.summary.duplicates),
        r.summary.reclaimable,
        r.summary.files_scanned
    )
}

/// What happens to a group member, in the order rows are printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Keep,
    Hardlink,
    Remove,
}

impl Role {
    /// The role of `path` in `g`, read only from the plan's `keep` and
    /// `remove` fields.
    fn of(path: &str, g: &GroupEntry) -> Self {
        if path == g.keep {
            Self::Keep
        } else if g.remove.iter().any(|r| r == path) {
            Self::Remove
        } else {
            // Keeper::choose never removes a hardlink of the kept file.
            Self::Hardlink
        }
    }
}

/// The longest common folder of the members' parent folders, or `None` when
/// it has at most one component (only `/`, or nothing for relative paths).
fn shared_folder(g: &GroupEntry) -> Option<PathBuf> {
    let mut parents = g
        .files
        .iter()
        .map(|f| Path::new(&f.path).parent().unwrap_or(Path::new("")));
    let mut common: Vec<Component> = parents.next()?.components().collect();
    for p in parents {
        let same = common
            .iter()
            .zip(p.components())
            .take_while(|(a, b)| **a == *b)
            .count();
        common.truncate(same);
    }
    (common.len() > 1).then(|| common.iter().collect())
}

/// `path` as shown to a human: every control character is replaced by its
/// escaped form (a newline becomes `\n`), every other character is kept.
fn printable(path: &Path) -> String {
    let mut s = String::new();
    for c in path.to_string_lossy().chars() {
        if c.is_control() {
            s.extend(c.escape_debug());
        } else {
            s.push(c);
        }
    }
    s
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Formats an instant as UTC RFC 3339 with second precision, e.g.
/// `2024-01-01T00:00:00Z`. Instants before the epoch clamp to it.
#[must_use]
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(i64::try_from(days).unwrap_or(i64::MAX));
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm (proleptic Gregorian).
#[allow(clippy::similar_names)] // doe/doy/yoe are the paper's names; clippy 1.90 flags them
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 1..=12, 1..=31
    (y, m as u32, d as u32)
}

//! End-to-end tests of the `twins` binary.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn twins() -> Command {
    let mut c = Command::cargo_bin("twins").expect("twins binary is built");
    c.env_remove("TWINS_CONFIG");
    c
}

const MIB: usize = 1024 * 1024;

/// Two 1 MiB duplicates, one 4-byte pair (below the default min size), a
/// unique file, a hardlink and an excluded log.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let w = |rel: &str, content: &[u8]| {
        let p = dir.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
    };
    w("a.bin", &vec![1u8; MIB]);
    w("sub/a-copy.bin", &vec![1u8; MIB]);
    w("s1", b"tiny");
    w("s2", b"tiny");
    w("unique.bin", &vec![2u8; MIB]);
    w("dup.log", &vec![1u8; MIB]);
    fs::hard_link(dir.path().join("a.bin"), dir.path().join("a-link.bin")).unwrap();
    dir
}

fn groups(json: &serde_json::Value) -> Vec<Vec<PathBuf>> {
    json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            g["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| PathBuf::from(f["path"].as_str().unwrap()))
                .collect()
        })
        .collect()
}

#[test]
fn version_prints_crate_version() {
    let expected = format!("twins {}\n", env!("CARGO_PKG_VERSION"));
    twins().arg("version").assert().success().stdout(expected);
}

#[test]
fn scan_json_reports_exactly_the_duplicate_groups() {
    let dir = fixture();
    let out = twins()
        .args(["scan", "--json", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["keep_strategy"], "oldest");
    assert_eq!(json["summary"]["groups"], 1);
    assert_eq!(json["summary"]["duplicates"], 1);
    assert_eq!(json["summary"]["reclaimable_bytes"], MIB);
    let g = &json["groups"][0];
    assert_eq!(g["size"], MIB);
    assert_eq!(g["reclaimable_bytes"], MIB);
    assert_eq!(g["remove"].as_array().unwrap().len(), 1);
    assert_eq!(
        groups(&json),
        vec![vec![
            dir.path().join("a-link.bin"),
            dir.path().join("a.bin"),
            dir.path().join("sub/a-copy.bin"),
        ]]
    );
}

#[test]
fn min_size_widens_the_scan_and_report_is_json_by_default() {
    let dir = fixture();
    let out = twins()
        .args(["report", "--min-size", "1", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let mut found = groups(&json);
    for g in &mut found {
        g.sort();
    }
    assert_eq!(found.len(), 2);
    assert_eq!(found[1], vec![dir.path().join("s1"), dir.path().join("s2")]);
}

#[test]
fn scan_text_marks_kept_file_and_never_lists_hardlinks_to_remove() {
    let dir = fixture();
    twins()
        .args(["scan", "--exclude", "*.log", "--jobs", "2"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[1] 1.0 MiB × 3  (1.0 MiB reclaimable)",
        ))
        .stdout(predicate::str::contains("★"))
        .stdout(predicate::str::contains(
            "1 group, 1 duplicate, 1.0 MiB reclaimable",
        ));
}

#[test]
fn nothing_found_is_reported_plainly() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("only.bin"), vec![3u8; MIB]).unwrap();
    twins()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout("No duplicates found (1 files scanned).\n");
}

#[test]
fn protected_root_and_bad_flags_fail_with_a_message() {
    twins()
        .args(["scan", "/System"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("protected"));
    twins()
        .args(["scan", "--min-size", "12X"])
        .arg(Path::new("."))
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid size"));
    twins()
        .args(["scan", "--exclude", "["])
        .arg(Path::new("."))
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid exclude pattern"));
}

// Characterization tests: they lock the exact output of `scan` and `report`
// so moving the orchestration into twins-core cannot change it silently.

fn file_entry(path: &Path) -> serde_json::Value {
    use std::os::unix::fs::MetadataExt;
    let md = fs::metadata(path).unwrap();
    serde_json::json!({
        "path": path.to_string_lossy(),
        "inode": md.ino(),
        "mtime": twins_core::report::rfc3339(md.modified().unwrap()),
    })
}

fn is_rfc3339_utc(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

#[test]
fn characterize_json_report_with_relative_root() {
    let dir = fixture();
    // The walk absolutizes a relative root against the process cwd, which
    // macOS reports through /private/var while tempdir() returns /var.
    let base = fs::canonicalize(dir.path()).unwrap();
    let out = twins()
        .current_dir(dir.path())
        .args(["scan", "--json", "--exclude", "*.log", "."])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let raw = String::from_utf8(out).unwrap();

    let keys = [
        "version",
        "scanned_at",
        "roots",
        "keep_strategy",
        "dry_run",
        "summary",
        "groups",
    ];
    let positions: Vec<usize> = keys
        .iter()
        .map(|k| {
            raw.find(&format!("\n  \"{k}\":"))
                .unwrap_or_else(|| panic!("top-level key {k} missing"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "top-level keys out of order: {positions:?}"
    );

    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let scanned_at = json
        .as_object_mut()
        .unwrap()
        .remove("scanned_at")
        .expect("scanned_at present");
    assert!(
        is_rfc3339_utc(scanned_at.as_str().unwrap()),
        "scanned_at {scanned_at} is not YYYY-MM-DDTHH:MM:SSZ"
    );

    let p = |rel: &str| base.join(rel);
    let expected = serde_json::json!({
        "version": 1,
        "roots": ["."],
        "keep_strategy": "oldest",
        "dry_run": false,
        "summary": {
            "files_scanned": 7,
            "candidates": 4,
            "groups": 1,
            "duplicates": 1,
            "reclaimable_bytes": MIB,
            "reclaimable": "1.0 MiB",
        },
        "groups": [{
            "size": MIB,
            "digest": "408b168d5e17ab41ebbccdfce45ad4da9cf9d4bb55aa56c5c2b11da98c13a0bc",
            "reclaimable_bytes": MIB,
            "keep": p("a-link.bin").to_string_lossy(),
            "remove": [p("sub/a-copy.bin").to_string_lossy()],
            "files": [
                file_entry(&p("a-link.bin")),
                file_entry(&p("a.bin")),
                file_entry(&p("sub/a-copy.bin")),
            ],
        }],
    });
    assert_eq!(json, expected);
}

#[test]
fn characterize_text_report() {
    let dir = fixture();
    let p = |rel: &str| dir.path().join(rel).display().to_string();
    let expected = format!(
        "[1] 1.0 MiB × 3  (1.0 MiB reclaimable)\n  ★ {}\n    {}\n    {}\n\n1 group, 1 duplicate, 1.0 MiB reclaimable (7 files scanned)\n",
        p("a-link.bin"),
        p("a.bin"),
        p("sub/a-copy.bin"),
    );
    twins()
        .args(["scan", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(expected)
        .stderr("");
}

#[test]
fn usage_errors_exit_2_with_the_same_message() {
    twins()
        .args(["scan", "/System"])
        .assert()
        .code(2)
        .stdout("")
        .stderr("twins: /System: protected location, refusing to scan\n");
    twins()
        .args(["scan", "--min-size", "12X", "."])
        .assert()
        .code(2)
        .stdout("")
        .stderr("twins: invalid size \"12X\"\n");
    twins()
        .args(["scan", "--exclude", "[", "."])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "twins: invalid exclude pattern \"[\": ",
        ));
}

#[test]
fn unreadable_files_are_counted_and_listed_with_verbose() {
    use std::os::unix::fs::PermissionsExt;

    let dir = fixture();
    let unique = dir.path().join("unique.bin");
    fs::set_permissions(&unique, fs::Permissions::from_mode(0o000)).unwrap();
    let restore = || fs::set_permissions(&unique, fs::Permissions::from_mode(0o644)).unwrap();
    if fs::File::open(&unique).is_ok() {
        // Running as root: permissions do not stop the read.
        restore();
        return;
    }

    twins()
        .args(["scan", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr("1 files could not be read (use --verbose to list them)\n");
    twins()
        .args(["scan", "-v", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr(
            predicate::str::contains(format!("skip {}: open ", unique.display()))
                .and(predicate::str::contains("could not be read").not()),
        );
    twins()
        .args(["scan", "--json", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr("");

    restore();
}

#[test]
fn json_stderr_is_silent_when_not_a_terminal() {
    // assert_cmd pipes stderr, so it is not a terminal: no progress line.
    let dir = fixture();
    let out = twins()
        .args(["scan", "--json", "--exclude", "*.log"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice::<serde_json::Value>(&out).expect("stdout is JSON");
}

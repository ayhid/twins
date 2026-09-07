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

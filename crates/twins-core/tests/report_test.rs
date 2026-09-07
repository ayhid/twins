//! Tests for the JSON / text report.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use twins_core::fsutil::{FileMeta, Identity};
use twins_core::group::{Group, Keeper, Strategy, plan};
use twins_core::hash::Digest;
use twins_core::report::{self, Meta, Report};

fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

fn sample() -> Report {
    let files = vec![
        FileMeta::new(
            PathBuf::from("/r/b.bin"),
            2048,
            at(1_700_000_100),
            Identity::new(1, 11),
        ),
        FileMeta::new(
            PathBuf::from("/r/a.bin"),
            2048,
            at(1_700_000_000),
            Identity::new(1, 10),
        ),
        FileMeta::new(
            PathBuf::from("/r/c.bin"),
            2048,
            at(1_700_000_200),
            Identity::new(1, 10),
        ),
    ];
    let g = Group::new(2048, Digest::from_hex(&"ab".repeat(32)).unwrap(), files);
    let actions = plan(&[g], &Keeper::new(Strategy::Oldest, None));
    let meta = Meta {
        roots: vec![PathBuf::from("/r")],
        files: 7,
        candidates: 3,
        strategy: Strategy::Oldest,
        dry_run: false,
    };
    report::build(&actions, &meta, at(1_704_067_200))
}

#[test]
fn json_schema_is_stable() {
    let mut out = Vec::new();
    report::write_json(&mut out, &sample()).unwrap();
    let expected = r#"{
  "version": 1,
  "scanned_at": "2024-01-01T00:00:00Z",
  "roots": [
    "/r"
  ],
  "keep_strategy": "oldest",
  "dry_run": false,
  "summary": {
    "files_scanned": 7,
    "candidates": 3,
    "groups": 1,
    "duplicates": 1,
    "reclaimable_bytes": 2048,
    "reclaimable": "2.0 KiB"
  },
  "groups": [
    {
      "size": 2048,
      "digest": "abababababababababababababababababababababababababababababababab",
      "reclaimable_bytes": 2048,
      "keep": "/r/a.bin",
      "remove": [
        "/r/b.bin"
      ],
      "files": [
        {
          "path": "/r/a.bin",
          "inode": 10,
          "mtime": "2023-11-14T22:13:20Z"
        },
        {
          "path": "/r/b.bin",
          "inode": 11,
          "mtime": "2023-11-14T22:15:00Z"
        },
        {
          "path": "/r/c.bin",
          "inode": 10,
          "mtime": "2023-11-14T22:16:40Z"
        }
      ]
    }
  ]
}
"#;
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

#[test]
fn json_round_trips_through_serde() {
    let r = sample();
    let text = serde_json::to_string(&r).unwrap();
    let back: Report = serde_json::from_str(&text).unwrap();
    assert_eq!(back, r);
}

#[test]
fn text_marks_the_kept_file_and_summarises() {
    let mut out = Vec::new();
    report::write_text(&mut out, &sample()).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("[1] 2.0 KiB × 3  (2.0 KiB reclaimable)"),
        "{text}"
    );
    assert!(text.contains("  ★ /r/a.bin\n"), "{text}");
    assert!(text.contains("    /r/b.bin\n"), "{text}");
    assert!(
        text.ends_with("\n1 group, 1 duplicate, 2.0 KiB reclaimable (7 files scanned)\n"),
        "{text}"
    );
}

#[test]
fn text_for_an_empty_report() {
    let meta = Meta {
        roots: vec![],
        files: 3,
        candidates: 0,
        strategy: Strategy::Oldest,
        dry_run: false,
    };
    let r = report::build(&[], &meta, at(0));
    let mut out = Vec::new();
    report::write_text(&mut out, &r).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "No duplicates found (3 files scanned).\n"
    );
}

#[test]
fn rfc3339_formats_utc_instants() {
    assert_eq!(report::rfc3339(at(0)), "1970-01-01T00:00:00Z");
    assert_eq!(report::rfc3339(at(951_782_400)), "2000-02-29T00:00:00Z");
    assert_eq!(report::rfc3339(at(4_102_444_799)), "2099-12-31T23:59:59Z");
}

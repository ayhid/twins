//! Tests for file metadata helpers.

use std::fs;

use twins_core::fsutil::{self, FileMeta};

#[test]
fn stat_reports_size_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, b"hello").unwrap();

    let meta: FileMeta = fsutil::stat(&path).unwrap();

    assert_eq!(meta.path(), path.as_path());
    assert_eq!(meta.size(), 5);
    assert_eq!(meta.nlink(), 1);
    assert!(!meta.dataless());
    assert_ne!(meta.identity().inode(), 0);
}

#[test]
fn hardlinks_share_identity_and_report_nlink() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.bin");
    let b = dir.path().join("b.bin");
    fs::write(&a, b"same").unwrap();
    fs::hard_link(&a, &b).unwrap();

    let ma = fsutil::stat(&a).unwrap();
    let mb = fsutil::stat(&b).unwrap();

    assert_eq!(ma.identity(), mb.identity());
    assert_eq!(ma.nlink(), 2);
}

#[test]
fn stat_rejects_symlinks_and_directories() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("t.bin");
    fs::write(&target, b"x").unwrap();
    let link = dir.path().join("l.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    assert!(matches!(
        fsutil::stat(&link),
        Err(fsutil::FsError::NotRegular(_))
    ));
    assert!(matches!(
        fsutil::stat(dir.path()),
        Err(fsutil::FsError::NotRegular(_))
    ));
}

#[test]
fn stat_missing_file_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        fsutil::stat(&dir.path().join("nope")),
        Err(fsutil::FsError::Io { .. })
    ));
}

#[test]
fn temp_dir_is_a_local_volume() {
    let dir = tempfile::tempdir().unwrap();
    assert!(fsutil::is_local_volume(dir.path()).unwrap());
}

#[test]
fn bundle_names_are_detected_case_insensitively() {
    for name in ["Foo.app", "Bar.FRAMEWORK", "x.photoslibrary", "p.xcodeproj"] {
        assert!(fsutil::is_bundle(name.as_ref()), "{name}");
    }
    for name in ["Foo", "app", "notes.txt", ".app.d", "archive.zip"] {
        assert!(!fsutil::is_bundle(name.as_ref()), "{name}");
    }
}

//! Tests for the directory walk and its exclusion rules.

mod fixtures;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;

use fixtures::{Tree, mib};
use twins_core::scan::{self, Options, ScanError};

fn visited(opts: &Options) -> (BTreeSet<PathBuf>, scan::Stats) {
    let seen = Mutex::new(BTreeSet::new());
    let stats = scan::walk(
        opts,
        |m| {
            seen.lock().unwrap().insert(m.path().to_path_buf());
        },
        |_, _| {},
    )
    .unwrap();
    (seen.into_inner().unwrap(), stats)
}

fn tree() -> Tree {
    let big = mib(1);
    Tree::build(&[
        ("a.bin", &big),
        ("sub/b.bin", &big),
        ("small.bin", b"tiny"),
        ("empty.bin", b""),
        ("notes.log", &big),
        ("node_modules/dep/x.bin", &big),
        ("Foo.app/Contents/x.bin", &big),
        (".git/objects/x", &big),
        ("Library/Caches/x.bin", &big),
        ("Library/keep.bin", &big),
    ])
}

#[test]
fn default_rules_visit_only_plain_large_files() {
    let t = tree();
    t.symlink("a.bin", "link.bin");
    let opts = Options::new(vec![t.root().to_path_buf()])
        .exclude(vec!["*.log".into()])
        .home(t.root().to_path_buf());

    let (seen, stats) = visited(&opts);

    assert_eq!(seen, t.paths(&["a.bin", "sub/b.bin"]));
    assert_eq!(stats.candidates, 2);
    assert!(stats.files >= 5, "{stats:?}");
    assert_eq!(stats.errors, 0);
}

#[test]
fn opt_ins_widen_the_walk() {
    let t = tree();
    let opts = Options::new(vec![t.root().to_path_buf()])
        .home(t.root().to_path_buf())
        .include_empty(true)
        .include_node_modules(true)
        .include_library(true);

    let (seen, _) = visited(&opts);

    assert_eq!(
        seen,
        t.paths(&[
            "a.bin",
            "sub/b.bin",
            "empty.bin",
            "notes.log",
            "node_modules/dep/x.bin",
            "Library/keep.bin",
        ])
    );
}

#[test]
fn min_size_zero_still_skips_empty_files() {
    let t = Tree::build(&[("z.bin", b""), ("s.bin", b"x")]);
    let opts = Options::new(vec![t.root().to_path_buf()]).min_size(0);
    let (seen, _) = visited(&opts);
    assert_eq!(seen, t.paths(&["s.bin"]));
}

#[test]
fn path_globs_match_the_full_path() {
    let t = tree();
    let pattern = format!("{}/sub/*.bin", t.root().display());
    let opts = Options::new(vec![t.root().to_path_buf()])
        .home(t.root().to_path_buf())
        .exclude(vec![pattern, "*.log".into()]);

    let (seen, _) = visited(&opts);

    assert_eq!(seen, t.paths(&["a.bin"]));
}

#[test]
fn nested_roots_are_visited_once() {
    let t = tree();
    let opts = Options::new(vec![t.path("sub"), t.root().to_path_buf(), t.path("sub")])
        .home(t.root().to_path_buf())
        .exclude(vec!["*.log".into()]);

    let count = Mutex::new(0);
    scan::walk(&opts, |_| *count.lock().unwrap() += 1, |_, _| {}).unwrap();

    assert_eq!(count.into_inner().unwrap(), 2);
}

#[test]
fn invalid_roots_are_rejected() {
    let t = tree();
    assert!(matches!(
        scan::walk(&Options::new(vec![]), |_| {}, |_, _| {}),
        Err(ScanError::NoRoots)
    ));
    assert!(matches!(
        scan::walk(&Options::new(vec!["/System".into()]), |_| {}, |_, _| {}),
        Err(ScanError::ProtectedRoot(_))
    ));
    assert!(matches!(
        scan::walk(&Options::new(vec![t.path("a.bin")]), |_| {}, |_, _| {}),
        Err(ScanError::NotDirectory(_))
    ));
    assert!(matches!(
        scan::walk(&Options::new(vec![t.path("missing")]), |_| {}, |_, _| {}),
        Err(ScanError::Io { .. })
    ));
}

#[test]
fn bad_glob_is_rejected_up_front() {
    let t = tree();
    let opts = Options::new(vec![t.root().to_path_buf()]).exclude(vec!["[".into()]);
    assert!(matches!(
        scan::walk(&opts, |_| {}, |_, _| {}),
        Err(ScanError::Glob { .. })
    ));
}

#[test]
fn unreadable_directories_are_reported_not_fatal() {
    use std::os::unix::fs::PermissionsExt;
    let t = tree();
    let locked = t.path("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let opts = Options::new(vec![t.root().to_path_buf()])
        .home(t.root().to_path_buf())
        .exclude(vec!["*.log".into()]);
    let errors = Mutex::new(Vec::new());
    let stats = scan::walk(
        &opts,
        |_| {},
        |p, _| errors.lock().unwrap().push(p.to_path_buf()),
    );
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    let stats = stats.unwrap();
    assert_eq!(stats.errors, 1);
    assert_eq!(errors.into_inner().unwrap(), vec![locked]);
}

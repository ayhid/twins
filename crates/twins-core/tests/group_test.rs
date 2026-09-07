//! Tests for the size → partial → full grouping pipeline.

mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use fixtures::{Tree, mib};
use twins_core::fsutil::{self, FileMeta};
use twins_core::group::{
    DirectHasher, FindError, Group, GroupError, Hasher, Index, Options, Progress, Stage, find,
};
use twins_core::hash::{self, Digest, HashError};

fn index_of(t: &Tree, rels: &[&str]) -> Index {
    let idx = Index::new();
    for r in rels {
        idx.add(fsutil::stat(&t.path(r)).unwrap());
    }
    idx
}

fn paths(g: &Group) -> Vec<&Path> {
    g.files().iter().map(FileMeta::path).collect()
}

#[test]
fn groups_identical_files_sorted_by_reclaimable_space() {
    let big = mib(1);
    let t = Tree::build(&[
        ("a1", &big),
        ("a2", &big),
        ("b1", b"bbbb"),
        ("b2", b"bbbb"),
        ("b3", b"bbbb"),
        ("c", b"cccc"),
        ("d", &mib(2)),
    ]);
    let idx = index_of(&t, &["a1", "a2", "b1", "b2", "b3", "c", "d"]);

    let groups = find(&idx, &Options::default()).unwrap();

    assert_eq!(groups.len(), 2);
    assert_eq!(paths(&groups[0]), vec![t.path("a1"), t.path("a2")]);
    assert_eq!(groups[0].size(), fixtures::MIB as u64);
    assert_eq!(groups[0].reclaimable(), fixtures::MIB as u64);
    assert_eq!(
        paths(&groups[1]),
        vec![t.path("b1"), t.path("b2"), t.path("b3")]
    );
    assert_eq!(groups[1].reclaimable(), 8);
    assert_eq!(groups[0].digest(), hash::full(&t.path("a1")).unwrap());
}

#[test]
fn same_size_different_content_is_not_a_duplicate() {
    let t = Tree::build(&[("a", b"abcd"), ("b", b"abce")]);
    let idx = index_of(&t, &["a", "b"]);
    assert!(find(&idx, &Options::default()).unwrap().is_empty());
}

#[test]
fn partial_collision_is_resolved_by_full_hash() {
    let sample = usize::try_from(hash::SAMPLE_SIZE).unwrap();
    let mut a = vec![7u8; 3 * sample];
    let mut b = a.clone();
    a[sample + 5] = 1;
    b[sample + 5] = 2;
    let t = Tree::build(&[("a", &a), ("b", &b), ("a2", &a)]);
    let idx = index_of(&t, &["a", "b", "a2"]);

    let groups = find(&idx, &Options::default()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(paths(&groups[0]), vec![t.path("a"), t.path("a2")]);
}

#[test]
fn hardlinks_are_not_duplicates_of_each_other() {
    let t = Tree::build(&[("a", b"same")]);
    t.hard_link("a", "link");
    let idx = index_of(&t, &["a", "link"]);
    assert!(find(&idx, &Options::default()).unwrap().is_empty());
}

#[test]
fn hardlinks_count_once_physically() {
    let t = Tree::build(&[("a", b"same"), ("b", b"same")]);
    t.hard_link("a", "link");
    let idx = index_of(&t, &["a", "link", "b"]);

    let groups = find(&idx, &Options::default()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files().len(), 3);
    assert_eq!(groups[0].physical(), 2);
    assert_eq!(groups[0].reclaimable(), 4);
}

/// Counts calls and can force every file onto one digest.
struct SpyHasher {
    partial_calls: AtomicUsize,
    full_calls: AtomicUsize,
    collide: bool,
}

impl SpyHasher {
    fn new(collide: bool) -> Self {
        Self {
            partial_calls: AtomicUsize::new(0),
            full_calls: AtomicUsize::new(0),
            collide,
        }
    }
}

impl Hasher for SpyHasher {
    fn partial(&self, m: &FileMeta) -> Result<u64, HashError> {
        self.partial_calls.fetch_add(1, Ordering::SeqCst);
        if self.collide {
            return Ok(42);
        }
        DirectHasher.partial(m)
    }

    fn full(&self, m: &FileMeta) -> Result<Digest, HashError> {
        self.full_calls.fetch_add(1, Ordering::SeqCst);
        if self.collide {
            return Ok(Digest::from_hex(&"ab".repeat(32)).unwrap());
        }
        DirectHasher.full(m)
    }
}

#[test]
fn hashes_each_identity_once() {
    let t = Tree::build(&[("a", b"same"), ("b", b"same")]);
    t.hard_link("a", "l1");
    t.hard_link("a", "l2");
    let idx = index_of(&t, &["a", "l1", "l2", "b"]);
    let spy = Arc::new(SpyHasher::new(false));

    let groups = find(&idx, &Options::default().hasher(spy.clone())).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(spy.partial_calls.load(Ordering::SeqCst), 2);
    assert_eq!(spy.full_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn verify_drops_files_that_do_not_match_byte_for_byte() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd"), ("x", b"abce")]);
    let idx = index_of(&t, &["a", "a2", "x"]);
    let errors = Mutex::new(Vec::new());
    let opts = Options::default()
        .hasher(Arc::new(SpyHasher::new(true)))
        .verify(true)
        .on_error(Box::new(|p, e| {
            errors
                .lock()
                .unwrap()
                .push((p.to_path_buf(), matches!(e, GroupError::HashCollision(_))));
        }));

    let groups = find(&idx, &opts).unwrap();
    drop(opts);

    assert_eq!(groups.len(), 1);
    assert_eq!(paths(&groups[0]), vec![t.path("a"), t.path("a2")]);
    assert_eq!(errors.into_inner().unwrap(), vec![(t.path("x"), true)]);
}

#[test]
fn without_verify_the_hash_is_trusted() {
    let t = Tree::build(&[("a", b"abcd"), ("x", b"abce")]);
    let idx = index_of(&t, &["a", "x"]);
    let opts = Options::default().hasher(Arc::new(SpyHasher::new(true)));
    let groups = find(&idx, &opts).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files().len(), 2);
}

#[test]
fn unreadable_file_is_reported_and_dropped() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd"), ("gone", b"abcd")]);
    let idx = index_of(&t, &["a", "a2", "gone"]);
    std::fs::remove_file(t.path("gone")).unwrap();
    let errors = Mutex::new(Vec::<PathBuf>::new());
    let opts = Options::default().on_error(Box::new(|p, _| {
        errors.lock().unwrap().push(p.to_path_buf());
    }));

    let groups = find(&idx, &opts).unwrap();
    drop(opts);

    assert_eq!(groups.len(), 1);
    assert_eq!(paths(&groups[0]), vec![t.path("a"), t.path("a2")]);
    assert_eq!(errors.into_inner().unwrap(), vec![t.path("gone")]);
}

#[test]
fn reports_progress_per_stage() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd")]);
    let idx = index_of(&t, &["a", "a2"]);
    let seen = Mutex::new(Vec::<Progress>::new());
    let opts = Options::default()
        .verify(true)
        .on_progress(Box::new(|p| seen.lock().unwrap().push(p)));

    find(&idx, &opts).unwrap();
    drop(opts);

    let seen = seen.into_inner().unwrap();
    let last = |stage: Stage| seen.iter().rfind(|p| p.stage == stage).copied();
    assert_eq!(
        last(Stage::Partial).map(|p| (p.done, p.total)),
        Some((2, 2))
    );
    assert_eq!(last(Stage::Full).map(|p| (p.done, p.total)), Some((2, 2)));
    assert!(last(Stage::Verify).is_some());
}

#[test]
fn honours_cancellation() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd")]);
    let idx = index_of(&t, &["a", "a2"]);
    let cancel = Arc::new(AtomicBool::new(true));
    let opts = Options::default().cancel(cancel);
    assert!(matches!(find(&idx, &opts), Err(FindError::Cancelled)));
}

#[test]
fn index_is_safe_for_concurrent_add_and_drops_singletons() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd"), ("c", b"c")]);
    let idx = Index::new();
    std::thread::scope(|s| {
        for r in ["a", "a2", "c"] {
            let idx = &idx;
            let p = t.path(r);
            s.spawn(move || idx.add(fsutil::stat(&p).unwrap()));
        }
    });
    assert_eq!(idx.len(), 3);
    let buckets = idx.candidates();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].len(), 2);
}

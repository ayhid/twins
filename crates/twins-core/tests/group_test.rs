//! Tests for the size → partial → full grouping pipeline.

mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    assert_eq!(find(&idx, &Options::default()).unwrap(), [] as [Group; 0]);
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
    assert_eq!(find(&idx, &Options::default()).unwrap(), [] as [Group; 0]);
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
fn each_stage_starts_with_a_zero_done_marker() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd")]);
    let idx = index_of(&t, &["a", "a2"]);
    let seen = Mutex::new(Vec::<Progress>::new());
    let opts = Options::default()
        .verify(true)
        .on_progress(Box::new(|p| seen.lock().unwrap().push(p)));

    find(&idx, &opts).unwrap();
    drop(opts);

    let seen = seen.into_inner().unwrap();
    let first = |stage: Stage| seen.iter().position(|p| p.stage == stage);
    for stage in [Stage::Partial, Stage::Full, Stage::Verify] {
        let i = first(stage).unwrap_or_else(|| panic!("no {stage} event"));
        let marker = seen[i];
        assert_eq!(marker.done, 0, "first {stage} event is not a marker");
        let last = seen.iter().rfind(|p| p.stage == stage).unwrap();
        assert_eq!(marker.total, last.total, "{stage} marker total");
        assert_eq!(
            seen.iter()
                .filter(|p| p.stage == stage && p.done == 0)
                .count(),
            1,
            "one {stage} marker"
        );
    }
    assert!(first(Stage::Partial) < first(Stage::Full));
    assert!(first(Stage::Full) < first(Stage::Verify));
}

/// 32 identical pairs of one size, 64 files in all: every file reaches
/// both hash stages.
fn same_size_pairs() -> (Tree, Index) {
    let contents: Vec<Vec<u8>> = (0..32u8).map(|i| vec![i; 64]).collect();
    let mut entries = Vec::new();
    for (i, c) in contents.iter().enumerate() {
        entries.push((format!("p{i}a"), c.as_slice()));
        entries.push((format!("p{i}b"), c.as_slice()));
    }
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, c)| (n.as_str(), *c)).collect();
    let t = Tree::build(&refs);
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    let idx = index_of(&t, &names);
    (t, idx)
}

#[test]
fn progress_done_rises_by_one_within_a_stage() {
    // 8 workers race to report.
    let (_t, idx) = same_size_pairs();
    let seen = Mutex::new(Vec::<Progress>::new());
    let opts = Options::default()
        .workers(8)
        .on_progress(Box::new(|p| seen.lock().unwrap().push(p)));

    find(&idx, &opts).unwrap();
    drop(opts);

    let seen = seen.into_inner().unwrap();
    for stage in [Stage::Partial, Stage::Full] {
        let done: Vec<u64> = seen
            .iter()
            .filter(|p| p.stage == stage)
            .map(|p| p.done)
            .collect();
        assert_eq!(
            done,
            (0..=64).collect::<Vec<u64>>(),
            "{stage} done sequence"
        );
    }
}

/// Direct hasher that counts partial fingerprints.
#[derive(Default)]
struct CountingHasher {
    partials: AtomicUsize,
}

impl Hasher for CountingHasher {
    fn partial(&self, m: &FileMeta) -> Result<u64, HashError> {
        self.partials.fetch_add(1, Ordering::SeqCst);
        DirectHasher.partial(m)
    }

    fn full(&self, m: &FileMeta) -> Result<Digest, HashError> {
        DirectHasher.full(m)
    }
}

#[test]
fn a_blocked_progress_callback_does_not_stall_the_other_workers() {
    let (_t, idx) = same_size_pairs();
    let hasher = Arc::new(CountingHasher::default());
    let counted = Arc::clone(&hasher);
    let partials_while_blocked = AtomicUsize::new(0);
    let seen = Mutex::new(Vec::<u64>::new());
    // Had the reporting worker held a lock the others need after every
    // file, they could hash at most one more file each (3) while it waits.
    let opts = Options::default()
        .workers(4)
        .hasher(hasher)
        .on_progress(Box::new(|p| {
            if p.stage != Stage::Partial {
                return;
            }
            if p.done == 1 {
                let deadline = Instant::now() + Duration::from_secs(10);
                while counted.partials.load(Ordering::SeqCst) < 32 && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(1));
                }
                partials_while_blocked
                    .store(counted.partials.load(Ordering::SeqCst), Ordering::SeqCst);
            }
            seen.lock().unwrap().push(p.done);
        }));

    find(&idx, &opts).unwrap();
    drop(opts);

    let blocked = partials_while_blocked.into_inner();
    assert!(
        blocked >= 32,
        "only {blocked} files hashed while the callback blocked"
    );
    // The counts held back while it blocked are still delivered, in order.
    assert_eq!(seen.into_inner().unwrap(), (0..=64).collect::<Vec<u64>>());
}

#[test]
fn empty_index_still_marks_hash_stages() {
    let seen = Mutex::new(Vec::<Progress>::new());
    let opts = Options::default()
        .verify(true)
        .on_progress(Box::new(|p| seen.lock().unwrap().push(p)));

    let groups = find(&Index::new(), &opts).unwrap();
    drop(opts);

    assert_eq!(groups, [] as [Group; 0]);
    let seen: Vec<(Stage, u64, u64)> = seen
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|p| (p.stage, p.done, p.total))
        .collect();
    assert_eq!(
        seen,
        vec![
            (Stage::Partial, 0, 0),
            (Stage::Full, 0, 0),
            (Stage::Verify, 0, 0)
        ]
    );
}

#[test]
fn honours_cancellation() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd")]);
    let idx = index_of(&t, &["a", "a2"]);
    let cancel = Arc::new(AtomicBool::new(true));
    let opts = Options::default().cancel(cancel);
    assert!(matches!(find(&idx, &opts), Err(FindError::Cancelled)));
}

/// Simulates a full hash interrupted mid-file: raises the shared cancel flag
/// and fails the way `hash::full_cancellable` does when it sees the flag.
struct CancellingHasher {
    cancel: Arc<AtomicBool>,
}

impl CancellingHasher {
    fn interrupted(&self, m: &FileMeta) -> HashError {
        self.cancel.store(true, Ordering::SeqCst);
        HashError {
            op: "cancelled",
            path: m.path().to_path_buf(),
            source: std::io::Error::from(std::io::ErrorKind::Interrupted),
        }
    }
}

impl Hasher for CancellingHasher {
    fn partial(&self, m: &FileMeta) -> Result<u64, HashError> {
        DirectHasher.partial(m)
    }

    fn full(&self, m: &FileMeta) -> Result<Digest, HashError> {
        Err(self.interrupted(m))
    }

    fn full_cancellable(&self, m: &FileMeta, _cancel: &AtomicBool) -> Result<Digest, HashError> {
        Err(self.interrupted(m))
    }
}

#[test]
fn cancel_during_full_hash_is_not_reported_as_error() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd")]);
    let idx = index_of(&t, &["a", "a2"]);
    let cancel = Arc::new(AtomicBool::new(false));
    let errors = Mutex::new(Vec::<PathBuf>::new());
    // One worker, so the interrupted file is hashed first and deterministically.
    let opts = Options::default()
        .workers(1)
        .hasher(Arc::new(CancellingHasher {
            cancel: cancel.clone(),
        }))
        .cancel(cancel)
        .on_error(Box::new(|p, _| {
            errors.lock().unwrap().push(p.to_path_buf());
        }));

    let r = find(&idx, &opts);
    drop(opts);

    assert!(matches!(r, Err(FindError::Cancelled)), "{r:?}");
    assert_eq!(errors.into_inner().unwrap(), Vec::<PathBuf>::new());
}

#[test]
fn verify_stops_when_cancelled() {
    let t = Tree::build(&[("a", b"abcd"), ("a2", b"abcd"), ("a3", b"abcd")]);
    let idx = index_of(&t, &["a", "a2", "a3"]);
    let cancel = Arc::new(AtomicBool::new(false));
    let raise = cancel.clone();
    let errors = Mutex::new(Vec::<PathBuf>::new());
    let opts = Options::default()
        .verify(true)
        .cancel(cancel)
        .on_progress(Box::new(move |p| {
            if p.stage == Stage::Verify && p.done == 0 {
                raise.store(true, Ordering::SeqCst);
            }
        }))
        .on_error(Box::new(|p, _| {
            errors.lock().unwrap().push(p.to_path_buf());
        }));

    let r = find(&idx, &opts);
    drop(opts);

    assert!(matches!(r, Err(FindError::Cancelled)), "{r:?}");
    assert_eq!(errors.into_inner().unwrap(), Vec::<PathBuf>::new());
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

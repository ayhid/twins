//! Tests for the scan pipeline: parity with the stages wired by hand, the
//! dry-run flag derived from the run mode, event ordering and cancellation.

mod fixtures;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;
use std::time::SystemTime;

use fixtures::{Tree, mib};
use twins_core::group::{self, Index, Keeper, Strategy};
use twins_core::observe::{CancelToken, Event, NoopObserver, Observer, Outcome, Stage};
use twins_core::pipeline::{self, PipelineError, RunMode, ScanSpec};
use twins_core::report::{self, Meta};
use twins_core::scan;

/// Records every event in order.
#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<Event>>,
}

impl Recorder {
    fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }
}

impl Observer for Recorder {
    fn on_event(&self, event: &Event) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// Raises its token when the given stage starts, and records every event.
struct CancelOn {
    stage: Stage,
    token: CancelToken,
    seen: Mutex<Vec<Event>>,
}

impl CancelOn {
    fn new(stage: Stage, token: &CancelToken) -> Self {
        Self {
            stage,
            token: token.clone(),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<Event> {
        self.seen.lock().unwrap().clone()
    }
}

impl Observer for CancelOn {
    fn on_event(&self, event: &Event) {
        if matches!(event, Event::StageStarted { stage, .. } if *stage == self.stage) {
            self.token.cancel();
        }
        self.seen.lock().unwrap().push(event.clone());
    }
}

fn walk_options(t: &Tree) -> scan::Options {
    scan::Options::new(vec![t.root().into()]).home(t.root().into())
}

fn spec(t: &Tree) -> ScanSpec {
    ScanSpec::new(walk_options(t))
}

/// Two duplicate pairs of 1 MiB files with different content, a unique
/// 1 MiB file and two small files below the default minimum size.
fn duplicate_tree() -> Tree {
    let (one, two, three) = (mib(1), mib(2), mib(3));
    Tree::build(&[
        ("a.bin", &one),
        ("sub/b.bin", &one),
        ("c.bin", &two),
        ("sub/d.bin", &two),
        ("e.bin", &three),
        ("tiny1", b"tiny"),
        ("tiny2", b"tiny"),
    ])
}

fn is_finished(e: &Event, expected: Outcome) -> bool {
    matches!(e, Event::Finished { outcome } if *outcome == expected)
}

fn is_started(e: &Event, expected: Stage) -> bool {
    matches!(e, Event::StageStarted { stage, .. } if *stage == expected)
}

#[test]
fn pipeline_matches_manual_wiring() {
    let t = duplicate_tree();

    let idx = Index::new();
    let stats = scan::walk(&walk_options(&t), |m| idx.add(m), |_, _| {}).unwrap();
    let groups = group::find(&idx, &group::Options::default()).unwrap();
    let keeper = Keeper::new(Strategy::default(), None);
    let actions = group::plan(&groups, &keeper);
    let meta = Meta {
        roots: vec![t.root().into()],
        files: stats.files,
        candidates: stats.candidates,
        strategy: keeper.strategy(),
        dry_run: false,
    };
    let expected = report::build(&actions, &meta, SystemTime::UNIX_EPOCH);

    let outcome = pipeline::scan(&spec(&t), &NoopObserver, &CancelToken::new()).unwrap();

    assert_eq!(outcome.report(SystemTime::UNIX_EPOCH), expected);
    assert_eq!(outcome.stats().files, stats.files);
    assert_eq!(expected.groups.len(), 2);
}

#[test]
fn dry_run_in_meta_follows_run_mode() {
    let t = duplicate_tree();
    let run = |mode: RunMode| {
        pipeline::scan(&spec(&t).mode(mode), &NoopObserver, &CancelToken::new()).unwrap()
    };

    let scanned = run(RunMode::Scan);
    assert!(!scanned.report(SystemTime::UNIX_EPOCH).dry_run);
    assert!(!scanned.meta().dry_run);

    let simulated = run(RunMode::DryRun);
    assert!(simulated.report(SystemTime::UNIX_EPOCH).dry_run);
    assert!(simulated.meta().dry_run);
}

#[test]
fn default_run_mode_is_scan() {
    assert_eq!(RunMode::default(), RunMode::Scan);
    let t = duplicate_tree();
    let s = spec(&t);
    assert_eq!(s.run_mode(), RunMode::Scan);
    let outcome = pipeline::scan(&s, &NoopObserver, &CancelToken::new()).unwrap();
    assert!(!outcome.report(SystemTime::UNIX_EPOCH).dry_run);
}

#[test]
fn finished_is_the_last_event() {
    let t = duplicate_tree();
    let rec = Recorder::default();
    let outcome = pipeline::scan(&spec(&t), &rec, &CancelToken::new()).unwrap();
    let events = rec.events();

    assert!(matches!(
        events.first(),
        Some(Event::StageStarted {
            stage: Stage::Walk,
            step: 1,
            steps: 4
        })
    ));

    let c = outcome.stats().candidates;
    let sizing = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::StageStarted {
                    stage: Stage::SizeGrouping,
                    step: 2,
                    steps: 4
                }
            )
        })
        .expect("size grouping started");
    assert!(matches!(
        events.get(sizing + 1),
        Some(Event::Progress {
            stage: Stage::SizeGrouping,
            done,
            total: Some(total),
        }) if *done == c && *total == c
    ));

    let finished: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::Finished { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(finished, vec![events.len() - 1]);
    assert!(is_finished(events.last().unwrap(), Outcome::Completed));
}

#[test]
fn verify_raises_the_step_count_to_five() {
    let t = duplicate_tree();
    let rec = Recorder::default();
    let s = spec(&t).verify(true);
    assert_eq!(s.steps(), 5);
    pipeline::scan(&s, &rec, &CancelToken::new()).unwrap();

    assert!(matches!(
        rec.events().first(),
        Some(Event::StageStarted {
            stage: Stage::Walk,
            step: 1,
            steps: 5
        })
    ));
}

#[test]
fn cancel_at_walk_returns_cancelled() {
    let t = duplicate_tree();
    let token = CancelToken::new();
    let obs = CancelOn::new(Stage::Walk, &token);

    let err = pipeline::scan(&spec(&t), &obs, &token).unwrap_err();

    assert!(err.is_cancelled());
    let seen = obs.seen();
    assert!(!seen.iter().any(|e| is_started(e, Stage::SizeGrouping)));
    assert!(is_finished(seen.last().unwrap(), Outcome::Cancelled));
}

#[test]
fn cancel_between_stages_is_not_ignored() {
    // A single file: no size collision, so the hashing has nothing to do
    // and would return Ok without ever looking at the flag.
    let t = Tree::build(&[("only.bin", &mib(1))]);
    let token = CancelToken::new();
    let obs = CancelOn::new(Stage::SizeGrouping, &token);

    let err = pipeline::scan(&spec(&t), &obs, &token).unwrap_err();

    assert!(matches!(err, PipelineError::Cancelled));
    assert!(is_finished(obs.seen().last().unwrap(), Outcome::Cancelled));
}

#[test]
fn walk_errors_fail_without_cancelling() {
    let t = Tree::build(&[]);
    let missing = t.path("does-not-exist");
    let s = ScanSpec::new(scan::Options::new(vec![missing]).home(t.root().into()));
    let rec = Recorder::default();

    let err = pipeline::scan(&s, &rec, &CancelToken::new()).unwrap_err();

    assert!(matches!(err, PipelineError::Scan(_)));
    assert!(!err.is_cancelled());
    assert!(is_finished(rec.events().last().unwrap(), Outcome::Failed));
}

#[test]
fn unreadable_file_is_reported_as_file_skipped() {
    let one = mib(1);
    let t = Tree::build(&[("a.bin", &one), ("b.bin", &one)]);
    let locked = t.path("b.bin");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let restore = || fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    if fs::File::open(&locked).is_ok() {
        // Running as root: permissions do not stop the read.
        restore();
        return;
    }

    let rec = Recorder::default();
    let outcome = pipeline::scan(&spec(&t), &rec, &CancelToken::new()).unwrap();
    restore();

    assert_eq!(outcome.errors(), 1);
    let expected = locked.to_string_lossy();
    assert!(
        rec.events()
            .iter()
            .any(|e| matches!(e, Event::FileSkipped { path, .. } if *path == expected)),
        "no FileSkipped for {expected}"
    );
}

/// One duplicate pair of 1 MiB files, so both hashing stages have work.
fn pair_tree() -> Tree {
    let one = mib(1);
    Tree::build(&[("a.bin", &one), ("b.bin", &one)])
}

/// The `(stage, step, steps)` of every `StageStarted`, in order.
fn stages(seen: &[Event]) -> Vec<(Stage, u8, u8)> {
    seen.iter()
        .filter_map(|e| match e {
            Event::StageStarted { stage, step, steps } => Some((*stage, *step, *steps)),
            _ => None,
        })
        .collect()
}

#[test]
fn stage_events_in_order_without_verify() {
    let t = pair_tree();
    let rec = Recorder::default();
    pipeline::scan(&spec(&t), &rec, &CancelToken::new()).unwrap();
    let events = rec.events();

    assert_eq!(
        stages(&events),
        vec![
            (Stage::Walk, 1, 4),
            (Stage::SizeGrouping, 2, 4),
            (Stage::PartialHash, 3, 4),
            (Stage::FullHash, 4, 4),
        ]
    );
    assert!(is_finished(events.last().unwrap(), Outcome::Completed));
}

#[test]
fn stage_events_in_order_with_verify() {
    let t = pair_tree();
    let rec = Recorder::default();
    pipeline::scan(&spec(&t).verify(true), &rec, &CancelToken::new()).unwrap();
    let events = rec.events();

    assert_eq!(
        stages(&events),
        vec![
            (Stage::Walk, 1, 5),
            (Stage::SizeGrouping, 2, 5),
            (Stage::PartialHash, 3, 5),
            (Stage::FullHash, 4, 5),
            (Stage::Verify, 5, 5),
        ]
    );
    assert!(is_finished(events.last().unwrap(), Outcome::Completed));
}

#[test]
fn stage_events_on_empty_tree() {
    let t = Tree::build(&[]);
    let rec = Recorder::default();
    pipeline::scan(&spec(&t), &rec, &CancelToken::new()).unwrap();
    let events = rec.events();

    assert_eq!(
        stages(&events),
        vec![
            (Stage::Walk, 1, 4),
            (Stage::SizeGrouping, 2, 4),
            (Stage::PartialHash, 3, 4),
            (Stage::FullHash, 4, 4),
        ]
    );
    assert!(is_finished(events.last().unwrap(), Outcome::Completed));
}

#[test]
fn progress_events_follow_their_stage_start() {
    let t = pair_tree();
    let rec = Recorder::default();
    pipeline::scan(&spec(&t).verify(true), &rec, &CancelToken::new()).unwrap();
    let events = rec.events();

    let mut current: Option<Stage> = None;
    let mut progressed = Vec::new();
    for e in &events {
        match e {
            Event::StageStarted { stage, .. } => current = Some(*stage),
            Event::Progress { stage, .. } => {
                assert_eq!(Some(*stage), current, "{e:?} outside its stage");
                if progressed.last() != Some(stage) {
                    progressed.push(*stage);
                }
            }
            _ => {}
        }
    }
    // Every hashing stage had work, so each reported progress.
    for stage in [Stage::PartialHash, Stage::FullHash, Stage::Verify] {
        assert!(progressed.contains(&stage), "no progress for {stage}");
    }
}

/// Runs `spec` with a token raised when `stage` starts and checks the run
/// stopped there: cancelled error, no later stage, `Finished(Cancelled)`.
fn assert_cancelled_at(spec: &ScanSpec, stage: Stage) -> Vec<Event> {
    let token = CancelToken::new();
    let obs = CancelOn::new(stage, &token);

    let err = pipeline::scan(spec, &obs, &token).unwrap_err();

    assert!(err.is_cancelled(), "{err:?}");
    let seen = obs.seen();
    let started = stages(&seen);
    assert_eq!(
        started.last().map(|s| s.0),
        Some(stage),
        "stages after cancel: {started:?}"
    );
    assert!(is_finished(seen.last().unwrap(), Outcome::Cancelled));
    seen
}

#[test]
fn cancel_at_partial_hash_stops_before_full_hash() {
    let t = pair_tree();
    let seen = assert_cancelled_at(&spec(&t), Stage::PartialHash);
    assert!(!seen.iter().any(|e| is_started(e, Stage::FullHash)));
}

#[test]
fn cancel_at_full_hash_returns_cancelled() {
    let t = pair_tree();
    let seen = assert_cancelled_at(&spec(&t).verify(true), Stage::FullHash);
    assert!(!seen.iter().any(|e| is_started(e, Stage::Verify)));
}

#[test]
fn cancel_at_verify_returns_cancelled() {
    let t = pair_tree();
    assert_cancelled_at(&spec(&t).verify(true), Stage::Verify);
}

#[test]
fn walk_progress_counts_files_before_size_grouping() {
    // Five small files, all below the default minimum size: the walk sees
    // them while listing even though none becomes a candidate.
    let t = Tree::build(&[
        ("a", b"1"),
        ("b", b"22"),
        ("sub/c", b"333"),
        ("sub/d", b"4444"),
        ("sub/deeper/e", b"55555"),
    ]);
    let rec = Recorder::default();
    let outcome = pipeline::scan(&spec(&t), &rec, &CancelToken::new()).unwrap();
    let events = rec.events();

    let walk_start = events
        .iter()
        .position(|e| is_started(e, Stage::Walk))
        .expect("walk starts");
    let sizing = events
        .iter()
        .position(|e| is_started(e, Stage::SizeGrouping))
        .expect("size grouping starts");
    let walked: Vec<(usize, u64)> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Event::Progress {
                stage: Stage::Walk,
                done,
                total: None,
            } => Some((i, *done)),
            _ => None,
        })
        .collect();

    assert!(!walked.is_empty(), "no walk progress in {events:?}");
    for &(i, done) in &walked {
        assert!(
            walk_start < i && i < sizing,
            "walk progress {done} at {i} outside ({walk_start}, {sizing})"
        );
    }
    let counts: Vec<u64> = walked.iter().map(|&(_, done)| done).collect();
    assert!(
        counts.windows(2).all(|w| w[0] <= w[1]),
        "walk progress went backwards: {counts:?}"
    );
    assert_eq!(counts.last().copied(), Some(outcome.stats().files));
    assert_eq!(outcome.stats().files, 5);
}

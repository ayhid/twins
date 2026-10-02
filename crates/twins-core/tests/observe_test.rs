//! Tests for the observer contract: the shared cancellation flag, the event
//! JSON shape, the progress throttle and the stage sequence it preserves.

mod fixtures;

use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

use fixtures::{Tree, mib};
use serde_json::json;
use twins_core::observe::{CancelToken, Event, Observer, Outcome, Stage, Throttle};
use twins_core::pipeline::{self, ScanSpec};
use twins_core::scan;

/// An interval no test run will ever reach.
const HOUR: Duration = Duration::from_secs(3600);

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

fn started(stage: Stage, step: u8, steps: u8) -> Event {
    Event::StageStarted { stage, step, steps }
}

fn progress(stage: Stage, done: u64, total: Option<u64>) -> Event {
    Event::Progress { stage, done, total }
}

/// `(stage, done)` of every forwarded `Progress`, in order.
fn progress_seen(events: &[Event]) -> Vec<(Stage, u64)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Progress { stage, done, .. } => Some((*stage, *done)),
            _ => None,
        })
        .collect()
}

/// `(stage, step, steps)` of every `StageStarted`, in order.
fn stages_seen(events: &[Event]) -> Vec<(Stage, u8, u8)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::StageStarted { stage, step, steps } => Some((*stage, *step, *steps)),
            _ => None,
        })
        .collect()
}

#[test]
fn cancel_token_clones_share_the_flag() {
    let token = CancelToken::new();
    assert!(!token.is_cancelled());

    let clone = token.clone();
    clone.cancel();
    assert!(token.is_cancelled());

    let other = CancelToken::new();
    other.flag().store(true, Ordering::SeqCst);
    assert!(other.is_cancelled());
}

#[test]
fn event_json_uses_camel_case_tags() {
    let to = |e: &Event| serde_json::to_value(e).unwrap();

    assert_eq!(
        to(&started(Stage::PartialHash, 3, 4)),
        json!({"type": "stageStarted", "stage": "partialHash", "step": 3, "steps": 4})
    );
    assert_eq!(
        to(&progress(Stage::Walk, 5, None)),
        json!({"type": "progress", "stage": "walk", "done": 5, "total": null})
    );
    assert_eq!(
        to(&progress(Stage::SizeGrouping, 2, Some(2)))["stage"],
        "sizeGrouping"
    );
    assert_eq!(
        to(&Event::FileSkipped {
            path: "/x".into(),
            reason: "r".into(),
        }),
        json!({"type": "fileSkipped", "path": "/x", "reason": "r"})
    );
    assert_eq!(
        to(&Event::Finished {
            outcome: Outcome::Cancelled,
        }),
        json!({"type": "finished", "outcome": "cancelled"})
    );
    assert_eq!(serde_json::to_value(Stage::FullHash).unwrap(), "fullHash");
}

#[test]
fn throttle_forwards_non_progress_events() {
    let events = vec![
        started(Stage::Walk, 1, 4),
        Event::FileSkipped {
            path: "/x".into(),
            reason: "r".into(),
        },
        Event::Finished {
            outcome: Outcome::Completed,
        },
    ];
    let throttle = Throttle::new(Recorder::default(), HOUR);
    for e in &events {
        throttle.on_event(e);
    }
    assert_eq!(throttle.inner().events(), events);
}

#[test]
fn throttle_limits_progress() {
    let throttle = Throttle::new(Recorder::default(), HOUR);
    throttle.on_event(&started(Stage::PartialHash, 3, 4));
    for done in 1..=100 {
        throttle.on_event(&progress(Stage::PartialHash, done, Some(1000)));
    }
    assert_eq!(
        progress_seen(&throttle.into_inner().events()),
        vec![(Stage::PartialHash, 1)]
    );
}

#[test]
fn throttle_forwards_final_progress() {
    let throttle = Throttle::new(Recorder::default(), HOUR);
    for done in 1..=10 {
        throttle.on_event(&progress(Stage::FullHash, done, Some(10)));
    }
    assert_eq!(
        progress_seen(&throttle.into_inner().events()),
        vec![(Stage::FullHash, 1), (Stage::FullHash, 10)]
    );
}

#[test]
fn throttle_reopens_on_stage_start() {
    let throttle = Throttle::new(Recorder::default(), HOUR);
    throttle.on_event(&progress(Stage::PartialHash, 1, Some(5)));
    throttle.on_event(&progress(Stage::PartialHash, 2, Some(5)));
    throttle.on_event(&started(Stage::FullHash, 4, 4));
    throttle.on_event(&progress(Stage::FullHash, 1, Some(5)));
    assert_eq!(
        throttle.into_inner().events(),
        vec![
            progress(Stage::PartialHash, 1, Some(5)),
            started(Stage::FullHash, 4, 4),
            progress(Stage::FullHash, 1, Some(5)),
        ]
    );
}

#[test]
fn throttle_zero_interval_forwards_everything() {
    let throttle = Throttle::new(Recorder::default(), Duration::ZERO);
    for done in 1..=100 {
        throttle.on_event(&progress(Stage::Walk, done, None));
    }
    assert_eq!(progress_seen(&throttle.into_inner().events()).len(), 100);
}

#[test]
fn throttled_observer_sees_the_same_stage_sequence() {
    let (one, two) = (mib(1), mib(2));
    let t = Tree::build(&[("a.bin", &one), ("sub/b.bin", &one), ("c.bin", &two)]);
    let spec = ScanSpec::new(scan::Options::new(vec![t.root().into()]).home(t.root().into()));

    let plain = Recorder::default();
    pipeline::scan(&spec, &plain, &CancelToken::new()).unwrap();
    let throttled = Throttle::new(Recorder::default(), HOUR);
    pipeline::scan(&spec, &throttled, &CancelToken::new()).unwrap();

    let plain = plain.events();
    let throttled = throttled.into_inner().events();
    assert!(!stages_seen(&plain).is_empty());
    assert_eq!(stages_seen(&throttled), stages_seen(&plain));
    let completed = Event::Finished {
        outcome: Outcome::Completed,
    };
    assert_eq!(plain.last(), Some(&completed));
    assert_eq!(throttled.last(), Some(&completed));
}

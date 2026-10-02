//! Terminal rendering of pipeline events.
//!
//! One stderr line, redrawn in place, shows the current stage with a step
//! counter and digit-grouped counts, such as `[1/4] walk  48 210 files` or
//! `[3/4] partial hash  1 234 / 5 000` (D-06). It is drawn whenever stderr is
//! a terminal, also with `--json` (D-09), and cleared on `Finished`, so it
//! never lingers after success, failure or cancel (D-04). With `--verbose`,
//! every skipped file is listed as `skip <path>: <reason>`.

use std::io::Write;
use std::sync::Mutex;

use twins_core::observe::{Event, Observer, Stage};

/// Renders one progress line. Placeholder: renders nothing yet.
pub(crate) fn line(
    _step: u8,
    _steps: u8,
    _stage: Stage,
    _progress: Option<(u64, Option<u64>)>,
) -> String {
    String::new()
}

/// Observer that draws the progress line and the verbose skip lines.
pub(crate) struct TerminalObserver<W> {
    progress: bool,
    verbose: bool,
    out: Mutex<W>,
    current: Mutex<Option<(u8, u8, Stage)>>,
}

impl<W> TerminalObserver<W> {
    /// Observer writing to `out`. `progress` draws the line, `verbose` lists
    /// skipped files.
    pub(crate) fn new(out: W, progress: bool, verbose: bool) -> Self {
        Self {
            progress,
            verbose,
            out: Mutex::new(out),
            current: Mutex::new(None),
        }
    }
}

impl TerminalObserver<std::io::Stderr> {
    /// Observer writing to stderr.
    pub(crate) fn stderr(progress: bool, verbose: bool) -> Self {
        Self::new(std::io::stderr(), progress, verbose)
    }
}

impl<W: Write + Send> Observer for TerminalObserver<W> {
    fn on_event(&self, _event: &Event) {}
}

#[cfg(test)]
mod tests {
    use twins_core::observe::Outcome;

    use super::*;

    fn written(obs: TerminalObserver<Vec<u8>>) -> String {
        String::from_utf8(obs.out.into_inner().unwrap()).unwrap()
    }

    #[test]
    fn line_formats_walk_count() {
        assert_eq!(
            line(1, 4, Stage::Walk, Some((48_210, None))),
            "[1/4] walk  48 210 files"
        );
    }

    #[test]
    fn line_formats_hash_progress() {
        assert_eq!(
            line(3, 4, Stage::PartialHash, Some((1_234, Some(5_000)))),
            "[3/4] partial hash  1 234 / 5 000"
        );
    }

    #[test]
    fn line_formats_size_grouping() {
        assert_eq!(
            line(2, 4, Stage::SizeGrouping, Some((5_000, Some(5_000)))),
            "[2/4] size grouping  5 000 candidates"
        );
    }

    #[test]
    fn line_shows_label_before_first_progress() {
        assert_eq!(line(5, 5, Stage::Verify, None), "[5/5] verify");
    }

    #[test]
    fn renders_progress_and_clears_on_finish() {
        let obs = TerminalObserver::new(Vec::new(), true, false);
        obs.on_event(&Event::StageStarted {
            stage: Stage::Walk,
            step: 1,
            steps: 4,
        });
        obs.on_event(&Event::Progress {
            stage: Stage::Walk,
            done: 3,
            total: None,
        });
        obs.on_event(&Event::Finished {
            outcome: Outcome::Completed,
        });
        assert_eq!(
            written(obs),
            "\r\x1b[K[1/4] walk\r\x1b[K[1/4] walk  3 files\r\x1b[K"
        );
    }

    #[test]
    fn verbose_skips_print_without_a_terminal() {
        let obs = TerminalObserver::new(Vec::new(), false, true);
        obs.on_event(&Event::StageStarted {
            stage: Stage::Walk,
            step: 1,
            steps: 4,
        });
        obs.on_event(&Event::FileSkipped {
            path: "/x".to_owned(),
            reason: "r".to_owned(),
        });
        obs.on_event(&Event::Finished {
            outcome: Outcome::Completed,
        });
        assert_eq!(written(obs), "skip /x: r\n");
    }

    #[test]
    fn silent_without_terminal_or_verbose() {
        let obs = TerminalObserver::new(Vec::new(), false, false);
        obs.on_event(&Event::StageStarted {
            stage: Stage::Walk,
            step: 1,
            steps: 4,
        });
        obs.on_event(&Event::Progress {
            stage: Stage::Walk,
            done: 3,
            total: None,
        });
        obs.on_event(&Event::FileSkipped {
            path: "/x".to_owned(),
            reason: "r".to_owned(),
        });
        obs.on_event(&Event::Finished {
            outcome: Outcome::Completed,
        });
        assert_eq!(written(obs), "");
    }
}

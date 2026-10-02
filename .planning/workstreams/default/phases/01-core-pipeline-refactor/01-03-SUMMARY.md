---
phase: 01-core-pipeline-refactor
plan: 03
subsystem: cli
tags: [rust, observer, progress, throttle, atomics, terminal]

requires:
  - phase: 01-core-pipeline-refactor (plan 01-01)
    provides: "twins_core::observe (Event, Stage, Observer, CancelToken) and pipeline::scan"
provides:
  - "twins_core::observe::Throttle<O>: lock-free 80 ms Progress rate limiter that never drops StageStarted, FileSkipped or Finished"
  - "twins_core::human::group_digits(u64) -> String (ASCII-space grouping)"
  - "crates/twins-cli/src/progress.rs: TerminalObserver<W> and the pure line() formatter"
  - "pipeline walk stage emits Progress { Walk, done, total: None } per candidate"
affects: [01-02, 01-07, phase-05-tauri-app]

actuals:
  tokens: 6100
  tasks: 2
  commits: 5

tech-stack:
  added: []
  patterns:
    - "Shell observers wrap their renderer in Throttle::new(.., 80 ms); only Progress is ever throttled"
    - "TerminalObserver holds its writer lock for a whole event so lines never interleave; generic over W: Write for unit tests"

key-files:
  created:
    - crates/twins-cli/src/progress.rs
  modified:
    - crates/twins-core/src/observe.rs
    - crates/twins-core/src/human.rs
    - crates/twins-core/src/pipeline.rs
    - crates/twins-core/tests/observe_test.rs
    - crates/twins-cli/src/main.rs
    - crates/twins-cli/src/run.rs
    - crates/twins-cli/tests/cli_test.rs

key-decisions:
  - "Throttle keeps one AtomicU64 (nanoseconds since a base Instant) with u64::MAX as the 'next Progress passes' sentinel; elapsed time is clamped to u64::MAX - 1 so it never collides with the sentinel"
  - "A Progress with done == total always passes and restamps the clock; StageStarted reopens the gate"
  - "group_digits uses is_multiple_of instead of % 3 == 0 (MSRV 1.90, clippy manual_is_multiple_of)"
  - "Walk progress is emitted by the pipeline per candidate and left to Throttle to rate-limit, matching the pre-refactor behaviour of ticking per walked file"

patterns-established:
  - "Progress rendering: [k/N] label, two spaces, then 'N files' (no total), 'N candidates' (size grouping) or 'done / total'"
  - "Clear the line (\\r\\x1b[K) before every verbose skip line and on Finished"

requirements-completed: [CORE-01]

coverage:
  - id: D1
    description: "Lock-free Throttle forwards structural events untouched and rate-limits Progress (first after StageStarted, final done == total, else once per interval)"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/observe_test.rs#throttle_forwards_non_progress_events, throttle_limits_progress, throttle_forwards_final_progress, throttle_reopens_on_stage_start, throttle_zero_interval_forwards_everything"
        status: pass
      - kind: integration
        ref: "crates/twins-core/tests/observe_test.rs#throttled_observer_sees_the_same_stage_sequence"
        status: pass
    human_judgment: false
  - id: D2
    description: "group_digits renders counts with ASCII spaces every three digits"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/human_test.rs#group_digits_inserts_spaces_every_three_digits"
        status: pass
    human_judgment: false
  - id: D3
    description: "TerminalObserver formats '[k/N] label  n / m', redraws in place, clears on Finished, keeps verbose skip lines, stays silent without a terminal or --verbose"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-cli/src/progress.rs#tests (7 tests: line_* x4, renders_progress_and_clears_on_finish, verbose_skips_print_without_a_terminal, silent_without_terminal_or_verbose)"
        status: pass
    human_judgment: false
  - id: D4
    description: "twins scan --json writes nothing to stderr when stderr is not a terminal and stdout stays parseable JSON"
    requirement: CORE-01
    verification:
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#json_stderr_is_silent_when_not_a_terminal"
        status: pass
      - kind: integration
        ref: "cargo test -p twins-cli --test cli_test characterize (2 passed)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Walk stage reports a running file count so the terminal shows '[1/4] walk  N files'"
    requirement: CORE-01
    verification:
      - kind: integration
        ref: "crates/twins-core/tests/observe_test.rs#walk_reports_a_running_file_count"
        status: pass
    human_judgment: false
  - id: D6
    description: "In a real terminal the single progress line updates through all four (or five with --verify) stages, digit groups look right, the line is cleared before the report, progress also shows with --json, and the 'N candidates' wording is acceptable (research A6)"
    requirement: CORE-01
    verification:
      - kind: manual_procedural
        ref: "script -q /dev/null ./target/debug/twins scan --json crates (pty smoke test: walk and size-grouping lines drawn then cleared before the JSON)"
        status: pass
    human_judgment: true
    rationale: "Visual in-place redraw on a real TTY, and the hashing-stage lines ([3/4], [4/4], [5/5]) only appear once plan 01-02's StageStarted mapping is merged; the plan's human-check covers this"

duration: 7min
completed: 2026-10-02
status: complete
plan_head_before: 71bdc8ed9a25f049020649d23264eaa0ea50a0cf
plan_head_after: 5bef5856ec356e605ba521f727972c08da6505a9
---

# Phase 1 Plan 03: Terminal progress from core events Summary

**Lock-free `Throttle<O>` (AtomicU64 + compare_exchange, 80 ms) and `group_digits` in core, plus a CLI `TerminalObserver` that redraws `[k/N] label  n / m` on stderr whenever it is a terminal (also with `--json`) and clears it on `Finished`**

## Performance

- **Duration:** about 7 min this session (Task 1 RED `cd48f49` was committed in an earlier session that halted at the TDD gate)
- **Started:** 2026-10-02T20:16:56Z
- **Completed:** 2026-10-02T20:24:00Z
- **Tasks:** 2
- **Files modified:** 8 (1 created, 7 modified)

## Accomplishments

- `Throttle<O>` in `twins_core::observe`. It passes the first Progress after each StageStarted, every Progress that completes its stage (`done == total`), and otherwise at most one Progress per interval. StageStarted, FileSkipped and Finished are never dropped, delayed or reordered. The check is lock-free (`grep Mutex observe.rs` prints nothing).
- `human::group_digits`: `48210` becomes `48 210` and `u64::MAX` becomes `18 446 744 073 709 551 615`.
- `crates/twins-cli/src/progress.rs`:
  - a pure `line()` formatter;
  - `TerminalObserver<W>`, which redraws the line with `\r\x1b[K`, ignores Progress for a stage that is not current, clears the line before each verbose `skip <path>: <reason>` and clears it on `Finished`, so success, error and cancel all leave a clean line;
  - none of it writes to stdout.
- `run.rs` now uses `Throttle::new(TerminalObserver::stderr(stderr().is_terminal(), verbose), 80 ms)`. Progress depends only on stderr being a terminal (D-09). `SkipPrinter` is removed.
- The pipeline's walk stage reports a running file count again, so `[1/4] walk  N files` shows up as it did before the refactor.

## Task Commits

1. **Task 1 RED (earlier session): failing tests for Throttle and digit grouping**: `cd48f49` (test)
2. **Task 1 GREEN: Throttle and digit grouping in core**: `1c429d1` (feat)
3. **Task 2 RED: failing tests for the terminal progress observer**: `3a69f33` (test)
4. **Task 2 GREEN: staged progress from core events in the terminal**: `f674e8a` (feat)
5. **Deviation RED: failing test for the walk file count**: `48d3ebe` (test)
6. **Deviation GREEN: walk file count reported as Progress events**: `5bef585` (fix)

**Plan metadata:** recorded in the docs commit that adds this SUMMARY.

## TDD Gate Compliance

- Task 1: RED `cd48f49` came before GREEN `1c429d1`. Before GREEN I re-ran the tests. They still failed on assertions: `group_digits` gave `"1000"` where `"1 000"` was expected, and `throttle_limits_progress`, `throttle_forwards_final_progress` and `throttle_reopens_on_stage_start` failed on their assertions.
- Task 2: RED `3a69f33` came before GREEN `f674e8a`. Six of the seven progress unit tests failed on assertions against compile-only stubs. I converted the `cargo test` output to TAP and checked it with `check tdd-red-evidence`. The verdict was `RED_EVIDENCE_OK` (target `progress::tests::renders_progress_and_clears_on_finish`, exit 101).
  - `json_stderr_is_silent_when_not_a_terminal` already passed at RED. It is a guard that the new observer must not break: before this plan, stderr was already silent in that mode.
- Walk deviation: RED `48d3ebe` came before GREEN `5bef585`. It failed with `left: None, right: Some(3)`.
- REFACTOR: none needed.

## Files Created/Modified

- `crates/twins-core/src/observe.rs`: `Throttle<O>` with `new`, `inner`, `into_inner` and `impl Observer`.
- `crates/twins-core/src/human.rs`: `group_digits`. The module doc now says "byte sizes and counts".
- `crates/twins-core/src/pipeline.rs`: `walk_stage` emits `Progress { Walk, done, total: None }` once per candidate. This is a deviation, see below.
- `crates/twins-core/tests/observe_test.rs`: adds `walk_reports_a_running_file_count`. The 8 tests from the earlier RED commit are unchanged.
- `crates/twins-cli/src/progress.rs`: new. `line()`, `TerminalObserver` and 7 unit tests.
- `crates/twins-cli/src/main.rs`: `mod progress;`.
- `crates/twins-cli/src/run.rs`: the observer is built from Throttle and TerminalObserver; `SkipPrinter` is deleted.
- `crates/twins-cli/tests/cli_test.rs`: `json_stderr_is_silent_when_not_a_terminal`.

## Decisions Made

- The Throttle sentinel is `u64::MAX`, and elapsed nanoseconds are clamped to `u64::MAX - 1`, so a real timestamp can never look like "open".
- A stage-final Progress restamps the clock with a plain store instead of a CAS. It must pass whatever happens, so racing for the slot is pointless.
- `group_digits` uses `is_multiple_of`, which follows the plan's no-`% n == 0` rule for MSRV 1.90.
- `TerminalObserver` always locks `out` before `current`, so the two locks cannot deadlock. A poisoned lock is recovered with `PoisonError::into_inner`, so a panic in one worker cannot silence the progress output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The walk stage emitted no Progress, so `[1/4] walk  N files` never appeared**
- **Found during:** Task 2, in a pty smoke test (`script -q /dev/null twins scan --min-size 1 target`). The walk line showed only `[1/4] walk`.
- **Issue:** Plan 01-01 moved the walk into `pipeline::walk_stage`, but the per-file count tick that the old `run.rs` had was not carried over. This plan's must-have truth requires `'[1/4] walk  48 210 files'`, and the plan's interface notes assumed walk events already existed.
- **Fix:** `walk_stage`'s visitor now increments an `AtomicU64` and emits `Progress { stage: Walk, done, total: None }` for each candidate. Throttle rate-limits it.
- **Files modified:** `crates/twins-core/src/pipeline.rs`. This file is outside this plan's `files_modified`. The change is a single hunk inside `walk_stage` and does not touch the hashing-stage `on_progress` closure that plan 01-02 owns. `git merge-tree --write-tree HEAD 4fe140e` (01-02's feat commit) merges with no conflict.
- **Verification:** the new `walk_reports_a_running_file_count` test checks that walk Progress events fall between Walk start and SizeGrouping start, that `total` is None, and that the highest `done` is 3. The full workspace suite stays green apart from the two 01-02 RED tests listed below.
- **Committed in:** `48d3ebe` (test) and `5bef585` (fix)

---

**Total deviations:** 1 auto-fixed (Rule 1 bug).
**Impact on plan:** without it the plan's first must-have truth would fail for the walk stage. No scope creep.

## Issues Encountered

- `cargo test --workspace` currently reports two failures in `group_test.rs`: `empty_index_still_marks_hash_stages` and `each_stage_starts_with_a_zero_done_marker`. Both are plan 01-02's RED tests (`9e7d53b`), already in the base commit, and 01-02 is making them pass in its own worktree. They are out of scope here. Every other test passes, and fmt and clippy (`-D warnings`) are clean.
- Until 01-02 merges, the pipeline emits no StageStarted for the hashing stages, so `[3/4] partial hash`, `[4/4] full hash` and `[5/5] verify` do not appear in a real terminal yet. `TerminalObserver` ignores Progress for a stage that is not current, so nothing wrong is drawn in the meantime. The renderer for those lines is unit-tested.
- The pre-commit plan ledger could not be written under `.git/worktrees/...` (the isolation hook refused it). The base SHA is recorded above as `plan_head_before`.

## Known Stubs

None. The compile-only stubs from the RED commits are all replaced.

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness

- Plan 01-07 (cancel message) can rely on `Finished` clearing the line before it prints `scan cancelled`.
- Phase 5 (Tauri) can reuse `Throttle` around its Channel observer.
- Still open: the plan's human check on a real TTY (all stages, `--verify`, `--json`, and the `N candidates` wording) after 01-02 merges.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*

## Self-Check: PASSED

- FOUND: crates/twins-cli/src/progress.rs, crates/twins-core/src/observe.rs (Throttle), crates/twins-core/src/human.rs (group_digits)
- FOUND commits: cd48f49, 1c429d1, 3a69f33, f674e8a, 48d3ebe, 5bef585

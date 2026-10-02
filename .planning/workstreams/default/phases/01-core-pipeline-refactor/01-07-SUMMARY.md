---
phase: 01-core-pipeline-refactor
plan: 07
subsystem: cli
tags: [rust, signal-hook, sigint, cancel, exit-codes, msrv]

requires:
  - phase: 01-core-pipeline-refactor (plan 01-01)
    provides: "CancelToken::flag(), PipelineError::is_cancelled(), run::exit_code mapping Cancelled to 130"
  - phase: 01-core-pipeline-refactor (plan 01-03)
    provides: "TerminalObserver clears the progress line on Event::Finished"
  - phase: 01-core-pipeline-refactor (plans 01-05, 01-06)
    provides: "walk and hashing stop within one stat / one 256 KiB chunk of a cancel"
provides:
  - "Ctrl+C in twins scan / twins report raises the core CancelToken; a cancelled run prints only 'scan cancelled' on stderr and exits 130"
  - "A second Ctrl+C exits 130 at once (signal-hook conditional shutdown)"
  - "run::is_cancelled(&anyhow::Error) for the CLI's exit path"
  - "Ignored real-signal tests sigint_cancels_scan_with_exit_130 and double_sigint_exits_130_immediately"
affects: [phase-02-journal, phase-05-tauri-app, agent-runner]

actuals:
  tokens: 2340
  tasks: 2
  commits: 3

tech-stack:
  added: ["signal-hook 0.4.4 (default-features = false, twins-cli only; pulls signal-hook-registry 1.4.8)"]
  patterns:
    - "Only a shell turns OS signals into a CancelToken; twins-core never installs handlers"
    - "Double Ctrl+C: register_conditional_shutdown(SIGINT, 130, flag) first, then register(SIGINT, flag)"
    - "main matches run::is_cancelled(&err) before the generic 'twins: ...' error arm"

key-files:
  created: []
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/twins-cli/Cargo.toml
    - crates/twins-cli/src/run.rs
    - crates/twins-cli/src/main.rs
    - crates/twins-cli/tests/cli_test.rs

key-decisions:
  - "SIGINT handlers are installed per scan, right after the CancelToken is created and before pipeline::scan, so `twins version` and argument errors keep the default SIGINT action"
  - "The cancel message has no 'twins: ' prefix and goes through eprintln! after the observer already cleared the line on Finished"
  - "Real-signal tests use std::process::Command with env!(\"CARGO_BIN_EXE_twins\") and piped output, so stderr is not a TTY and the expected stderr is exactly 'scan cancelled\\n'"

patterns-established:
  - "Signal-driven CLI tests are #[ignore] with a reason string and run at the phase gate: cargo test -p twins-cli --test cli_test -- --ignored sigint"

requirements-completed: [CORE-02, CORE-04]

coverage:
  - id: D1
    description: "run::is_cancelled recognises PipelineError::Cancelled and rejects scan errors and plain errors"
    requirement: CORE-02
    verification:
      - kind: unit
        ref: "crates/twins-cli/src/run.rs#tests::is_cancelled_detects_pipeline_cancellation"
        status: pass
      - kind: unit
        ref: "crates/twins-cli/src/run.rs#tests::exit_code_maps_cancel_to_130, exit_code_maps_scan_errors_to_2, exit_code_maps_other_errors_to_1"
        status: pass
    human_judgment: false
  - id: D2
    description: "One SIGINT during a scan stops it within 2 s, exits 130, writes nothing to stdout and exactly 'scan cancelled' to stderr"
    requirement: CORE-02
    verification:
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#sigint_cancels_scan_with_exit_130 (cargo test -p twins-cli --test cli_test -- --ignored sigint, 4 runs, all pass)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Two SIGINTs 20 ms apart exit 130 within 1 s, never by the default SIGINT action"
    requirement: CORE-02
    verification:
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#double_sigint_exits_130_immediately"
        status: pass
    human_judgment: false
  - id: D4
    description: "signal-hook lives only in twins-cli; twins-core has no signal-hook, ctrlc or tokio, and twins-cli has no unsafe code"
    requirement: CORE-02
    verification:
      - kind: other
        ref: "cargo tree -p twins-core -e normal --prefix none | grep -E '^(signal-hook|ctrlc|tokio) ' (no match)"
        status: pass
      - kind: other
        ref: "grep -rn unsafe crates/twins-cli/src (no match)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Usage errors still exit 2 with their messages"
    requirement: CORE-02
    verification:
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#usage_errors_exit_2_with_the_same_message"
        status: pass
    human_judgment: false
  - id: D6
    description: "Phase gate green on stable (fmt, clippy -D warnings, 112 tests) and on Rust 1.90.0 with RUSTFLAGS=-D warnings"
    requirement: CORE-04
    verification:
      - kind: other
        ref: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"-D warnings\" cargo +1.90.0 test --workspace (exit 0, no FAILED)"
        status: pass
    human_judgment: false
  - id: D7
    description: "In a real terminal, Ctrl+C clears the progress line before 'scan cancelled'; 'twins report > file' leaves the file empty; double Ctrl+C on a slow external volume exits at once"
    requirement: CORE-02
    verification: []
    human_judgment: true
    rationale: "Needs an interactive TTY and a slow external volume; the automated tests pipe stderr, so the in-place line clear and keyboard Ctrl+C are only checked by the plan's human-check"

duration: 4min
completed: 2026-10-02
status: complete
plan_head_before: a8555b75c1df6f6ea5cc9d5532e087073f62e0b4
plan_head_after: a5d9a3a51b174c5df3ef99edaacf61506bf42e8a
---

# Phase 1 Plan 07: Ctrl+C cancels the scan Summary

**signal-hook in the CLI only: the first Ctrl+C raises the core CancelToken and the run ends with `scan cancelled` on stderr, empty stdout and exit 130; a second Ctrl+C exits 130 at once. The phase gate is green on stable and on Rust 1.90.0.**

## Performance

- **Duration:** about 4 min
- **Started:** 2026-10-02T20:34:01Z
- **Completed:** 2026-10-02T20:38:03Z
- **Tasks:** 2
- **Files modified:** 6 (plus REQUIREMENTS.md in the metadata commit)

## Accomplishments

- Added `signal-hook = { version = "0.4.4", default-features = false }` to the workspace and to `twins-cli` only. Cargo.lock gains `signal-hook 0.4.4` and `signal-hook-registry 1.4.8`. `cargo tree -p twins-core` shows no signal-hook, ctrlc or tokio.
- `run.rs`:
  - New private `install_sigint(&CancelToken)`. It calls `register_conditional_shutdown(SIGINT, 130, cancel.flag())` first and `register(SIGINT, cancel.flag())` second, each with the context "cannot install the Ctrl+C handler". It runs in `scan()` right after the token is created.
  - New public `is_cancelled(&anyhow::Error) -> bool`.
  - Both signal-hook functions are safe, so there is no `unsafe` in twins-cli.
- `main.rs`: a new `Err(err) if run::is_cancelled(&err)` arm prints `scan cancelled` (no `twins: ` prefix) and returns `ExitCode::from(130)`. Usage errors still exit 2 and other failures exit 1.
- Two `#[ignore]` real-signal tests run on 32 sparse 1 GiB files. With one SIGINT the scan stopped within the 2 s bound with exit 130, empty stdout and stderr `scan cancelled\n`. With two SIGINTs 20 ms apart the process exited 130 within 1 s. Both tests finished in about 0.77 s together, and passed in 4 out of 4 runs.

## Task Commits

1. **Task 1 RED: failing test for is_cancelled**: `ba0c413` (test)
2. **Task 1 GREEN: cancel a scan with Ctrl+C**: `707f32e` (feat)
3. **Task 2: cover Ctrl+C with real signals**: `a5d9a3a` (test)

**Plan metadata:** the docs commit that adds this SUMMARY and REQUIREMENTS.md.

## TDD Gate Compliance

- Task 1: RED `ba0c413` came before GREEN `707f32e`.
  - At RED, `is_cancelled` was a stub that returned `false`. `cargo test -p twins-cli --bin twins` exited 101: 10 tests passed and 1 failed. The failing test was `run::tests::is_cancelled_detects_pipeline_cancellation`, on `assertion failed: is_cancelled(&anyhow::Error::from(PipelineError::Cancelled))`.
  - I converted the `cargo test` result lines to TAP and checked them with `check tdd-red-evidence`. The verdict was `RED_EVIDENCE_OK` (reason `target_test_failed`).
  - After GREEN, all 11 tests pass.
- The RED commit only touches `crates/twins-cli/src/run.rs`, because the plan puts this unit test in the `#[cfg(test)]` module. This is the known Rust gap (#4379): a path-based RED-commit gate cannot see in-file `#[test]` changes. The evidence above is the proof that RED really failed.
- REFACTOR: none needed.
- Task 2 is `type="auto"` with no `tdd` flag. It adds tests only.

## Files Created/Modified

- `Cargo.toml`: new workspace dependency `signal-hook`, between serde_json and tempfile.
- `Cargo.lock`: adds signal-hook and signal-hook-registry. `errno` and `libc` were already locked.
- `crates/twins-cli/Cargo.toml`: `signal-hook.workspace = true`.
- `crates/twins-cli/src/run.rs`: `install_sigint`, `is_cancelled` and the unit test.
- `crates/twins-cli/src/main.rs`: the cancelled arm.
- `crates/twins-cli/tests/cli_test.rs`:
  - helpers `big_sparse_tree`, `spawn_busy_scan` and `send_sigint`;
  - tests `sigint_cancels_scan_with_exit_130` and `double_sigint_exits_130_immediately`.

## Decisions Made

- The handlers are installed inside `run::scan`, not in `main`. So only scan and report runs change the SIGINT behaviour. Ctrl+C before installation, or during `twins version`, still uses the default action, which is harmless because no work has started.
- The `#[ignore]` attribute carries a reason string. The comment above each test gives the command for running it at the phase gate.

## Deviations from Plan

None. The plan was executed as written.

## Issues Encountered

- The worktree isolation hook refused compound shell commands, such as the plan's `bash -o pipefail -c "cargo tree ... | { ! grep ...; }"` gate. I ran the same check in two steps with the same result: `cargo tree` exited 0, and the grep for `^(signal-hook|ctrlc|tokio) ` found nothing.
- I could not write the plan commit ledger under `.git/worktrees/...`. The base SHA is recorded as `plan_head_before`.

## Known Stubs

None. The RED stub of `is_cancelled` was replaced in `707f32e`.

## Threat Flags

None. The only new surface is the SIGINT handler, which threat-model entries T-01-14 to T-01-17 already cover:
- T-01-14 (no partial report on stdout) is tested.
- T-01-15 (no hand-written signal handler) holds: there is no unsafe code and only signal-hook's safe functions are used.
- T-01-16 (core must not install signal handlers) holds: the cargo tree gate passes.
- T-01-17 (cleanup skipped on the second Ctrl+C) is accepted as planned.

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness

- Phase 1 is complete in code: every plan in the phase now has a SUMMARY, and the gate is green on stable and on 1.90.0.
- The end-of-phase human check (interactive TTY Ctrl+C, empty redirected report, double Ctrl+C on a slow volume) is still open. It is coverage item D7.
- When Phase 2 adds journaling, revisit T-01-17: the second-Ctrl+C `_exit` skips cleanup, and a journal may need flushing first.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*

## Self-Check: PASSED

- FOUND: crates/twins-cli/src/run.rs (install_sigint, is_cancelled), crates/twins-cli/src/main.rs (ExitCode::from(130)), crates/twins-cli/tests/cli_test.rs (both SIGINT tests)
- FOUND commits: ba0c413, 707f32e, a5d9a3a

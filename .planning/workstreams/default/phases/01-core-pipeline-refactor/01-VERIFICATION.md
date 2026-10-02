---
phase: 01-core-pipeline-refactor
verified: 2026-10-02T21:25:00Z
status: human_needed
score: 39/39 must-haves verified
covered_files:
  - .github/workflows/ci.yml
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-01-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-01-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-02-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-02-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-03-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-03-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-04-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-04-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-05-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-05-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-06-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-06-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-07-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-07-SUMMARY.md
  - Cargo.lock
  - Cargo.toml
  - crates/twins-cli/Cargo.toml
  - crates/twins-cli/src/main.rs
  - crates/twins-cli/src/progress.rs
  - crates/twins-cli/src/run.rs
  - crates/twins-cli/tests/cli_test.rs
  - crates/twins-core/src/group/find.rs
  - crates/twins-core/src/group/hasher.rs
  - crates/twins-core/src/hash.rs
  - crates/twins-core/src/human.rs
  - crates/twins-core/src/lib.rs
  - crates/twins-core/src/observe.rs
  - crates/twins-core/src/pipeline.rs
  - crates/twins-core/src/report.rs
  - crates/twins-core/src/scan/mod.rs
  - crates/twins-core/src/scan/walk.rs
  - crates/twins-core/tests/group_test.rs
  - crates/twins-core/tests/hash_test.rs
  - crates/twins-core/tests/human_test.rs
  - crates/twins-core/tests/observe_test.rs
  - crates/twins-core/tests/pipeline_test.rs
  - crates/twins-core/tests/scan_test.rs
covered_digest: "v2:sha256:41366165cc57f53f29c6e9034d3e07dcb13eb5644dcc8a075e30a8c864a8855f"
behavior_unverified: 0
overrides_applied: 0
human_verification:
  - test: "In a real terminal (not a pty emulator), run `cargo run -q -p twins-cli -- scan ~/Downloads` (or any large folder), then again with `--verify`, then with `--json > /dev/null`."
    expected: "One stderr line redraws in place through `[1/4] walk  N files`, `[2/4] size grouping  N candidates`, `[3/4] partial hash  a / b`, `[4/4] full hash  a / b` (and `[5/5] verify` with --verify), with digit groups separated by spaces. The line is cleared before the report. Progress also shows with --json. The `N candidates` wording is acceptable (research A6)."
    why_human: "Visual in-place redraw and wording quality. The byte stream was checked under `script(1)` (correct stage order, `\\r\\x1b[K` clears, cleared before the report, shown with --json), but how it looks and reads is a human judgment. This is the 01-03 human-check (SUMMARY D6, human_judgment: true), still open."
  - test: "In a real terminal, run `cargo run -q -p twins-cli -- scan ~`, press Ctrl+C once during a hashing stage, then `echo $?`. Repeat with `cargo run -q -p twins-cli -- report ~ > /tmp/twins-out.json` and Ctrl+C."
    expected: "Within about a second the progress line disappears, exactly one line `scan cancelled` follows, `echo $?` prints 130, and /tmp/twins-out.json is empty."
    why_human: "A real home-directory tree and a TTY Ctrl+C (process-group SIGINT). The automated test (sparse 1 GiB files, kill -INT, non-TTY) and a pty spot-check both pass, but a real large tree on the user's disk is the roadmap's stated scenario."
  - test: "Start a scan on a slow or external volume and press Ctrl+C twice quickly."
    expected: "The process exits immediately and `echo $?` prints 130. A stale progress line may remain (IN-01, accepted)."
    why_human: "Needs a stuck or slow read that cooperative cancellation cannot interrupt. The escape-hatch path itself was observed working (see Behavioral Spot-Checks), but not against a blocked read."
  - test: "Push the phase commits (main is 83 commits ahead of origin) or open the PR, and check the GitHub Actions `ci` workflow."
    expected: "Both jobs are green: `test` (stable: fmt, clippy -D warnings, test, release build) and the new `msrv` job on dtolnay/rust-toolchain@1.90.0 (build + test)."
    why_human: "The msrv job has never run on GitHub; the last CI run (2026-09-07) predates the phase. The same commands pass locally on 1.90.0 with RUSTFLAGS=-D warnings, and actionlint is clean."
---

# Phase 1: Core Pipeline Refactor Verification Report

**Phase Goal:** The scan pipeline runs inside `twins-core`, and any caller (CLI now, app and agent later) can follow its progress by stage and cancel it, on the Rust 1.90 toolchain Tauri needs.
**Verified:** 2026-10-02T21:25:00Z
**Status:** human_needed
**Re-verification:** No. This is the initial verification. It covers the tree after the three code-review fix iterations (HEAD d95fdf2, last code change 04f1f90).

## Goal Achievement

All five roadmap success criteria hold in the code and were exercised by running it. I ran the gates myself, built the pre-phase binary (10401aa) for an output comparison, and drove the real binary under SIGINT and under a pty. Four items remain for a human: the TTY rendering, interactive Ctrl+C on a real tree, double Ctrl+C on a slow volume, and the first remote CI run of the msrv job.

### Roadmap Success Criteria

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| SC1 | `twins scan`/`twins report` produce the same groups and JSON (schema v1) as before, orchestration moved out of `run.rs` into `twins-core`, existing suite passes | ✓ VERIFIED | `run.rs` only builds `ScanSpec` and calls `pipeline::scan(&spec, &observer, &cancel)` (run.rs:42). The grep gate for `scan::walk\|group::find\|group::plan\|Keeper::new\|Index::new\|Meta {\|dry_run` in `crates/twins-cli/src` is empty. `report.rs` diff since the phase base is one `#[allow]` line, and `VERSION = 1`. I ran a parity check: the pre-phase binary vs HEAD on a richer tree (hardlink, nested dirs, node_modules, empty files, logs, relative root, overlapping roots, --verify, --min-size, --include-empty, protected root, bad size). All 13 invocations gave identical JSON (minus `scanned_at`), identical text, identical stderr and identical exit codes. The characterization tests were committed first (c141fe0, cli_test.rs only) and later only tightened, never loosened. `cargo test --workspace`: 131 passed, 2 ignored. |
| SC2 | A long scan in a terminal shows single-line progress by stage, driven by the core observer; a test observer receives the same stage events in order | ✓ VERIFIED (visual: human) | `run.rs:38-41` wraps `TerminalObserver::stderr(stderr.is_terminal(), verbose)` in `Throttle(80ms)`. `pipeline_test::stage_events_in_order_without_verify`, `_with_verify` and `_on_empty_tree` pass, as does `observe_test::throttled_observer_sees_the_same_stage_sequence`. Under `script(1)` the byte stream reads `[1/5] walk` → `[2/5] size grouping  16 candidates` → `[3/5] partial hash  1 / 16 … 16 / 16` → `[4/5] full hash …` → `[5/5] verify  1 / 1` → `\r\x1b[K` → report. |
| SC3 | Ctrl+C stops the scan promptly (~1 s on a large tree), prints that it was cancelled, writes no partial report | ✓ VERIFIED (real tree: human) | `install_sigint` registers `register_conditional_shutdown(SIGINT,130)` before `register(SIGINT, cancel.flag())` (run.rs:67-71). `main.rs` has a cancelled arm that prints `scan cancelled` and exits 130. The report is written only after `pipeline::scan` returns Ok. `cargo test -p twins-cli --test cli_test -- --ignored sigint`: 2 passed. Spot-check: `twins report <16×1 GiB sparse>`, then SIGINT after 1 s, gave rc=130 in 0.016 s, 0 stdout bytes, stderr `scan cancelled`. Under a pty the line was cleared (`\r\x1b[K`) before `scan cancelled`. |
| SC4 | `meta.dry_run` taken from the run's real mode; a test fails if a dry-run report says otherwise | ✓ VERIFIED | `pipeline.rs:448` sets `dry_run: spec.mode.is_dry_run()`. That is the only place the flag is set, and twins-cli never names it. `pipeline_test::dry_run_in_meta_follows_run_mode` asserts false for Scan and true for DryRun, on both the report and `meta()`. `default_run_mode_is_scan` passes too. |
| SC5 | Workspace builds and passes tests and clippy (pedantic) on Rust 1.90; `rust-version` and CI raised | ✓ VERIFIED (remote CI: human) | `Cargo.toml` sets `rust-version = "1.90"` and there is no rust-toolchain file. On rustc 1.90.0 (1159e78c4) with `RUSTFLAGS=-D warnings`: `cargo +1.90.0 build --workspace --all-targets` exits 0 and `cargo +1.90.0 test --workspace` gives 131 passed, 0 failed. `cargo +1.90.0 clippy --workspace --all-targets -- -D warnings` exits 0, and stable clippy and fmt pass. `ci.yml` gained an `msrv` job on `dtolnay/rust-toolchain@1.90.0`, and actionlint is clean. |

### Plan Must-Have Truths (merged; restatements of SCs folded in)

| Plan | Truth (abridged) | Status | Evidence |
|------|------------------|--------|----------|
| 01-01 | JSON identical incl. relative root, key order | ✓ | `characterize_json_report_with_relative_root` + parity run |
| 01-01 | Text output byte-identical, empty stderr | ✓ | `characterize_text_report` + parity run |
| 01-01 | /System, `12X`, `[` exit 2 with same message | ✓ | `usage_errors_exit_2_with_the_same_message` (now exact glob message) + parity run |
| 01-01 | Unreadable file summary / `--verbose` listing | ✓ | `unreadable_files_are_counted_and_listed_with_verbose` |
| 01-01 | run.rs gets its report from `pipeline::scan` only | ✓ | grep gate empty; run.rs:42 |
| 01-01 | Exactly one `Finished`, last, Completed/Cancelled/Failed | ✓ | `pipeline.rs:246-259`; `finished_is_the_last_event`, `walk_errors_fail_without_cancelling`, `cancel_at_walk_returns_cancelled` |
| 01-01 | Cancel at Walk start or between walk and hashing → Cancelled | ✓ | `check(cancel)` at pipeline.rs:266/272/280; `cancel_between_stages_is_not_ignored` |
| 01-02 | `find` emits one `done == 0` marker per stage, even empty | ✓ | find.rs:238, 330; `each_stage_starts_with_a_zero_done_marker`, `empty_index_still_marks_hash_stages` |
| 01-02 | StageStarted order with steps 4/5, Finished last | ✓ | = SC2 tests |
| 01-02 | Same four StageStarted on empty tree | ✓ | `stage_events_on_empty_tree` |
| 01-02 | Progress inside its stage | ✓ | `progress_events_follow_their_stage_start` |
| 01-02 | Cancel at PartialHash/FullHash/Verify start → Cancelled, no later StageStarted | ✓ | `cancel_at_partial_hash_stops_before_full_hash`, `cancel_at_full_hash_returns_cancelled`, `cancel_at_verify_returns_cancelled` |
| 01-03 | `[k/N] label  n / m` line, space digit grouping | ✓ | `progress.rs::line` + 4 unit tests; `group_digits_inserts_spaces_every_three_digits`; pty capture |
| 01-03 | Line cleared on `Finished` | ✓ | `renders_progress_and_clears_on_finish`; pty capture on success and on cancel |
| 01-03 | Progress on any TTY incl. `--json`; non-TTY `--json` stderr empty | ✓ | run.rs uses only `is_terminal()`; `json_stderr_is_silent_when_not_a_terminal`; pty spot-check with `--json` showed progress |
| 01-03 | `--verbose` skip lines with/without TTY, clearing the line first | ✓ | `verbose_skips_print_without_a_terminal`; pty spot-check showed `\r\x1b[K` + `skip <path>: open …` |
| 01-03 | Throttle never drops/reorders structural events; Progress gate rules | ✓ | observe.rs:278-295; 7 throttle tests in observe_test (now Mutex-gated per WR-03, also drops stale `done`) |
| 01-03 | Throttled observer sees same stage sequence | ✓ | `throttled_observer_sees_the_same_stage_sequence` |
| 01-04 | rust-version 1.90, no rust-toolchain file | ✓ | Cargo.toml:10; `find` for rust-toolchain* empty |
| 01-04 | `cargo +1.90.0` build/test pass | ✓ | run here, see SC5 |
| 01-04 | MSRV-aware clippy passes | ✓ | stable + 1.90 clippy exit 0; one justified `#[allow(clippy::similar_names)]` in report.rs |
| 01-04 | CI stable job unchanged + msrv job | ✓ | ci.yml (stable job intact; msrv appended) |
| 01-05 | Walk with raised flag → `ScanError::Cancelled` | ✓ | walk.rs:125; `walk_cancelled_before_start` |
| 01-05 | Cancel during stat phase stops visiting | ✓ | walk.rs:133-141; `walk_cancel_during_stat_phase_stops` |
| 01-05 | Walk Progress (total None) before SizeGrouping, non-decreasing, last = files | ✓ | pipeline.rs:384-395; `walk_progress_counts_files_before_size_grouping` |
| 01-05 | Public `scan::walk` signature unchanged | ✓ | walk.rs:77 |
| 01-06 | `full_cancellable`/`equal_cancellable` check per 256 KiB chunk, `cancelled`+Interrupted | ✓ | hash.rs `check_cancel` at loop top; 4 hash_test tests |
| 01-06 | Interrupted full hash never yields a Digest | ✓ | only `Err` path after check; `full_cancellable_stops_on_raised_flag` |
| 01-06 | Cancel during hashing → `FindError::Cancelled`, file not reported | ✓ | find.rs:341-343; `cancel_during_full_hash_is_not_reported_as_error` |
| 01-06 | Verify polls before and inside each comparison | ✓ | find.rs:433-444, 247; `verify_stops_when_cancelled` |
| 01-06 | `hash::full`, `hash::equal`, `Hasher` methods source-compatible | ✓ | defaulted trait method; SpyHasher compiles (group_test passes) |
| 01-07 | Ctrl+C → `scan cancelled`, nothing on stdout, exit 130 | ✓ | = SC3 |
| 01-07 | Second Ctrl+C exits 130 immediately, never default action | ✓ | conditional shutdown registered first; spot-check (see below): 4/5 back-to-back double-SIGINT runs exited 130 with **empty** stderr, which proves the `_exit(130)` path (the cooperative path always prints `scan cancelled`) |
| 01-07 | Exit codes 130 / 2 / 1 distinct | ✓ | `exit_code` + 6 unit tests; `usage_errors_exit_2…` |
| 01-07 | signal-hook only in twins-cli; core tree has no signal-hook/ctrlc/tokio | ✓ | `cargo tree -p twins-core -e normal` has no match; core Cargo.toml clean |
| 01-07 | twins-cli has no unsafe | ✓ | `grep -rn unsafe crates/twins-cli/src` empty |
| 01-07 | Workspace incl. signal-hook passes on 1.90.0 | ✓ | see SC5; Cargo.lock has signal-hook + signal-hook-registry |

**Score:** 39/39 distinct truths verified, 0 present but behavior-unverified. Every behavior-dependent truth (cancellation, ordering, Finished-last, no-digest-on-cancel, dry-run derivation) is backed by a named passing test, a direct run of the binary, or both.

### Prohibitions (all `verification: test`, all with wired enforcement)

| Plan | Prohibition | Enforcement | Status |
|------|-------------|-------------|--------|
| 01-01 | twins-cli MUST NOT construct `report::Meta` or decide dry-run | grep gate (empty) + `dry_run_in_meta_follows_run_mode` | ✓ enforced |
| 01-03 | Progress MUST NOT go to stdout | `! grep 'print!\|println!\|stdout' progress.rs` (empty) + `json_stderr_is_silent…` + exact-stdout characterization tests | ✓ enforced |
| 01-06 | Interrupted hash MUST NOT yield a prefix digest or be reported unreadable | `full_cancellable_stops_on_raised_flag`, `cancel_during_full_hash_is_not_reported_as_error`, `verify_stops_when_cancelled` | ✓ enforced |
| 01-07 | Cancelled/failed scan MUST NOT write report bytes or exit 0 | `sigint_cancels_scan_with_exit_130` (stdout empty, 130) — `#[ignore]`d, run here | ✓ enforced (see Info: not run in CI) |
| 01-07 | twins-core MUST NOT install signal handlers / depend on signal-hook, ctrlc, tokio | `cargo tree` gate (empty) | ✓ enforced |

### Required Artifacts

`gsd-tools verify.artifacts` gave 22/22 passed across the 7 plans. I also read each file in full.

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/twins-core/src/observe.rs` | ✓ VERIFIED | CancelToken, Stage (step, Display, From), Outcome, Event (camelCase tagged), Observer, NoopObserver, Throttle |
| `crates/twins-core/src/pipeline.rs` | ✓ VERIFIED | RunMode, ScanSpec, ScanOutcome, PipelineError, `scan()`; keeper mapping from review fixes |
| `crates/twins-core/src/scan/walk.rs` | ✓ VERIFIED | `walk_observed`, per-root/per-entry/per-file cancel polls, `normalise_roots` pub(crate) |
| `crates/twins-core/src/group/find.rs` | ✓ VERIFIED | done==0 markers, Ticker in-order progress, cancel re-check, `equal_cancellable` in verify |
| `crates/twins-core/src/hash.rs`, `group/hasher.rs` | ✓ VERIFIED | cancellable full/equal; defaulted trait method, DirectHasher override |
| `crates/twins-core/src/human.rs` | ✓ VERIFIED | `group_digits` |
| `crates/twins-cli/src/progress.rs` | ✓ VERIFIED | `line()`, `TerminalObserver` + 7 unit tests |
| `crates/twins-cli/src/run.rs`, `main.rs` | ✓ VERIFIED | thin shell; SIGINT; cancelled arm |
| `Cargo.toml`, `.github/workflows/ci.yml` | ✓ VERIFIED | 1.90; msrv job |
| tests: pipeline_test (28), observe_test (11), scan_test (10), group_test (18), hash_test (9), cli_test (11 + 2 ignored) | ✓ VERIFIED | all pass |

### Key Link Verification

`gsd-tools verify.key-links` gave 12/12 verified. I confirmed each one by reading the code.

| From | To | Via | Status |
|------|----|-----|--------|
| run.rs | pipeline.rs | `pipeline::scan(&spec, &observer, &cancel)` then `outcome.report(now)` | WIRED |
| pipeline.rs | report.rs | `Meta { dry_run: spec.mode.is_dry_run() }` | WIRED |
| pipeline.rs | scan/group options | `.cancel(cancel.flag())` on both | WIRED |
| find.rs | pipeline.rs | `on_progress`: `done == 0` becomes StageStarted | WIRED |
| run.rs | progress.rs | `Throttle::new(TerminalObserver::stderr(..), 80ms)` | WIRED |
| progress.rs | human.rs | `group_digits(` | WIRED |
| ci.yml | Cargo.toml | `rust-toolchain@1.90.0` | WIRED |
| pipeline.rs | walk.rs | `scan::walk_observed(` with a Walk Progress closure | WIRED |
| find.rs → hasher.rs → hash.rs | | `full_cancellable(m, opts.cancel_flag())`, then `hash::full_cancellable` | WIRED |
| run.rs | observe.rs | `register(SIGINT, cancel.flag())` | WIRED |
| main.rs | run.rs | `Err(err) if run::is_cancelled(&err)` | WIRED |

### Data-Flow Trace (Level 4)

| Artifact | Data | Source | Real data | Status |
|----------|------|--------|-----------|--------|
| CLI report (stdout) | `outcome.report(now)` | `pipeline::scan` (walk, Index, find, plan) | yes: parity with the pre-phase binary | ✓ FLOWING |
| Progress line (stderr) | Event stream | pipeline `started`/Progress closures from walk counters and the find Ticker | yes: pty capture shows real counts (16 candidates, 1/16 … 16/16) | ✓ FLOWING |
| `meta.dry_run` | `spec.mode` | `RunMode`. The CLI always uses the default `Scan`, which is correct: there is no dry-run CLI flag per D-14, and Phase 2 adds the modes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| fmt / clippy / tests (stable) | `cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace` | 0 / 0 / 131 passed, 2 ignored | ✓ PASS |
| Rust 1.90.0 | `RUSTFLAGS=-D warnings cargo +1.90.0 build --all-targets` / `test` / `clippy -D warnings` | all exit 0, 131 passed | ✓ PASS |
| Real SIGINT tests | `cargo test -p twins-cli --test cli_test -- --ignored sigint` | 2 passed | ✓ PASS |
| Output parity vs pre-phase binary | old (10401aa) vs HEAD, 13 invocations | all identical | ✓ PASS |
| `report` + SIGINT | 16×1 GiB sparse, `kill -INT` after 1 s | rc 130, 0.016 s, stdout 0 B, stderr `scan cancelled` | ✓ PASS |
| Pty progress + cancel | `script -q … twins scan --verify`, SIGINT | stages in order; `\r\x1b[K` then `scan cancelled`; `EXIT=130` | ✓ PASS |
| Pty `--json` progress (D-09) | `script -q … twins scan --json` | progress drawn, then JSON | ✓ PASS |
| Pty `--verbose` skip clears line | unreadable candidate, `-v --min-size 1` | `\r\x1b[K` + `skip <path>: open …` | ✓ PASS |
| Double SIGINT escape hatch | `kill -INT $P; kill -INT $P` ×5 | 5/5 rc 130, stdout empty; 4/5 stderr empty (`_exit(130)` path), 1/5 cooperative | ✓ PASS |
| Core has no signal deps | `cargo tree -p twins-core -e normal \| grep signal-hook\|ctrlc\|tokio` | no match | ✓ PASS |

### Probe Execution

Step 7c: SKIPPED. There are no `scripts/*/tests/probe-*.sh` files, and no plan declares a probe.

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CORE-01 | 01-01, 01-02, 01-03, 01-05 | Pipeline in twins-core, staged progress to any caller through an observer | ✓ SATISFIED (visual check pending) | SC1, SC2; `Observer` trait + serde `Event` ready for the app/agent. REQUIREMENTS.md still shows `[ ]` / Pending, waiting for the 01-03 TTY human check. |
| CORE-02 | 01-01, 01-02, 01-05, 01-06, 01-07 | Cancel a running scan, stops promptly, no partial state | ✓ SATISFIED (for scan; "clean" arrives in Phase 2) | SC3; cancel at every stage, mid-file and in the stat phase; no stdout on cancel |
| CORE-03 | 01-01 | `dry_run` reflects the real mode | ✓ SATISFIED | SC4 |
| CORE-04 | 01-04, 01-07 | Builds on Rust 1.90 with tests passing | ✓ SATISFIED | SC5 |

There are no orphaned requirements. REQUIREMENTS.md maps only CORE-01..04 to Phase 1, and every plan's `requirements` field accounts for all four.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (all phase-modified src files) | — | TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER | none found | none |
| crates/twins-cli/tests/cli_test.rs | 425-443 | `double_sigint_exits_130_immediately` cannot tell the escape hatch from a fast cooperative cancel: on sparse files the first SIGINT alone exits in ~16 ms, so the test passes even if the second signal is never handled | ⚠️ Warning (test weakness) | The behavior itself was proven by the stderr-discriminating spot-check above. Asserting empty stderr when the second signal lands would make the test meaningful. |
| crates/twins-cli/tests/cli_test.rs | 401-443 | The real-SIGINT tests are `#[ignore]` and no CI job runs `--ignored` (review IN-06) | ℹ️ Info | Exit-130 / no-stdout coverage is manual-only |
| crates/twins-cli/src/run.rs | 44-49 | A second Ctrl+C that lands *while the report is being written* calls `_exit(130)` mid-write and can leave a truncated report on stdout | ℹ️ Info | Narrow window (writing is fast); outside the SC3 scenario (Ctrl+C *during a scan*) |
| crates/twins-core/src/scan/walk.rs | 195 | Walk progress has no total and is only emitted while listing. Its final count is not forced through Throttle, and it freezes during the stat phase (IN-04). The pty run showed `walk  1 files` on a 16-file tree | ℹ️ Info | Cosmetic. On large trees the 80 ms throttle shows a rising count. |
| crates/twins-core/src/pipeline.rs | 367-372 | `FileSkipped.reason` repeats the path (`skip /x: open /x: …`), which is now part of the public event contract (IN-03) | ℹ️ Info | Seen in the pty spot-check. Matches the pre-phase CLI text. |
| .claude/CLAUDE.md | 20,33,39,89 | Still says Rust 1.85 (IN-08) | ℹ️ Info | Docs only |
| review | — | 01-REVIEW iteration 3 WR-01 was fixed in 04f1f90 with 3 regression tests, but no review pass ran after that fix. Info items IN-01…IN-12 remain open | ℹ️ Info | The `InDir` keeper is not reachable from the CLI in this phase. DEL-01 belongs to Phase 2. |

### Human Verification Required

#### 1. Terminal progress rendering

**Test:** In a real terminal, run `cargo run -q -p twins-cli -- scan ~/Downloads`. Run it again with `--verify`, then with `--json > /dev/null`.
**Expected:** One stderr line redraws in place through `[1/4] walk  N files`, `[2/4] size grouping  N candidates`, `[3/4] partial hash  a / b` and `[4/4] full hash  a / b`. With `--verify`, the steps read /5 and `[5/5] verify` appears. The line is cleared before the report and is also drawn with `--json`. The `N candidates` wording reads well.
**Why human:** This is visual quality and wording. The byte stream is already verified under `script(1)`.

#### 2. Interactive Ctrl+C on a real large tree

**Test:** Run `cargo run -q -p twins-cli -- scan ~`, press Ctrl+C once while it is hashing, then run `echo $?`. Repeat with `report ~ > /tmp/twins-out.json`.
**Expected:** The progress line disappears and one `scan cancelled` line appears, within about a second. `echo $?` prints 130 and `/tmp/twins-out.json` is empty.
**Why human:** Needs a real home tree and a terminal-generated SIGINT. The automated and pty checks pass.

#### 3. Double Ctrl+C on a slow volume

**Test:** Start a scan on a slow or external volume and press Ctrl+C twice quickly.
**Expected:** The process exits immediately and `echo $?` prints 130.
**Why human:** Needs a blocked read. The `_exit(130)` path was observed working on a local disk.

#### 4. First remote CI run

**Test:** Push the branch or open the PR, then check GitHub Actions `ci`.
**Expected:** Both `test` and `msrv` (Rust 1.90.0) are green.
**Why human:** The job has never run remotely. Main is 83 commits ahead of origin.

### Gaps Summary

There are no gaps. Each success criterion is backed by code I read, tests I ran, and direct runs of the binary:
- The pipeline lives in `twins-core`. The CLI is a thin shell, and its output matches the pre-phase binary exactly.
- Stage events reach any `Observer` in order and drive the terminal line.
- One `CancelToken` stops the walk, the hashing (within a chunk) and the verify, and Ctrl+C maps to `scan cancelled` with exit 130 and nothing on stdout.
- `meta.dry_run` is derived from `RunMode`.
- The workspace is green on Rust 1.90.0, including pedantic clippy.

The status is `human_needed` only because of the four checks above: TTY look, interactive Ctrl+C on a real tree, the stuck-volume escape hatch, and the first remote msrv CI run. I also recommend strengthening `double_sigint_exits_130_immediately` to assert empty stderr, so it actually discriminates the escape-hatch path.

---

_Verified: 2026-10-02T21:25:00Z_
_Verifier: Claude (gsd-verifier)_

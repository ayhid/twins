---
phase: 01-core-pipeline-refactor
verified: 2026-10-03T16:18:26Z
status: human_needed
score: 50/50 must-haves verified
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
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-08-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-08-SUMMARY.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-09-PLAN.md
  - .planning/workstreams/default/phases/01-core-pipeline-refactor/01-09-SUMMARY.md
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
  - crates/twins-core/tests/keep_test.rs
  - crates/twins-core/tests/observe_test.rs
  - crates/twins-core/tests/pipeline_test.rs
  - crates/twins-core/tests/report_test.rs
  - crates/twins-core/tests/scan_test.rs
covered_digest: "v2:sha256:07569cd5c4c380d9d91f5c90a3dd6f125e5d5cf3cc0dabd5a89dca8a5ba88eff"
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: human_needed
  previous_score: 39/39
  gaps_closed:
    - "G-01-4 (locally): stable clippy 1.99 pedantic passes with -D warnings on every target; the CI test and msrv command sequences pass locally"
    - "G-01-1 (in code): the text report labels every row keep / keep (hardlink) / remove, prints each group's folder once, separates groups with one blank line and escapes control characters"
  gaps_remaining: []
  regressions: []
advisory:
  - finding: "Review WR-01: printable() escapes only char::is_control (Cc). U+2028/U+2029, bidi overrides (U+202A-U+202E, U+2066-U+2069) and zero-width characters pass through, and a literal backslash is not escaped, so a filename can still visually disguise a row in renderers that honour those characters"
    category: security
    reason: "Plan 01-09's contract defines the escape set as char::is_control, and that contract is met and tested. The wider spoofing surface is new scope from code review iteration 4, with no failing test. Resolve by widening the escape set plus tests, or by explicitly deferring it to Phase 2, where the text report becomes the review surface before clean"
    evidence_status: "none provided"
  - finding: "Review WR-02: a filename padded with spaces soft-wraps in the terminal and can show a forged `    keep    x.bin` continuation at column 0; the `  (hardlink)` suffix is free text a filename can imitate"
    category: security
    reason: "Visual-only, terminal-width dependent and not covered by any test. The plan, JSON and roles are unaffected. Resolve by quoting paths with space runs and moving the hardlink marker into the label column, or defer explicitly"
    evidence_status: "none provided"
human_verification:
  - test: "Push main (it is 19 commits ahead of origin/main and the 01-08 fix is not on the remote yet), then run `gh run list --branch main --limit 1` and `gh run view <run-id>`."
    expected: "The `ci` run is success, and both the `test` job (stable toolchain: fmt, clippy -D warnings, test, release build) and the `msrv` job (dtolnay/rust-toolchain@1.90.0: build all targets, test) are green. This closes G-01-4 and UAT test 4."
    why_human: "Needs the user's push and a remote GitHub Actions run. The last remote run (37133285179) is the failing one that predates 01-08. Every command in both jobs passes locally on rustc 1.99.0 and 1.90.0 with RUSTFLAGS=-D warnings."
  - test: "Re-run UAT test 1: in a real terminal, run `cargo run -q -p twins-cli -- scan <a real folder with duplicates, hardlinks and subfolder copies>`, and once with `--verify`."
    expected: "The progress line redraws in place by stage and is cleared before the report. In the report, groups are separated by a blank line, each group's folder is printed once on a line ending in `/`, every row says `keep`, `keep … (hardlink)` or `remove` with a short relative path, and the result reads well."
    why_human: "Readability is a human judgment (01-09 D6, human_judgment: true). The exact layout is pinned by tests, and the binary was run here on a sample tree, but the user reported G-01-1, so the user must confirm it is closed."
---

# Phase 1: Core Pipeline Refactor Verification Report

**Phase Goal:** The scan pipeline runs inside `twins-core`, and any caller (CLI now, app and agent later) can follow its progress by stage and cancel it, on the Rust 1.90 toolchain Tauri needs.
**Verified:** 2026-10-03T16:18:26Z (HEAD 451f3b6; the last code change is 1a18001)
**Status:** human_needed
**Re-verification:** Yes. This follows UAT (01-UAT.md), which found G-01-4 (the CI test job failed on Rust 1.99 clippy `assert_is_empty`) and G-01-1 (the `twins scan` text report was hard to read). Plans 01-08 and 01-09 closed them. I fully re-verified both gap plans and regression-checked the 39 truths that passed before.

## Goal Achievement

The goal holds in the code. Both UAT gaps are closed in code. I ran every gate myself on rustc 1.99.0 (current stable) and on 1.90.0, and drove the built binary on sample trees. Two items still need a human:

- **Remote CI:** the 01-08 fix is not pushed yet, so CI on GitHub has not run with it.
- **Readability:** the user should re-check that the new text report reads well.

**MVP-mode note:** ROADMAP marks this phase `**Mode:** mvp`, but the goal is not in user-story form (`user-story.validate` → `valid: false`). Plan 01-01 records this and did not invent a story. As in the initial verification, I used standard goal-backward verification and did not produce a User Flow Coverage table. Run `/gsd-mvp-phase 1` if you want the story framing.

### Roadmap Success Criteria

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| SC1 | `twins scan`/`twins report` produce the same groups and JSON (schema v1) as before, orchestration lives in `twins-core`, the existing suite passes | ✓ VERIFIED | **Orchestration:** run.rs:42 still calls `pipeline::scan(&spec, &observer, &cancel)`, and the CLI grep gate (`scan::walk\|group::find\|group::plan\|Keeper::new\|Index::new\|Meta {\|dry_run`) is empty. **JSON unchanged:** since d95fdf2, report.rs changes only the import line, the `write_text` doc and body, and the new private `Role`/`shared_folder`/`printable` helpers. `Report`, `Summary`, `GroupEntry`, `FileEntry`, `build`, `write_json` and `VERSION = 1` are unchanged. `json_schema_is_stable` and `characterize_json_report_with_relative_root` pass and were not modified. **Runtime check:** `twins report` on a sample tree gave version 1 with the expected keep/remove. **Text layout:** only the human text layout changed, as the user asked in UAT. **Suite:** `cargo test --workspace` gives 134 passed, 0 failed, 2 ignored. |
| SC2 | A long scan shows single-line progress by stage, driven by the core observer; a test observer gets the same stage events in order | ✓ VERIFIED (visual: human) | Wiring is unchanged (run.rs:39 `TerminalObserver::stderr(..)` inside `Throttle`). `stage_events_in_order_without_verify`/`_with_verify` pass. `throttled_observer_sees_the_same_stage_sequence` is now stronger: it pins `(Walk,1,4) (SizeGrouping,2,4) (PartialHash,3,4) (FullHash,4,4)` exactly. |
| SC3 | Ctrl+C stops the scan promptly, prints that it was cancelled, writes no partial report | ✓ VERIFIED | Unchanged since the initial verification: run.rs:68-70 and the main.rs:28-30 cancelled arm with exit 130. `cargo test -p twins-cli --test cli_test -- --ignored sigint` gives 2 passed. UAT tests 2 and 3 passed with the user on a real tree and on an external volume. |
| SC4 | `meta.dry_run` comes from the run's real mode, and a test fails if a dry-run report says otherwise | ✓ VERIFIED | `pipeline.rs:448 dry_run: spec.mode.is_dry_run()`. `dry_run_in_meta_follows_run_mode` passes. |
| SC5 | Workspace builds and passes tests and clippy (pedantic) on Rust 1.90; `rust-version` and CI raised | ✓ VERIFIED (remote CI: human) | `rust-version = "1.90"`, with no rust-toolchain file. On rustc **1.99.0**: fmt check 0, `clippy --workspace --all-targets --keep-going -D warnings` 0 (0 error/warning lines), `RUSTFLAGS=-D warnings` test (134 passed) and release build 0. On **1.90.0** with `RUSTFLAGS=-D warnings`: build all targets 0, test 134 passed, clippy -D warnings 0. ci.yml has the stable `test` job and the `msrv` job on `rust-toolchain@1.90.0`, and no commit touched it since d95fdf2. The remote run is pending a push. |

### Gap Closure (UAT)

| Gap | Truth | Status | Evidence |
|-----|-------|--------|----------|
| G-01-4 | CI green on main: `test` (stable, clippy -D warnings) and `msrv` pass | ✓ closed locally / remote run: human | **Reproduction:** stable 1.99.0 is installed, the same clippy that failed run 37133285179. **Fixes:** observe_test.rs:238 is now an exact `assert_eq!` on the stage sequence. group_test.rs:61/85/366 and keep_test.rs:127 use `assert_eq!(.., [] as [T; 0])`. **No allow:** `grep -rn assert_is_empty crates` is empty. **CI commands:** the full test-job and msrv-job sequences pass locally. **Not pushed:** origin/main is 19 commits behind and still on the failing run. |
| G-01-1 | Text report readable: groups separated, keep/remove clear, paths not repeated | ✓ closed in code / readability: human | **Code:** `write_text` (report.rs:165-216) implements the plan's layout, and four exact-format tests pin it. **Real run:** two groups, a blank line between them, a folder line, then `keep    beach-link.jpg` / `keep    beach.jpg  (hardlink)` / `remove  2024/beach-copy.jpg`. A file named `ev<ESC>[31mil<LF>keep` printed as one row: `remove  ev\u{1b}[31mil\nkeep`. |

### Plan Must-Have Truths

**Plans 01-08 and 01-09 (full verification):**

| Plan | Truth (abridged) | Status | Evidence |
|------|------------------|--------|----------|
| 01-08 | Local stable is Rust 1.99+ | ✓ | `rustc 1.99.0 (b940084d7 2026-09-28)`, `clippy 0.1.99` |
| 01-08 | Stable clippy -D warnings passes on every target; observe_test:238 fixed; every --keep-going site fixed in code; no allow for the new lint | ✓ | `--keep-going` run exit 0; diff of the 4 test files read; `grep assert_is_empty crates` empty |
| 01-08 | `throttled_observer_sees_the_same_stage_sequence` asserts the exact 4-step sequence | ✓ | observe_test.rs:238-246; test passes (single run in workspace output) |
| 01-08 | CI test-job commands pass on stable; msrv-job commands pass on 1.90.0 | ✓ | ran all six commands here, all exit 0 |
| 01-08 | ci.yml unchanged, nothing pushed; CI green confirmed by user after push | ✓ (CI green: human) | `git log d95fdf2..HEAD -- .github/workflows/ci.yml` empty; origin/main...HEAD = 0/19 |
| 01-09 | Rows labelled `keep` / `keep … (hardlink)` / `remove`; ★ and blank marker gone | ✓ | report.rs:198-203; `grep ★ crates` empty; `text_labels_every_row_and_shows_the_group_folder`, `scan_text_marks_kept_file_and_never_lists_hardlinks_to_remove` pass; binary run |
| 01-09 | Shared folder printed once ending in `/`, relative rows; only `/` shared → no folder line, full paths | ✓ | `shared_folder` (report.rs:243-258, `common.len() > 1`); `members_sharing_only_the_root_show_full_paths`, `characterize_text_report` pass |
| 01-09 | Exactly one blank line between groups and before totals | ✓ | report.rs:174-176 and the `\n` before the totals; `two_groups_are_separated_by_one_blank_line` (exact) passes |
| 01-09 | Roles come only from `keep` and `remove` | ✓ | `Role::of` (report.rs:229-238) reads only `g.keep`/`g.remove`; roles match against the raw strings, before `printable` |
| 01-09 | Control characters escaped, one file = one row, cannot forge a row | ✓ (see advisory WR-01/WR-02) | `printable` uses `escape_debug` for `is_control` chars; `control_characters_in_paths_stay_on_one_row` (exact text + `lines().count() == 6`); ESC and LF escaped end to end in a binary run. The plan's contract (Cc) is met. Wider visual spoofing (Unicode separators, bidi, soft-wrap) is the open review advisory. |
| 01-09 | JSON byte-identical; json tests unmodified; `write_text` signature kept | ✓ | diff hunks confined to `write_text` and helpers; `pub fn write_text(w: &mut impl Write, r: &Report) -> io::Result<()>` at report.rs:165; the json tests are untouched by the report_test diff (it starts after `json_round_trips_through_serde`) |

**Plans 01-01 to 01-07 (regression check against the 39 truths passed on 2026-10-02):** all hold. I re-checked:

- **Grep gates:** the CLI orchestration gate is empty, `cargo tree -p twins-core -e normal` has no signal-hook/ctrlc/tokio, and `crates/twins-cli/src` has no `unsafe`.
- **Wiring:** run.rs:39/42/49/68/70, main.rs:28-30 and pipeline.rs:448 are in place.
- **Tests:** every named test from the initial report passed in the single workspace run, including the cancel-at-every-stage, Finished-last, done==0 marker, hash-cancel and throttle tests, plus the 2 ignored SIGINT tests run explicitly.

One truth was intentionally superseded:

| Plan | Truth | Status | Note |
|------|-------|--------|------|
| 01-01 | "twins scan prints byte-identical text output for the fixture tree, and nothing on stderr" | ✓ VERIFIED (superseded by 01-09) | **Refactor parity:** this characterized the refactor, and the initial verification proved it with byte-for-byte parity against the pre-phase binary (10401aa). **Requested change:** UAT G-01-1 then asked for a new text layout. `characterize_text_report` still pins exact stdout and empty stderr, now for the new layout. **Roadmap SC1:** it only requires the same groups and JSON, and those are unchanged. |

**Score:** 50/50 truths verified (5 SC + 34 from 01-01..01-07 + 5 from 01-08 + 6 from 01-09), 0 present but behavior-unverified. Every behavior-dependent truth is backed by a named passing test.

### Prohibitions (all `verification: test`, all with wired enforcement)

| Plan | Prohibition | Enforcement | Status |
|------|-------------|-------------|--------|
| 01-01 | twins-cli MUST NOT construct `report::Meta` or decide dry-run | grep gate (empty) + `dry_run_in_meta_follows_run_mode` | ✓ enforced |
| 01-03 | Progress MUST NOT go to stdout | `json_stderr_is_silent…` + exact-stdout characterization tests | ✓ enforced |
| 01-06 | Interrupted hash MUST NOT yield a prefix digest or be reported unreadable | `full_cancellable_stops_on_raised_flag`, `cancel_during_full_hash_is_not_reported_as_error`, `verify_stops_when_cancelled` | ✓ enforced |
| 01-07 | Cancelled/failed scan MUST NOT write report bytes or exit 0 | `sigint_cancels_scan_with_exit_130` (ignored in CI, run here: pass) | ✓ enforced |
| 01-07 | twins-core MUST NOT install signal handlers / depend on signal-hook, ctrlc, tokio | `cargo tree` gate (empty) | ✓ enforced |
| 01-08 | A lint MUST NOT be silenced by deleting/weakening an assertion or by an allow for `assert_is_empty` | `grep -rn assert_is_empty crates` empty + clippy 1.99 `-D warnings` exit 0 + diff read: each replacement keeps the check or strengthens it (observe_test) | ✓ enforced |
| 01-09 | JSON report types, `build`, `write_json`, `VERSION` MUST NOT change | `json_schema_is_stable` (exact, unmodified) + `characterize_json_report_with_relative_root` (unmodified) + diff confined to `write_text` | ✓ enforced |

### Required Artifacts

`gsd-tools verify.artifacts`: 01-08 gave 1/1 and 01-09 gave 3/3. The 22/22 for 01-01..01-07 still hold, since none of those artifacts changed except where noted.

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/twins-core/tests/observe_test.rs` | ✓ VERIFIED | contains `(Stage::FullHash, 4, 4)` inside the throttled test |
| `crates/twins-core/src/report.rs` | ✓ VERIFIED | `Role`, `shared_folder`, `printable`, `(hardlink)`, `escape_debug`; the JSON surface is untouched |
| `crates/twins-core/tests/report_test.rs` | ✓ VERIFIED | 4 exact-format text tests, 3 of them new, plus the unchanged json tests |
| `crates/twins-cli/tests/cli_test.rs` | ✓ VERIFIED | `characterize_text_report` is exact for the new layout; the hardlink and remove predicates are in place |
| group_test.rs, keep_test.rs | ✓ VERIFIED | emptiness asserts now print values on failure |
| Earlier artifacts (observe.rs, pipeline.rs, walk.rs, find.rs, hash.rs, hasher.rs, human.rs, progress.rs, run.rs, main.rs, Cargo.toml, ci.yml) | ✓ VERIFIED | no code change since d95fdf2; all tests pass |

### Key Link Verification

`gsd-tools verify.key-links`: 01-08 gave 1/1 and 01-09 gave 2/2. The 12 earlier links are unchanged.

| From | To | Via | Status |
|------|----|-----|--------|
| observe_test.rs | pipeline.rs | the plain Recorder's StageStarted events equal the pipeline's 4-step sequence | WIRED (test passes) |
| run.rs | report.rs | `report::write_text(&mut out, &r)` at run.rs:49; the new layout reaches stdout with no CLI change | WIRED (binary run shows it) |
| report.rs | group/keep.rs | roles come from `GroupEntry.keep`/`remove`, built from `Action` | WIRED |
| run.rs | pipeline.rs | `pipeline::scan(&spec, &observer, &cancel)` | WIRED |
| run.rs | observe.rs / signal-hook | `register_conditional_shutdown` then `register(SIGINT, cancel.flag())` | WIRED |
| main.rs | run.rs | `Err(err) if run::is_cancelled(&err)` → 130 | WIRED |

### Data-Flow Trace (Level 4)

| Artifact | Data | Source | Real data | Status |
|----------|------|--------|-----------|--------|
| Text report (stdout) | `Report` → `write_text` | `pipeline::scan` → `outcome.report(now)` | yes: sample tree showed real groups, hardlink and copies | ✓ FLOWING |
| JSON report | `write_json` | same `Report` | yes: version 1, keep/remove match the text roles | ✓ FLOWING |
| Progress line | Event stream | pipeline stage/progress closures | yes (initial verification; wiring unchanged) | ✓ FLOWING |
| `meta.dry_run` | `spec.mode` | `RunMode` | yes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Toolchain | `rustc +stable --version`; `cargo +stable clippy --version` | 1.99.0 / 0.1.99 | ✓ PASS |
| CI test job (stable) | `cargo +stable fmt --all --check`; `clippy --workspace --all-targets --keep-going -- -D warnings`; `RUSTFLAGS=-D warnings test --workspace`; `… build --workspace --release` | 0 / 0 (no diagnostics) / 134 passed, 0 failed, 2 ignored / 0 | ✓ PASS |
| CI msrv job (1.90.0) | `RUSTFLAGS=-D warnings cargo +1.90.0 build --workspace --all-targets`; `… test --workspace`; `cargo +1.90.0 clippy … -D warnings` | 0 / 134 passed / 0 | ✓ PASS |
| Real SIGINT tests | `cargo test -p twins-cli --test cli_test -- --ignored sigint` | 2 passed | ✓ PASS |
| New text layout end to end | `twins scan <tree with hardlink, subfolder copy, second pair>` | blank line between groups, folder lines, `keep` / `keep … (hardlink)` / `remove` rows, totals | ✓ PASS |
| Control chars end to end | `twins scan --min-size 1 <dir with "ev\x1b[31mil\nkeep">` | one row: `remove  ev\u{1b}[31mil\nkeep` | ✓ PASS |
| JSON still v1 | `twins report <tree>` | `version` 1, keep/remove consistent with the text | ✓ PASS |
| Remote CI | `gh run list --branch main --limit 3` | latest run is 37133285179 (failure, before 01-08); fix not pushed | ? SKIP → human |

### Probe Execution

Step 7c: SKIPPED. There are no `scripts/*/tests/probe-*.sh` files, and no plan declares a probe.

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CORE-01 | 01-01, 01-02, 01-03, 01-05, 01-09 | Pipeline in twins-core, staged progress to any caller through an observer | ✓ SATISFIED (readability re-check pending) | SC1, SC2, G-01-1 closed in code. REQUIREMENTS.md still shows Pending, which is right until the human UAT re-check. |
| CORE-02 | 01-01, 01-02, 01-05, 01-06, 01-07 | Cancel a running scan, stops promptly, no partial state | ✓ SATISFIED (scan; "clean" arrives in Phase 2) | SC3; UAT tests 2 and 3 passed |
| CORE-03 | 01-01 | `dry_run` reflects the real mode | ✓ SATISFIED | SC4 |
| CORE-04 | 01-04, 01-07, 01-08 | Builds on Rust 1.90 with tests passing | ✓ SATISFIED (remote CI pending) | SC5, G-01-4 closed locally |

There are no orphaned requirements. REQUIREMENTS.md maps only CORE-01..04 to Phase 1, and each one is claimed by at least one plan.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| all phase-modified files | — | TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER | none found | none |
| crates/twins-core/src/report.rs | 260-272 | `printable` escapes only Cc. U+2028/2029, bidi and zero-width characters pass through, and `\` is not escaped, so `a\nb` (literal) and `a<LF>b` print the same (review WR-01) | ⚠️ Warning (advisory) | Visual spoofing in renderers that honour those characters, such as the planned Tauri webview. The plan's Cc contract is met. |
| crates/twins-core/src/report.rs | 192-204 | A space-padded filename soft-wraps into a forged `keep` row at column 0, and the `  (hardlink)` suffix can be imitated by a filename (review WR-02) | ⚠️ Warning (advisory) | Visual only, and it does not change the plan. It matters once Phase 2's clean uses this view for review. |
| crates/twins-core/src/report.rs | 229-238 | Any member that is neither keep nor remove is labelled `(hardlink)` without an inode check (review IN-01) | ℹ️ Info | Correct for pipeline output. An overlapping-root alias would be mislabelled as a hardlink. |
| crates/twins-cli/tests/cli_test.rs | ~425-443 | `double_sigint_exits_130_immediately` cannot tell the `_exit(130)` path from a fast cooperative cancel (carried from the initial report) | ⚠️ Warning (test weakness) | The behavior was proven by a spot-check and by UAT test 3. |
| crates/twins-cli/tests/cli_test.rs | — | The SIGINT tests are `#[ignore]` and never run in CI (IN-09) | ℹ️ Info | Manual-only coverage |
| .github/workflows/ci.yml | 14 | The `test` job uses unpinned `dtolnay/rust-toolchain@stable`, so a future clippy can break main again (T-01-18, accepted under D-16) | ℹ️ Info | This caused G-01-4 and can recur. |

### Human Verification Required

#### 1. Remote CI after push (closes G-01-4 / UAT test 4)

**Test:** Push main (19 commits ahead of origin). Then run `gh run list --branch main --limit 1` and `gh run view <run-id>`.
**Expected:** The run is success, and both `test` (stable) and `msrv` (1.90.0) are green.
**Why human:** Needs the user's push. Every job command passes locally on 1.99.0 and 1.90.0.

#### 2. Text report readability re-check (closes G-01-1 / UAT test 1)

**Test:** Run `cargo run -q -p twins-cli -- scan <real folder with duplicates>` in a real terminal, with and without `--verify`.
**Expected:** The progress line redraws in place and is cleared. Groups are separated by a blank line, each group's folder is printed once, every row says `keep`, `keep … (hardlink)` or `remove` with a short path, and the result reads well.
**Why human:** Readability is subjective, and the user raised the gap.

### Gaps Summary

No gaps block the goal:

- **Core pipeline:** the pipeline lives in `twins-core` behind `pipeline::scan`, and the CLI is a thin shell.
- **Progress:** stage events reach any `Observer` in order, and that order is pinned more tightly now.
- **Cancellation:** one `CancelToken` stops every stage, and Ctrl+C gives `scan cancelled`, exit 130 and nothing on stdout.
- **dry_run:** `meta.dry_run` is derived from `RunMode`.
- **Toolchain:** the workspace is green on Rust 1.90.0 and on current stable 1.99.0, including pedantic clippy.
- **G-01-4:** closed in the code and tests. Only the remote CI run is outstanding.
- **G-01-1:** closed in code, exactly as the plan specified, with JSON untouched. Only the user's readability judgment is outstanding.

Code review iteration 4 left two warnings (WR-01, WR-02), both about filenames visually spoofing rows in the text report. They are recorded as advisory: they do not affect the plan, the JSON or the phase goal. Triage them before Phase 2 makes this text the review screen for `twins clean`, either by fixing them or by marking them `deferred` in 01-REVIEW-DISPOSITION.md.

---

_Verified: 2026-10-03T16:18:26Z_
_Verifier: Claude (gsd-verifier)_

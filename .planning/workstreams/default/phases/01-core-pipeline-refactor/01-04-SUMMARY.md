---
phase: 01-core-pipeline-refactor
plan: 04
subsystem: infra
tags: [rust, msrv, rust-1.90, clippy, github-actions, ci]

requires:
  - phase: 01-core-pipeline-refactor
    provides: "01-01 removed the old run.rs Progress struct (the source of the two expected manual_is_multiple_of lints)"
provides:
  - "Workspace MSRV declared as rust-version = \"1.90\" (no rust-toolchain file; local dev stays on stable)"
  - "CI msrv job: builds (all targets) and tests the workspace on dtolnay/rust-toolchain@1.90.0 on every push and PR"
affects: [tauri-app, ci, release-workstream]

actuals:
  tokens: 150
  tasks: 2
  commits: 2
plan_head_before: 7b99dad547ce9bb4e1042dc1ccbcb8763aee82f7
plan_head_after: 247710bbb84130366e45409367e0447c275c35c2

tech-stack:
  added: []
  patterns:
    - "MSRV is enforced two ways: stable clippy is MSRV-aware through rust-version, and the CI msrv job runs build and test on the pinned 1.90.0 toolchain"

key-files:
  created: []
  modified:
    - Cargo.toml
    - .github/workflows/ci.yml
    - crates/twins-core/src/report.rs

key-decisions:
  - "Narrow #[allow(clippy::similar_names)] on civil_from_days rather than renaming doe/doy/yoe. Those are the names in Hinnant's paper, and only the older clippy 1.90 flags them."
  - "The msrv CI job does not run clippy or fmt (D-16). The 1.90 clippy pass is a local, recorded check."

patterns-established:
  - "Lints that only an older clippy reports get a targeted allow with a trailing justifying comment, the same style as the existing allows"

requirements-completed: [CORE-04]

coverage:
  - id: D1
    description: "Workspace declares rust-version 1.90, builds and tests on the 1.90.0 toolchain, and passes clippy on both 1.90.0 and stable"
    requirement: CORE-04
    verification:
      - kind: other
        ref: "grep -n 'rust-version = \"1.90\"' Cargo.toml"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"-D warnings\" cargo +1.90.0 build --workspace --all-targets"
        status: pass
      - kind: unit
        ref: "RUSTFLAGS=\"-D warnings\" cargo +1.90.0 test --workspace (74 tests, 13 binaries, all ok)"
        status: pass
      - kind: other
        ref: "cargo +1.90.0 clippy --workspace --all-targets -- -D warnings"
        status: pass
      - kind: other
        ref: "cargo clippy --workspace --all-targets -- -D warnings (stable 1.98)"
        status: pass
    human_judgment: false
  - id: D2
    description: "CI gains an msrv job on Rust 1.90.0 that runs build and test. The stable test job is unchanged."
    requirement: CORE-04
    verification:
      - kind: other
        ref: "actionlint .github/workflows/ci.yml"
        status: pass
      - kind: other
        ref: "git show --numstat 247710b -- .github/workflows/ci.yml (8 added, 0 deleted)"
        status: pass
    human_judgment: true
    rationale: "No local tool can run the job on GitHub's macos-latest runner. The first push to main or the first PR has to show the msrv job going green."

duration: 3min
completed: 2026-10-02
status: complete
---

# Phase 01 Plan 04: Rust 1.90 MSRV Summary

**The workspace MSRV goes from 1.85 to 1.90, and a new CI msrv job builds and tests on `dtolnay/rust-toolchain@1.90.0`. Build, test and clippy all pass locally on the 1.90.0 toolchain after one narrow `similar_names` allow.**

## Performance

- **Duration:** about 3 min
- **Started:** 2026-10-02T14:33:45Z
- **Completed:** 2026-10-02T14:36:40Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- `Cargo.toml` `[workspace.package]` now declares `rust-version = "1.90"`. No `rust-toolchain*` file exists anywhere, so local development stays on stable.
- Local proof on the installed 1.90.0 toolchain (rustc 1.90.0, 1159e78c4 2025-09-14):
  - `RUSTFLAGS="-D warnings" cargo +1.90.0 build --workspace --all-targets` exits 0.
  - `RUSTFLAGS="-D warnings" cargo +1.90.0 test --workspace` exits 0: 74 tests in 13 binaries, none failed.
- Stable clippy (1.98, MSRV-aware) reports no issues at rust-version 1.90. The `manual_is_multiple_of` hits that research predicted are gone, because 01-01 deleted that code.
- **`cargo +1.90.0 clippy --workspace --all-targets -- -D warnings` passes** after the fix described under Deviations. There is no unknown-lint noise, so Open Question 1 settles cleanly: clippy passes on both 1.90 and stable.
- `.github/workflows/ci.yml` has a new `msrv` job (macos-latest, checkout@v4, rust-toolchain@1.90.0, rust-cache@v2, `cargo build --workspace --all-targets`, `cargo test --workspace`). It inherits `RUSTFLAGS: -D warnings`. The diff adds 8 lines and removes none, and `actionlint` is clean.

## Task Commits

1. **Task 1: Workspace declares and passes on Rust 1.90 (build, test, clippy)**: `30f42f7` (build)
2. **Task 2: CI builds and tests on Rust 1.90 on every push**: `247710b` (ci)

**Plan metadata:** recorded in the docs(01-04) commit that adds this SUMMARY

## Files Created/Modified
- `Cargo.toml`: `rust-version` changed from 1.85 to 1.90.
- `.github/workflows/ci.yml`: `msrv` job appended. The `test` job is byte-for-byte unchanged.
- `crates/twins-core/src/report.rs`: `#[allow(clippy::similar_names)]` with a justifying comment on the private `civil_from_days`.
- `.planning/workstreams/default/phases/01-core-pipeline-refactor/deferred-items.md`: logs a flaky test that predates this plan.

## Decisions Made
- I kept Hinnant's variable names and allowed `similar_names` on that one function. Renaming `doe`/`doy`/`yoe` would make the code harder to check against the published algorithm, which is the case where D-17 permits a targeted allow.
- The 1.90.0 clippy run is not wired into CI. D-16 is unchanged: stable owns clippy and fmt, and msrv owns build and test.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] clippy 1.90 `similar_names` error in a file outside the plan's list**
- **Found during:** Task 1, while running `cargo +1.90.0 clippy`.
- **Issue:** The 1.90 clippy failed with `binding's name is too similar to existing binding` at `crates/twins-core/src/report.rs:219` (`doy` next to `doe`). Stable clippy 1.98 does not report it, because newer clippy narrowed the lint. It is a real lint, not unknown-lint noise from an old clippy, so the plan's rule to "fix any other 1.90 failure" applied.
- **Fix:** `#[allow(clippy::similar_names)] // doe/doy/yoe are the paper's names; clippy 1.90 flags them` on `civil_from_days`. The allow is scoped to that one function, and no blanket allow was added. `report.rs` is not in the `files_modified` of any sibling plan (01-02, 01-03), so there is no merge overlap.
- **Files modified:** `crates/twins-core/src/report.rs`
- **Verification:** clippy on 1.90.0 and on stable both exit 0 with `-D warnings`, and fmt is clean.
- **Committed in:** `30f42f7` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1).
**Impact on plan:** A single attribute line on a private function. Behavior is unchanged and the scope did not grow.

## Issues Encountered
- **The stable test `reports_progress_per_stage` is flaky, and the flakiness predates this plan.** The first stable `cargo test --workspace` failed at `crates/twins-core/tests/group_test.rs:216` (`left: Some((1, 2))`, `right: Some((2, 2))`). Ten isolated reruns failed 3 times. It comes from out-of-order progress callbacks across worker threads in `group::find`, which this plan does not touch. A rust-version bump has no effect on runtime behavior, and the 1.90 test run passed. The rerun of the full stable suite passed (13 of 13 binaries ok). I logged it in `deferred-items.md` for plan 01-02, which owns `find.rs` and `group_test.rs`. The CI jobs are exposed to the same flake until 01-02 or a later plan fixes it.

## Plan-level Verification
- `cargo fmt --all --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings` (stable): pass, "No issues found"
- `cargo test --workspace` (stable): pass on rerun. The first run hit the intermittent failure described above.
- `cargo +1.90.0 test --workspace` with `RUSTFLAGS="-D warnings"`: pass
- `cargo +1.90.0 clippy --workspace --all-targets -- -D warnings`: pass
- `actionlint .github/workflows/ci.yml`: pass, no findings
- `find . -name 'rust-toolchain*'` (excluding target, .git and .worktrees): prints nothing

## User Setup Required
None. No external service configuration is required.

## Next Phase Readiness
- Success criterion 5 is met locally. The CI msrv job is the remaining proof and runs on the next push or PR.
- Concern: the flaky `reports_progress_per_stage` can turn either CI job red at random until it is fixed. See deferred-items.md.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*

## Self-Check: PASSED
- FOUND: Cargo.toml contains rust-version = "1.90"
- FOUND: .github/workflows/ci.yml contains dtolnay/rust-toolchain@1.90.0
- FOUND: commit 30f42f7 (build(01-04))
- FOUND: commit 247710b (ci(01-04))

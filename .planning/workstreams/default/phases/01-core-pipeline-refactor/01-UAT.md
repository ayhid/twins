---
status: complete
phase: 01-core-pipeline-refactor
source: [01-VERIFICATION.md]
started: 2026-10-02T21:25:07Z
updated: 2026-10-03T15:29:01Z
---

## Current Test

[testing complete]

## Tests

### 1. Terminal progress look and wording
expected: Single in-place progress line by stage with correct [k/N] counters, cleared before the report; also shown on stderr with --json; wording reads well.
result: issue
reported: "first feedback it's not very readble"
severity: minor
notes: "Text report observed (raw bytes): groups run together with no blank line between them, every row repeats the full absolute path, the ★ keep marker has no legend and removal rows carry no marker. Progress line itself not commented on. (Images attached to the report were the scanned PNG; the terminal paste turned a path into an image, not a twins bug.)"

### 2. Ctrl+C on a real tree
expected: Ctrl+C during `twins scan ~` prints `scan cancelled` and exits 130 (`echo $?`); Ctrl+C during `twins report ~ > out.json` leaves out.json empty.
result: pass

### 3. Double Ctrl+C on a slow or external volume
expected: Pressing Ctrl+C twice quickly during a scan of a slow/external volume exits 130 immediately.
result: pass
notes: "Retest on /Volumes/Le Divan du monde: prints `scan cancelled`, exit=130."

### 4. CI green on GitHub
expected: After pushing main (currently ahead of origin), both the `test` and the new `msrv` (Rust 1.90.0) jobs pass.
result: issue
reported: "push → CI run 37133285179: msrv success, test failure (clippy fails on crates/twins-core/tests/observe_test.rs:238, lint clippy::assert_is_empty)"
severity: blocker

## Summary

total: 4
passed: 2
issues: 2
pending: 0
skipped: 0
blocked: 0

## Gaps

- gap_id: G-01-1
  truth: "The text scan report is easy to read: groups are visually separated, the keep/remove roles are clear and paths are not needlessly repeated"
  status: failed
  reason: "User reported: first feedback it's not very readble"
  severity: minor
  test: 1
  artifacts: []
  missing: []

- gap_id: G-01-4
  truth: "CI is green on main: both the `test` job (stable toolchain, clippy -D warnings) and the `msrv` job pass"
  status: failed
  reason: "User reported: push → CI run 37133285179: msrv success, test failure (clippy::assert_is_empty at crates/twins-core/tests/observe_test.rs:238)"
  severity: blocker
  test: 4
  artifacts: []
  missing: []

---
status: testing
phase: 01-core-pipeline-refactor
source: [01-VERIFICATION.md]
started: 2026-10-02T21:25:07Z
updated: 2026-10-03T16:20:34Z
---

## Current Test

number: 1
name: Text report readability (re-test after 01-09, gap G-01-1)
expected: |
  `twins scan` on a real folder: each row is labelled `keep`, `keep … (hardlink)` or `remove`; each group's folder is printed once and rows show paths relative to it; groups are separated by one blank line; the report reads well. Progress line still single-line by stage and cleared before the report.
awaiting: user response

## Tests

### 1. Text report readability (re-test after 01-09, gap G-01-1)
expected: `twins scan` on a real folder prints keep / keep (hardlink) / remove labelled rows, one folder line per group with relative paths, one blank line between groups, and reads well. Progress line unchanged.
result: [pending]
previous: "issue — first feedback it's not very readble (closed in code by 01-09)"

### 2. Ctrl+C on a real tree
expected: Ctrl+C during `twins scan ~` prints `scan cancelled` and exits 130 (`echo $?`); Ctrl+C during `twins report ~ > out.json` leaves out.json empty.
result: pass

### 3. Double Ctrl+C on a slow or external volume
expected: Pressing Ctrl+C twice quickly during a scan of a slow/external volume exits 130 immediately.
result: pass
notes: "Retest on /Volumes/Le Divan du monde: prints `scan cancelled`, exit=130."

### 4. CI green on GitHub (re-test after 01-08, gap G-01-4)
expected: After pushing main (ahead of origin), both the `test` (stable, clippy -D warnings) and `msrv` (Rust 1.90.0) jobs pass (`gh run view`).
result: [pending]
previous: "issue — CI run 37133285179 test job failed on clippy::assert_is_empty (closed in code by 01-08)"

## Summary

total: 4
passed: 2
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps

Gaps G-01-1 and G-01-4 were closed in code by gap plans 01-09 and 01-08 (see 01-VERIFICATION.md); tests 1 and 4 re-confirm them.

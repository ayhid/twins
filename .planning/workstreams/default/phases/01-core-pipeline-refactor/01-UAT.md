---
status: testing
phase: 01-core-pipeline-refactor
source: [01-VERIFICATION.md]
started: 2026-10-02T21:25:07Z
updated: 2026-10-02T21:25:07Z
---

## Current Test

number: 1
name: Terminal progress look and wording
expected: |
  In a real terminal, `twins scan ~/Downloads` (also with `--verify` and with `--json > /dev/null`) redraws one progress line in place: [1/4] walk, [2/4] size grouping, [3/4] partial hash, [4/4] full hash ([5/5] verify with --verify). The line clears before the report. The "N candidates" wording reads naturally.
awaiting: user response

## Tests

### 1. Terminal progress look and wording
expected: Single in-place progress line by stage with correct [k/N] counters, cleared before the report; also shown on stderr with --json; wording reads well.
result: [pending]

### 2. Ctrl+C on a real tree
expected: Ctrl+C during `twins scan ~` prints `scan cancelled` and exits 130 (`echo $?`); Ctrl+C during `twins report ~ > out.json` leaves out.json empty.
result: [pending]

### 3. Double Ctrl+C on a slow or external volume
expected: Pressing Ctrl+C twice quickly during a scan of a slow/external volume exits 130 immediately.
result: [pending]

### 4. CI green on GitHub
expected: After pushing main (currently ahead of origin), both the `test` and the new `msrv` (Rust 1.90.0) jobs pass.
result: [pending]

## Summary

total: 4
passed: 0
issues: 0
pending: 4
skipped: 0
blocked: 0

## Gaps

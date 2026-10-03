---
phase: 01-core-pipeline-refactor
plan: 09
subsystem: report
tags: [rust, text-report, cli, uat-gap, tdd]

requires:
  - phase: 01-core-pipeline-refactor
    provides: "01-08: clippy 1.99 clean workspace on the updated stable toolchain"
provides:
  - "twins scan text report with labelled keep / keep (hardlink) / remove rows"
  - "Each group's shared folder printed once, rows relative to it; full paths when only / is shared"
  - "One blank line between groups; control characters in paths printed escaped"
affects: [02-safe-deletion, clean, uat-01]

actuals:
  tokens: 2602
  tasks: 2
  commits: 4
plan_head_before: d36a0c3423d0fd99d6b029b86309f403792ab788
plan_head_after: 1a18001c8abeb8ed2956aad25965701f3ecdcca5

tech-stack:
  added: []
  patterns:
    - "Text roles derive only from GroupEntry.keep / GroupEntry.remove (never recomputed), so text and JSON cannot disagree"
    - "Human-facing paths go through printable(), which escapes char::is_control chars via escape_debug"

key-files:
  created: []
  modified:
    - crates/twins-core/src/report.rs
    - crates/twins-core/tests/report_test.rs
    - crates/twins-cli/tests/cli_test.rs

key-decisions:
  - "Paths are shortened against each group's shared folder, not the scanned root: Report.roots keeps roots as typed (e.g. `.`), so a root match would need cwd or canonicalization inside a pure renderer"
  - "shared_folder returns None when the common prefix has at most one component, so a group spanning /a and /b shows full paths and no `//` line"
  - "CORE-01 left Pending in REQUIREMENTS.md: phase 01 still awaits human UAT (verification_deferred_human), and shared artifacts are the orchestrator's to update"

patterns-established:
  - "Exact-format assert_eq! tests for the text report (core sample + CLI characterization) instead of contains() predicates"

requirements-completed: [CORE-01]

coverage:
  - id: D1
    description: "Every text row is labelled keep, keep (hardlink) or remove, with roles taken only from the report's keep/remove fields"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/report_test.rs#text_labels_every_row_and_shows_the_group_folder"
        status: pass
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#scan_text_marks_kept_file_and_never_lists_hardlinks_to_remove"
        status: pass
    human_judgment: false
  - id: D2
    description: "Each group's shared folder printed once with rows relative to it, proven end to end through twins scan"
    requirement: CORE-01
    verification:
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#characterize_text_report"
        status: pass
    human_judgment: false
  - id: D3
    description: "Groups separated by one blank line; members sharing only / show full paths with no folder line"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/report_test.rs#two_groups_are_separated_by_one_blank_line"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/report_test.rs#members_sharing_only_the_root_show_full_paths"
        status: pass
    human_judgment: false
  - id: D4
    description: "A control character in a path is shown escaped, so a filename cannot forge a keep or remove row"
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/report_test.rs#control_characters_in_paths_stay_on_one_row"
        status: pass
    human_judgment: false
  - id: D5
    description: "JSON report schema v1 byte-identical"
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/report_test.rs#json_schema_is_stable"
        status: pass
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#characterize_json_report_with_relative_root"
        status: pass
    human_judgment: false
  - id: D6
    description: "The text report reads well on a real folder (UAT test 1 re-check, gap G-01-1)"
    verification: []
    human_judgment: true
    rationale: "Readability is a human judgment; the plan's <human-check> asks the user to re-run UAT test 1 via /gsd-verify-work 01"

duration: 5min
completed: 2026-10-03
status: complete
---

# Phase 01 Plan 09: Readable Text Report Summary

**`twins scan` text rows now say `keep`, `keep … (hardlink)` or `remove`. Each group's folder is printed once with rows relative to it, groups are separated by a blank line, and control characters are escaped. JSON schema v1 is byte-identical.**

## Performance

- **Duration:** about 5 min
- **Started:** 2026-10-03T16:01:51Z
- **Completed:** 2026-10-03T16:06:37Z
- **Tasks:** 2 of 2
- **Files modified:** 3

## Accomplishments

- Closed UAT gap G-01-1. The kept file, hardlinks of it and removals are labelled on every row, and the unexplained star is gone.
- Each group's shared folder is printed once on a line ending in `/`, with rows relative to it. A group spanning unrelated top-level folders shows full paths and no folder line.
- One blank line between groups and one before the totals line. The empty-report message is unchanged.
- A newline or other control character in a filename prints escaped (`\n`), so a filename cannot forge a `keep` row (T-01-21).
- `Report`, `Summary`, `GroupEntry`, `FileEntry`, `build`, `write_json`, `VERSION` and the `write_text` signature are unchanged. `run.rs` needed no change.

## Sample Output

`cargo run -q -p twins-cli -- scan <tmp>/sample` on a tree with `photos/beach.jpg`, its hardlink `photos/beach-link.jpg`, a copy `photos/2024/beach-copy.jpg`, and a second pair under `backup/photos/`:

```text
[1] 2.0 MiB × 3  (2.0 MiB reclaimable)
  /private/tmp/claude-501/-Users-ayoub-projects-twins/84df560b-f76f-4816-9b58-90875e84b341/scratchpad/sample/photos/
    keep    beach-link.jpg
    keep    beach.jpg  (hardlink)
    remove  2024/beach-copy.jpg

[2] 1.0 MiB × 2  (1.0 MiB reclaimable)
  /private/tmp/claude-501/-Users-ayoub-projects-twins/84df560b-f76f-4816-9b58-90875e84b341/scratchpad/sample/backup/photos/
    keep    notes.pdf
    remove  notes-old.pdf

2 groups, 2 duplicates, 3.0 MiB reclaimable (5 files scanned)
```

## Task Commits

1. **Task 1 (tracer): labelled rows under a per-group folder line, end to end**
   - RED `f4cc73a` (test): expect labelled rows under a per-group folder in the text report
   - GREEN `076c8c9` (feat): label keep and remove rows and print each group's folder once
2. **Task 2: separation, root-only fallback, control-character escaping**
   - RED `83146ef` (test): expect separated groups, a root-only fallback and escaped control characters
   - GREEN `1a18001` (feat): separate groups, fall back to full paths and escape control characters in the text report

No REFACTOR commits were needed.

## TDD Gate Compliance

| | Task 1 | Task 2 |
|---|---|---|
| RED commit | `f4cc73a` | `83146ef` |
| RED evidence (`check tdd-red-evidence`) | RED_EVIDENCE_OK for both targets, `text_labels_every_row_and_shows_the_group_folder` and `characterize_text_report` (exit 101) | RED_EVIDENCE_OK for all three targets, `two_groups_are_separated_by_one_blank_line`, `members_sharing_only_the_root_show_full_paths` and `control_characters_in_paths_stay_on_one_row` (exit 101) |
| GREEN commit | `076c8c9` | `1a18001` |

The checker reads only TAP or Surefire. As in 01-02 and 01-06, a script ran the real `cargo test` command, turned each `test NAME ... ok|FAILED` line into a TAP line, and kept the raw cargo output in the record. The records are in the session scratchpad, not the repo. Every RED failure hit the intended `assert_eq!` on the expected text, with no compile error:
- Task 1 core: the old output was `  ★ /r/a.bin\n    /r/b.bin\n    /r/c.bin`, with full paths, a star and blank markers.
- Task 1 CLI: `Unexpected stdout`, still the starred full-path layout. `scan_text_marks_kept_file_and_never_lists_hardlinks_to_remove` also failed at RED, on the new row predicates.
- Task 2: no blank line between `[1]` and `[2]`, a `  //` folder line with rows `a/x.bin`, and a raw newline that split the remove row into a forged `    keep    x.bin` row.

Tracer gate: auto mode was active. After GREEN, all three Task 1 `<verify>` commands passed, so expansion went ahead.

## Files Created/Modified

- `crates/twins-core/src/report.rs`: new `write_text` body and private helpers `Role` (Keep, Hardlink, Remove, read from `keep` and `remove`), `shared_folder` (common component prefix of the parents, `None` at one component or fewer) and `printable` (`escape_debug` for control chars). The doc comment describes the new layout.
- `crates/twins-core/tests/report_test.rs`: `text_marks_the_kept_file_and_summarises` renamed to `text_labels_every_row_and_shows_the_group_folder`, now an exact `assert_eq!`. Three new exact-format tests and the helpers `report_of`, `text_of` and `member`.
- `crates/twins-cli/tests/cli_test.rs`: `characterize_text_report` now expects the new layout exactly. `scan_text_marks_kept_file_and_never_lists_hardlinks_to_remove` checks the remove row and the hardlink row, and that neither `a.bin` nor `a-link.bin` is listed for removal.

## Decisions Made

- Paths are shortened against each group's shared folder rather than the scanned root, as the plan asked. The folder comes from the report alone and needs no cwd or `~` lookup.
- `shared_folder` follows the plan's rule of at most one component, so it also returns `None` for a single relative component. In practice member paths are always absolute.
- REQUIREMENTS.md is not modified. `requirements.ready-ids` reports CORE-01 as ready, but phase 01 is still `verification_deferred_human`. The orchestrator owns shared-artifact writes after the wave, and CORE-01 stays Pending until phase verification.

## Deviations from Plan

None. The plan was executed as written.

The plan-commit ledger (protocol 0c) could not be written to the worktree's git dir, because the isolation guard rejects that command. The base `d36a0c3` was recorded from the dispatch instead, and `commits: 4` was measured with `git rev-list --count d36a0c3..HEAD`.

## Verification

- `cargo fmt --all --check`: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` on rustc 1.99.0: no issues
- `cargo test --workspace` on stable: 134 passed, 2 ignored (the SIGINT tests)
- `RUSTFLAGS="-D warnings" cargo +1.90.0 test --workspace`: every suite ok
- `json_schema_is_stable` and `characterize_json_report_with_relative_root` pass and are unmodified. The only diff mention of the latter is a hunk header.
- Acceptance greps pass: there is no `★` in code, docs or tests, `(hardlink)` and `escape_debug` are present, and the signature and `VERSION` are unchanged.

## Issues Encountered

None.

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness

- G-01-1 is closed in code. The remaining step is the human check: re-run UAT test 1 with `/gsd-verify-work 01` and confirm `twins scan` reads well on a real folder.
- Phase 2's clean can show the same labelled rows for its dry-run view.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-03*

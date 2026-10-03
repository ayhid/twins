---
phase: 01
review: 01-REVIEW.md
titles: json
findings:
  - id: WR-01
    severity: warning
    disposition: open
    title: "`printable` lets Unicode separators, bidi controls and invisible characters through, so a filename can disguise or visually split its row"
  - id: WR-02
    severity: warning
    disposition: open
    title: "A filename padded with spaces soft-wraps in the terminal and renders a forged `keep` row at column 0"
  - id: IN-01
    severity: info
    disposition: open
    title: "Any member missing from `keep` / `remove` is labelled `(hardlink)` without checking its inode"
  - id: IN-02
    severity: info
    disposition: open
    title: "Roles are matched on lossy path strings"
  - id: IN-03
    severity: info
    disposition: open
    title: "The text layout test for the hardlink row relies on a path tiebreak that is not documented"
  - id: IN-04
    severity: info
    disposition: open
    title: "`MissingKeepDir` still comes before root validation, contrary to the `scan` doc"
  - id: IN-05
    severity: info
    disposition: open
    title: "The keeper and the walk each compute `normalise_roots`, so \"must never diverge\" holds only by convention"
  - id: IN-06
    severity: info
    disposition: open
    title: "The case-insensitive keep-dir test passes silently on case-sensitive volumes"
  - id: IN-07
    severity: info
    disposition: open
    title: "A keep dir under a path the walk skips is accepted, and the scan quietly does not use it (carried forward, iteration-3 IN-11)"
  - id: IN-08
    severity: info
    disposition: open
    title: "A root that is a symlink nested in another root is silently never scanned (carried forward, iteration-3 IN-12)"
  - id: IN-09
    severity: info
    disposition: open
    title: "Iteration-3 findings still open in reviewed files (carried forward)"
  - id: IN-10
    severity: info
    disposition: open
    title: "`Role` sorting puts hardlink rows between keep and remove, but the header count includes them as copies"
open: 12
total: 12
recorded: 2026-10-03T16:14:25.536Z
---

# Phase 01: Code Review Disposition

| Finding | Severity | Disposition | Source |
|---------|----------|-------------|--------|
| WR-01 | warning | open | - |
| WR-02 | warning | open | - |
| IN-01 | info | open | - |
| IN-02 | info | open | - |
| IN-03 | info | open | - |
| IN-04 | info | open | - |
| IN-05 | info | open | - |
| IN-06 | info | open | - |
| IN-07 | info | open | - |
| IN-08 | info | open | - |
| IN-09 | info | open | - |
| IN-10 | info | open | - |

Dispositions: `open` (recorded, not yet triaged), `fixed`, `skipped`, `deferred`.
Set `deferred` by hand and put the reason in the Source cell; both are preserved. A `|` in the reason is kept as prose and escaped on the next run.
Re-running the gate keeps every row it can. A row the current review no longer reports is kept and its Source cell flagged, so a finding does not leave this record silently. ONE exception: when a finding id is REUSED by a different finding, the earlier decision cannot keep a row — the id is taken — and it is dropped. A RECORDED decision (anything but `open`) is named on the console when that happens; a row still at `open` is replaced silently, because `open` records no decision to lose.

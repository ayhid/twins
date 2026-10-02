---
phase: 01-core-pipeline-refactor
fixed_at: 2026-10-02T21:16:02Z
review_path: .planning/workstreams/default/phases/01-core-pipeline-refactor/01-REVIEW.md
iteration: 3
findings_in_scope: 1
fixed: 1
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-10-02T21:16:02Z
**Source review:** .planning/workstreams/default/phases/01-core-pipeline-refactor/01-REVIEW.md
**Iteration:** 3

**Summary:**
- Findings in scope: 1 (WR-01). The fix scope is critical_warning, so the 12 Info findings are out of scope. IN-10 was resolved anyway, as a side effect of the WR-01 fix.
- Fixed: 1
- Skipped: 0

**Verification:** every gate ran in the main checkout (`/Users/ayoub/projects/twins`, branch `main`, no worktree), on the final tree:
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` (131 passed, 2 ignored) all pass.
- I ran the three new regression tests against the pre-fix source and all three failed there. The two nested-root tests kept `a.bin`, the wrong copy. The IN-10 test got `KeepDirOutsideRoots` instead of `NoRoots`.

## Earlier iterations (summary)

- **Iteration 1:** fixed four warnings.
  - WR-01: keep dir absolutized, `fc41ba7`. This was only a partial fix.
  - WR-02: `Ticker` for hash progress, `3ae882f`.
  - WR-03: `Throttle` behind a `Mutex<Gate>`, `94462b9`.
  - WR-04: a repeated error source printed once, `d336d52`.
- **Iteration 2:** WR-01, `e8c1c54`. `keeper` now resolves the keep dir on disk (canonicalize) and rebuilds it under the walk's spelling of the first root that contains it. This covers symlinks, `/private`, letter case and a relative path. It also added `KeepDirOutsideRoots` and `KeepDir { path, source }`. The iteration-3 review confirmed this work but found that roots were still taken in caller order. This iteration fixes that.

## Fixed Issues

### WR-01: The in-dir keep dir can still be mapped onto a root the walk drops, so the copy in the keep dir is planned for removal

**Files modified:** `crates/twins-core/src/scan/walk.rs`, `crates/twins-core/src/scan/mod.rs`, `crates/twins-core/src/pipeline.rs`, `crates/twins-core/tests/pipeline_test.rs`
**Commit:** 04f1f90
**Status:** fixed: requires human verification (plan logic: which copy survives)
**Applied fix:**
- `walk::normalise_roots` is now `pub(crate)` and re-exported as `scan::normalise_roots`. Its doc now says that it returns exactly the roots the walk descends, and that `pipeline` depends on this.
- `pipeline::keeper` no longer takes roots from `spec.walk_options().roots()` in caller order. It calls `scan::normalise_roots(spec.walk_options())`, the same function `walk_observed` calls on the same roots. The roots then come out validated and sorted, with nested roots dropped. The keep dir is mapped onto the first walked root whose canonical path contains it, so the keeper and the walk agree by construction.
  - For roots `[R/shortcut, R]` (with `shortcut -> keep`) and for `[R/KEEP, R]`, the keep dir is now rebuilt as `R/keep`, which is the name the walk gives those files.
  - Suppose a nested root is a symlink that points outside the outer root (the IN-12 case) and the user names it as the keep dir. The walk never scans it, so the keeper now fails with `KeepDirOutsideRoots`. Before, it would have mapped the keep dir onto a root the walk drops.
- Order of checks for in-dir: `MissingKeepDir` first (a spec error with no I/O), then root validation, then keep dir resolution, then mapping.
- IN-10 is fixed as a side effect. With in-dir selected, invalid roots now fail as `Scan(NoRoots | ProtectedRoot | NotDirectory | RemoteRoot | Io)` before any keep dir error, the same as with other strategies. Strategies other than in-dir are unchanged: they still return before reading roots or the keep dir.
- Docs updated: the `keeper` comment, the `# Errors` section of `scan` (roots are validated first), and `ScanSpec::keep_dir` (a nested root maps onto the outer root).
- New tests in `pipeline_test.rs`:
  - `in_dir_maps_the_keep_dir_past_a_nested_symlinked_root`: roots `[R/shortcut, R]` and `[R, R/shortcut]`, with keep dir `R/shortcut`. Asserts that `R/keep/b.bin` is kept and `R/a.bin` is removed. Fails on the pre-fix code.
  - `in_dir_maps_the_keep_dir_past_a_nested_root_in_a_different_case`: roots `[R/KEEP, R]` and `[R, R/KEEP]`, with keep dir `R/keep`. Runs only on a case-insensitive volume and returns early otherwise. Fails on the pre-fix code on this machine (APFS, case-insensitive).
  - `in_dir_reports_root_errors_before_keep_dir_errors` (IN-10): no roots gives `NoRoots`, `/usr` gives `ProtectedRoot`, and a file root gives `NotDirectory`. Each case emits only `Finished`. Fails on the pre-fix code.
  - The existing `in_dir_fails_on_a_root_it_cannot_resolve` still passes. A missing root now fails at the walk's own `stat` check, still as `Scan(Io)` naming the path.
- Notes for the reviewer:
  - The roots are validated twice per in-dir run, once in `keeper` and once in the walk. The check is cheap and only runs for in-dir. Passing the list into the walk would have widened the change to `walk_observed`'s signature, so I left it.
  - IN-11 (a keep dir under an excluded path) and IN-12 (a nested symlinked root that is never scanned) are not addressed. They are Info findings and out of scope. Because of this fix, the IN-12 case can no longer silently mislead the in-dir keeper: it fails loudly instead.

---

_Fixed: 2026-10-02T21:16:02Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 3_

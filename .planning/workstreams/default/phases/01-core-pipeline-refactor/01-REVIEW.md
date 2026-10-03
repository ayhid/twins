---
phase: 01-core-pipeline-refactor
reviewed: 2026-10-03T16:12:39Z
depth: standard
iteration: 4
files_reviewed: 10
files_reviewed_list:
  - crates/twins-cli/tests/cli_test.rs
  - crates/twins-core/src/pipeline.rs
  - crates/twins-core/src/report.rs
  - crates/twins-core/src/scan/mod.rs
  - crates/twins-core/src/scan/walk.rs
  - crates/twins-core/tests/group_test.rs
  - crates/twins-core/tests/keep_test.rs
  - crates/twins-core/tests/observe_test.rs
  - crates/twins-core/tests/pipeline_test.rs
  - crates/twins-core/tests/report_test.rs
findings:
  critical: 0
  warning: 2
  info: 10
  total: 12
status: issues_found
---

# Phase 01: Code Review Report (iteration 4)

**Reviewed:** 2026-10-03T16:12:39Z
**Depth:** standard
**Files Reviewed:** 10
**Status:** issues_found

## Summary

This is an incremental review of `96a4f4d..HEAD`. It covers three changes:

- **The iteration-3 WR-01 fix.** `keeper` now maps the keep dir onto `scan::normalise_roots`.
- **Plan 01-08.** The clippy 1.99 `assert_is_empty` rewrites in the tests.
- **Plan 01-09.** The new keep/remove text layout in `report.rs`.

Gates on the current tree: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all pass.

**Verified as correct:**

- **WR-01 (iteration 3) is resolved.** `keeper` iterates the same de-duplicated, sorted and validated list the walk descends (`pipeline.rs:316`, `walk.rs:267-285`). A nested root such as `R/shortcut` or `R/KEEP` can no longer be the mapping target. The new tests `in_dir_maps_the_keep_dir_past_a_nested_symlinked_root` and `..._in_a_different_case` cover both root orders. I also checked that `PathBuf` ordering is component-wise, so every descendant of a root sorts directly after it, and the `out.last()`-only comparison still drops every nested root.
- **IN-10 (iteration 3) is mostly resolved.** Root errors (`NoRoots`, `ProtectedRoot`, `NotDirectory`) now come before keep-dir errors, and a test covers this. One leftover case is listed as IN-04.
- **The JSON report is unchanged.** The diff only touches `write_text`, its private helpers and the doc comment. `build`, `to_entry`, `to_file`, `display` and `write_json` are byte-identical, and `json_schema_is_stable` was not modified and still passes.
- **Text roles come only from `GroupEntry::keep` / `remove`.** `Role::of` (`report.rs:229-238`) reads nothing else. Because `Keeper::choose` (`keep.rs:91-99`) removes every member whose identity differs from the keeper's, any member that is neither `keep` nor in `remove` shares the kept file's identity, so the `keep` label is accurate for pipeline output.
- **A filename cannot add a logical line.** Every `char::is_control` character, including `\n`, `\r`, ESC and U+0085, is escaped. The test asserts `lines().count()`.
- **The 01-08 test rewrites keep their meaning.** In `observe_test`, the non-empty check became an exact stage-sequence assertion, which is stronger than before.

**Key concerns:**

The escaping in `printable` only blocks line breaks that `str::lines` sees. Two things still let a filename visually forge or disguise a row:

- **WR-01:** Unicode line and paragraph separators, bidi overrides and zero-width characters pass through unescaped. I verified each one against `is_control` / `escape_debug` and confirmed that APFS accepts such names.
- **WR-02:** a run of plain spaces makes the terminal soft-wrap, so a forged `    keep    x.bin` row starts at column 0.

Neither changes the plan itself. But the dry-run text report is how a user decides whether to delete, and plan 01-09's stated invariant is that a filename must not be able to forge a row.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: `printable` lets Unicode separators, bidi controls and invisible characters through, so a filename can disguise or visually split its row

**File:** `crates/twins-core/src/report.rs:260-272`
**Issue:** `printable` escapes only `char::is_control()`, which is general category Cc. I checked the following characters against the toolchain and created such names on APFS in the scratchpad: Python `open('x\u2028y')` and `open('x\u202ey')` both succeed. Every one of them has `is_control() == false`, so `printable` passes it through verbatim:

- U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR. Webviews (the planned Tauri app), many editors and some pagers render them as line breaks. A name like `evil\u{2028}    keep    x.bin` then shows a second row labelled `keep`.
- U+202A-U+202E and U+2066-U+2069, the bidi embeddings, overrides and isolates. Trojan Source-style, a `remove` row can display a path that reads like a different file, for example the kept file's name. It can also visually reorder its `  (hardlink)` suffix.
- U+200B-U+200F (zero-width characters, LRM/RLM), U+2060-U+2064 and U+FEFF. Two members of one group can then display as byte-for-byte identical names, one labelled `keep` and one `remove`.

A related problem: `\` itself is not escaped, so the escaped form is ambiguous. A file literally named `a\nb` (backslash, `n`) and a file named `a<LF>b` both print as `a\nb`.

The function's doc and the 01-09 goal ("a filename must not be able to forge a row") are therefore only met for consumers that split on Cc line breaks.

**Fix:** Escape the separator, format and bidi ranges as well, and escape the backslash so the output stays unambiguous. Do not switch to `char::escape_debug` for every character. It also escapes grapheme extenders such as U+0301, which would mangle legitimate NFD names (common on HFS+-era files).

```rust
fn printable(path: &Path) -> String {
    let mut s = String::new();
    for c in path.to_string_lossy().chars() {
        let invisible = matches!(c,
            '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{2069}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}');
        if c == '\\' {
            s.push_str("\\\\");
        } else if c.is_control() || invisible {
            s.extend(c.escape_debug());
        } else {
            s.push(c);
        }
    }
    s
}
```

Add tests with `\u{2028}`, `\u{202E}`, `\u{200B}` and a literal backslash.

### WR-02: A filename padded with spaces soft-wraps in the terminal and renders a forged `keep` row at column 0

**File:** `crates/twins-core/src/report.rs:192-204`; test `crates/twins-core/tests/report_test.rs:196-212`
**Issue:** Spaces are printed verbatim. Take a member named `evil` followed by enough spaces to reach the right margin, then `    keep    x.bin`. Its row `    remove  evil<spaces>    keep    x.bin` is one logical line, but the terminal wraps it. The continuation line starts at column 0 with `    keep    x.bin`, identical to a real keep row.

- **Reachability:** APFS NAME_MAX is 255 bytes. The row prefix is 12 columns, plus the relative path, so a single path component can forge rows on terminals up to about 230 columns wide. That includes the default 80-column Terminal.
- **The suffix can be forged too:** the `  (hardlink)` marker is trailing free text, so a kept file named `a  (hardlink)` is indistinguishable from a hardlink row.
- **The test misses it:** `control_characters_in_paths_stay_on_one_row` asserts only `lines().count()`, so it cannot catch a visual forgery.

This matters because the text report is the human review surface before a delete. The plan is unaffected, but the user can be misled about which copy survives.

**Fix:** Make the path column unforgeable:

- Quote any path that contains a run of two or more spaces, leading or trailing whitespace, or any escaped character, the way `ls --quoting-style=shell-escape` does, for example `remove  'evil      keep    x.bin'`.
- Move the hardlink marker into the fixed-width label column instead of a trailing suffix, for example `keep*` or a `link` label, so that no filename text can imitate it.
- Extend the test to assert that every printed path is quoted when it contains a space run.

## Info

### IN-01: Any member missing from `keep` / `remove` is labelled `(hardlink)` without checking its inode

**File:** `crates/twins-core/src/report.rs:229-238`
**Issue:** The fallback branch assumes that "not keep and not removed" means "a hardlink of the kept file". That is not always true:

- **Overlapping roots spelled differently.** With roots such as `/tmp/x` and `/private/tmp/x`, which the walk visits twice (iteration-3 summary), the second spelling is the same file, not a hardlink, yet it is shown as `keep  ...  (hardlink)`.
- **External or inconsistent reports.** `Report` is `pub` and `Deserialize`, and the app is expected to consume it. A report from any other source, or an inconsistent one, gets every unlisted member silently rendered as kept.

`FileEntry` carries `inode`, so the writer can verify the claim.
**Fix:** Look up the keep entry's inode in `g.files`. Label a member `Hardlink` only when `f.inode == keep_inode`, and otherwise use a distinct role such as `unplanned` (or `debug_assert!`). Consider a `(same file)` suffix when the inode matches and the paths are aliases.

### IN-02: Roles are matched on lossy path strings

**File:** `crates/twins-core/src/report.rs:140-142`, `crates/twins-core/src/report.rs:229-233`
**Issue:** `display` uses `to_string_lossy`. Two members whose names differ only in invalid UTF-8 bytes collapse to the same string, and `Role::of` then labels both `keep`. APFS and HFS+ reject such names (I confirmed `EILSEQ` on APFS), so this is reachable only on FUSE-style local volumes. The JSON has the same ambiguity, which predates this phase.
**Fix:** Document the assumption next to `Role::of`, or make `display` lossless, for example by escaping invalid bytes as `\xNN`. Only paths that are already lossy would change, so the JSON for valid UTF-8 paths stays the same.

### IN-03: The text layout test for the hardlink row relies on a path tiebreak that is not documented

**File:** `crates/twins-cli/tests/cli_test.rs:121-124`, `crates/twins-cli/tests/cli_test.rs:266-272`
**Issue:** The expected output `keep    a-link.bin` / `keep    a.bin  (hardlink)` holds only because the hardlinks share an mtime and `"a-link.bin" < "a.bin"` in the final path tiebreak (`keep.rs:120-122`). This is correct today, but the characterization test silently depends on that ordering, and the negative assertions (`"remove  a.bin"` and `"remove  a-link.bin"` absent) are redundant given the exact-match test.
**Fix:** Add a comment in the test naming the tiebreak, so that a future strategy change is understood as intentional churn rather than a regression.

### IN-04: `MissingKeepDir` still comes before root validation, contrary to the `scan` doc

**File:** `crates/twins-core/src/pipeline.rs:238-244`, `crates/twins-core/src/pipeline.rs:312-316`
**Issue:** The doc says the keep dir is "checked before the walk, after the roots are validated". However, `keeper` returns `MissingKeepDir` (line 315) before it calls `normalise_roots` (line 316). With `InDir`, no keep dir and no roots or a protected root, the user is told the keep dir is missing instead of `NoRoots` or `ProtectedRoot`. This is the leftover of iteration-3 IN-10.
**Fix:** Call `scan::normalise_roots` first, or change the doc to say that a missing keep directory is reported first. Add the case to `in_dir_reports_root_errors_before_keep_dir_errors`.

### IN-05: The keeper and the walk each compute `normalise_roots`, so "must never diverge" holds only by convention

**File:** `crates/twins-core/src/pipeline.rs:316`, `crates/twins-core/src/scan/walk.rs:262-266`
**Issue:** `keeper` and `scan::walk_observed` call `normalise_roots` separately. Each root is therefore stat'ed and statfs'ed twice. The keeper's list and the walk's list can also differ if the filesystem changes in between, for example a root replaced by a symlink or removed. The doc comment on `normalise_roots` states the invariant, but nothing enforces it.
**Fix:** Compute the list once in `run` and pass it to both `keeper` and the walk, for example with an `Options` variant or a `walk_roots(&[PathBuf], ...)` entry point.

### IN-06: The case-insensitive keep-dir test passes silently on case-sensitive volumes

**File:** `crates/twins-core/tests/pipeline_test.rs` (`in_dir_maps_the_keep_dir_past_a_nested_root_in_a_different_case`)
**Issue:** On a case-sensitive volume, the test hits `return` and reports `ok`, with no signal that it did not run. A CI runner on a case-sensitive APFS volume would never exercise the WR-01 case-mismatch path.
**Fix:** Print a skip notice (`eprintln!`), or create a case-insensitive disk image in CI. Alternatively, mark the test `#[ignore]` with a reason and run it explicitly.

### IN-07: A keep dir under a path the walk skips is accepted, and the scan quietly does not use it (carried forward, iteration-3 IN-11)

**File:** `crates/twins-core/src/pipeline.rs:309-341`, `crates/twins-core/src/scan/walk.rs:165-177`
**Issue:** Still valid. A keep dir under an `--exclude` glob, `node_modules`, `~/Library` or a remote mount passes every `keeper` check. Its files are never grouped, so a copy that the user expected to be removed survives as the oldest.
**Fix:** Check the mapped keep dir's components against `Rules::skip_dir` and the Library and remote checks, and fail with a usage variant. Otherwise, document the behaviour on `ScanSpec::keep_dir`.

### IN-08: A root that is a symlink nested in another root is silently never scanned (carried forward, iteration-3 IN-12)

**File:** `crates/twins-core/src/scan/walk.rs:278-283`
**Issue:** Still valid. `normalise_roots` drops a root that lexically `starts_with` another, but the walk does not follow links (`walk.rs:165`), so `twins scan ~ ~/Dropbox` with `~/Dropbox -> ~/Library/CloudStorage/Dropbox` never scans Dropbox. Since the WR-01 fix, an in-dir keep dir under such a root now fails loudly with `KeepDirOutsideRoots`. That message is confusing, because the user did list the folder as a root.
**Fix:** De-duplicate on canonical paths and keep the walk spelling of the surviving root. Alternatively, drop a nested root only when `canonicalize(nested).starts_with(canonicalize(outer))`.

### IN-09: Iteration-3 findings still open in reviewed files (carried forward)

**File:** `crates/twins-core/src/pipeline.rs:169`, `crates/twins-core/src/pipeline.rs:367-372`, `crates/twins-core/src/pipeline.rs:327-333`, `crates/twins-core/src/scan/walk.rs:230`, `crates/twins-cli/tests/cli_test.rs:376-385`
**Issue:** These are unchanged by this diff and still valid:

- **Not `#[non_exhaustive]`:** `PipelineError` still lacks it, and so does `observe::Outcome` (old IN-07).
- **Path printed twice:** `skipped` builds its `reason` from a `Display` that already contains the path (old IN-03).
- **Exit code 2 for an I/O error:** the keeper's `ScanError::Io { op: "resolve" }` maps to exit 2 although it is an I/O failure (old IN-05).
- **Progress freezes during stat:** `handle_file` emits no progress (old IN-04).
- **SIGINT tests never run in CI:** they are `#[ignore]` and use a fixed 700 ms sleep (old IN-06).

**Fix:** Apply the fixes recorded in iteration 3 for each item.

### IN-10: `Role` sorting puts hardlink rows between keep and remove, but the header count includes them as copies

**File:** `crates/twins-core/src/report.rs:177-184`
**Issue:** The header `[1] 1.0 MiB × 3` counts `g.files.len()`, which includes hardlinks of the kept file and aliases of the same file. The new layout labels those rows `keep`, so a reader sees "× 3" but only two physical copies and one removal. This predates the phase, but the new labels make the mismatch more visible.
**Fix:** Show physical copies in the header (`× 2 copies, 3 paths`), or keep the count and document it in the `write_text` doc.

---

_Reviewed: 2026-10-03T16:12:39Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

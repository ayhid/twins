---
phase: 01-core-pipeline-refactor
plan: 06
subsystem: core
tags: [rust, cancellation, blake3, hashing, rayon]

requires:
  - phase: 01-core-pipeline-refactor (plan 01-02)
    provides: per-stage done == 0 markers and in-order progress in group::find
provides:
  - hash::full_cancellable(&Path, &AtomicBool) and hash::equal_cancellable(&Path, &Path, &AtomicBool)
  - Defaulted Hasher::full_cancellable trait method, overridden by DirectHasher
  - group::find stops within one 256 KiB chunk of a cancel in the full hash and verify stages
  - A cancelled read is never a digest and never an on_error report
affects: [01-07 second Ctrl+C escape hatch, phase 4 hash cache (CACHE-03), twins-app cancel button]

actuals:
  tokens: 3758
  tasks: 2
  commits: 4

plan_head_before: 09f59c99a38d14858273a9aaa8b166edeff1ece8
plan_head_after: 640b31be971731113b60005975bfeed67f93ccf2

tech-stack:
  added: []
  patterns:
    - "Cancellable I/O loop: poll an &AtomicBool before every chunk and fail with op \"cancelled\" / ErrorKind::Interrupted, never return a partial result"
    - "Defaulted trait method for a new capability, so existing Hasher implementors compile unchanged"
    - "Re-check cancel after each unit of work in compute_keys, so an interrupted key becomes FindError::Cancelled and is never reported"

key-files:
  created: []
  modified:
    - crates/twins-core/src/hash.rs
    - crates/twins-core/src/group/hasher.rs
    - crates/twins-core/src/group/find.rs
    - crates/twins-core/tests/hash_test.rs
    - crates/twins-core/tests/group_test.rs

key-decisions:
  - "hash::full and hash::equal now delegate to the cancellable forms with a never-raised local AtomicBool, so there is one read loop per operation"
  - "The cancel check lives in a private hash::check_cancel helper, which builds HashError { op: \"cancelled\", source: Interrupted } with the file's path (path a for equal)"
  - "Options::cancel_flag() returns the configured flag or a module-level static NEVER, so the full and verify stages always have a flag to pass"
  - "In verify, an Err seen while the flag is raised returns the files kept so far without reporting; find_in_pool then returns Cancelled before progress or grouping"
  - "The RED commit for Task 1 carried flag-ignoring stubs of the two new functions, so RED failed on assertions instead of compilation (a compile error is INVALID_RED)"

patterns-established:
  - "No digest from an interrupted read: documented on hash::full_cancellable and Hasher::full_cancellable as the contract a Phase 4 cache relies on"

requirements-completed: [CORE-02]

coverage:
  - id: D1
    description: "hash::full_cancellable and hash::equal_cancellable return Err with op cancelled and Interrupted when the flag is raised, otherwise match full / equal"
    requirement: CORE-02
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/hash_test.rs#full_cancellable_stops_on_raised_flag"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/hash_test.rs#full_cancellable_matches_full"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/hash_test.rs#equal_cancellable_stops_on_raised_flag"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/hash_test.rs#equal_cancellable_matches_equal"
        status: pass
    human_judgment: false
  - id: D2
    description: "Hasher::full_cancellable is a defaulted trait method; DirectHasher polls the flag; SpyHasher compiles unchanged"
    requirement: CORE-02
    verification:
      - kind: unit
        ref: "cargo test -p twins-core --test group_test (SpyHasher unchanged, hashes_each_identity_once)"
        status: pass
    human_judgment: false
  - id: D3
    description: "find returns FindError::Cancelled and never reports the interrupted file when the cancel lands during a full hash"
    requirement: CORE-02
    verification:
      - kind: integration
        ref: "crates/twins-core/tests/group_test.rs#cancel_during_full_hash_is_not_reported_as_error"
        status: pass
    human_judgment: false
  - id: D4
    description: "The verify stage polls the flag per file and inside each comparison and find returns Cancelled"
    requirement: CORE-02
    verification:
      - kind: integration
        ref: "crates/twins-core/tests/group_test.rs#verify_stops_when_cancelled"
        status: pass
      - kind: integration
        ref: "cargo test -p twins-core --test pipeline_test cancel"
        status: pass
    human_judgment: false

duration: 4min (execution; context loading before that not counted)
completed: 2026-10-02
status: complete
---

# Phase 1 Plan 06: Cancel lands mid-file in hashing and verify Summary

**`hash::full_cancellable` and `hash::equal_cancellable` check the cancel flag before every 256 KiB chunk and fail with op `cancelled` / `Interrupted`, never a digest. `group::find` hashes the full stage with `Hasher::full_cancellable`, compares in verify with `equal_cancellable`, and turns an interrupted file into `FindError::Cancelled` instead of an `on_error` report.**

## Performance

- **Duration:** about 4 min of execution (start recorded at 2026-10-02T20:27:43Z, after the required reading)
- **Started:** 2026-10-02T20:27:43Z
- **Completed:** 2026-10-02T20:31:15Z
- **Tasks:** 2 of 2
- **Files modified:** 5

## Accomplishments

- `hash.rs`: new `full_cancellable` and `equal_cancellable`. Both poll the flag at the top of every read iteration through a private `check_cancel` helper, so a cancel lands within one 256 KiB chunk even in a multi-gigabyte file. A cancelled call can only return `Err`. `full` and `equal` keep their signatures and delegate with a never-raised flag.
- `hasher.rs`: `Hasher::full_cancellable(&self, m, cancel)` is a defaulted method. The default ignores the flag and calls `full`, so `SpyHasher` and any other implementor compiles unchanged. `DirectHasher` overrides it to call `hash::full_cancellable`. The doc states the no-prefix-digest guarantee a Phase 4 cache depends on (T-01-12).
- `find.rs`: `static NEVER` plus `Options::cancel_flag()`. The full stage calls `opts.hasher.full_cancellable(m, opts.cancel_flag())`. `compute_keys` returns `None` when the flag is up after a key is computed, before any progress call, so the interrupted file becomes `FindError::Cancelled` and is never reported (T-01-13). `verify` polls per file, uses `equal_cancellable`, and returns early without reporting on a cancelled compare. The bucket loop returns `Cancelled` right after `verify`. The `Options::cancel` doc now says the flag is polled inside each full hash and byte comparison.
- The 01-02 guarantees still hold: the `done == 0` markers are unchanged, and a completed stage still ends at `(total, total)`. The new early return only fires once the run is cancelled.

## Task Commits

1. **Task 1: Hashing and byte comparison stop within one chunk of a cancel**
   - RED `de4b940` test(01-06): add failing tests for cancellable full hash and compare (#1)
   - GREEN `53700ef` feat(01-06): let hashing stop mid-file on cancel (#1)
2. **Task 2: find stops on cancel mid-file and never reports the interrupted file as unreadable**
   - RED `c11d13a` test(01-06): add failing tests for cancel during full hash and verify (#1)
   - GREEN `640b31b` fix(01-06): stop hashing promptly and never report a cancelled read (#1)

No REFACTOR commits were needed.

## TDD Gate Compliance

| Gate | Task 1 | Task 2 |
|------|--------|--------|
| RED commit | `de4b940` | `c11d13a` |
| RED evidence (`check tdd-red-evidence`) | RED_EVIDENCE_OK, target `full_cancellable_stops_on_raised_flag` | RED_EVIDENCE_OK, target `cancel_during_full_hash_is_not_reported_as_error` |
| GREEN commit | `53700ef` (feat) | `640b31b` (fix, the message the plan prescribes) |

The checker reads TAP only. As in 01-02, each record came from a script that ran the real `cargo test` command and turned every `test NAME ... ok|FAILED` line into a TAP line, with the raw cargo output kept in the record. The records are in the session scratchpad, not in the repo. Both RED runs failed on the intended assertions:
- Task 1: `unwrap_err()` on `Ok(Digest(48e2…))` for `full_cancellable_stops_on_raised_flag`, and on `Ok(true)` for `equal_cancellable_stops_on_raised_flag`. The flag was ignored.
- Task 2: `on_error` received `[".../a"]` where `[]` was expected. The interrupted file was reported as unreadable.

`verify_stops_when_cancelled` already passed at RED, because the existing per-bucket poll catches a cancel raised at the Verify marker. The plan asks only for the first Task 2 test to fail, so this one is kept as a regression guard.

## Files Created/Modified

- `crates/twins-core/src/hash.rs`: `full_cancellable`, `equal_cancellable`, `check_cancel`; `full` and `equal` delegate
- `crates/twins-core/src/group/hasher.rs`: defaulted `Hasher::full_cancellable`, `DirectHasher` override
- `crates/twins-core/src/group/find.rs`: `NEVER`, `cancel_flag()`, cancellable full stage, cancel re-check in `compute_keys`, cancel-aware `verify`, check after `verify` in the bucket loop, updated `Options::cancel` doc
- `crates/twins-core/tests/hash_test.rs`: 4 tests plus a `mib_of` helper
- `crates/twins-core/tests/group_test.rs`: `CancellingHasher` and 2 tests

## Decisions Made

- `full` and `equal` reuse the cancellable loops with a local `AtomicBool::new(false)` instead of keeping two copies of each loop.
- In `equal_cancellable`, the cancel error carries path `a`, as the plan specifies.
- `cancel_during_full_hash_is_not_reported_as_error` uses `.workers(1)`, so the first file is always hashed first and the RED failure was deterministic. With more workers the old code sometimes returned `Cancelled` before reaching the reported file, which made RED flaky.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The Task 1 RED commit carried signature stubs**
- **Found during:** Task 1 RED
- **Issue:** The plan says to confirm the new tests "fail to compile". Under `TDD_MODE`, a compile error is INVALID_RED (`fixture_or_load_failure`) and does not authorize GREEN.
- **Fix:** The RED commit also added `full_cancellable` and `equal_cancellable` stubs that ignored the flag and delegated to `full` / `equal`. RED then failed on the target assertion (RED_EVIDENCE_OK). GREEN replaced both stubs.
- **Files modified:** `crates/twins-core/src/hash.rs`
- **Verification:** `check tdd-red-evidence` returned RED_EVIDENCE_OK. A grep for "RED stub" in the sources now finds nothing.
- **Committed in:** `de4b940` (RED), `53700ef` (GREEN)

**2. [Rule 1 - Bug] Clippy pedantic `semicolon_if_nothing_returned` in the new tests**
- **Found during:** Task 2 verification
- **Issue:** The two new `on_error` closures ended without `;`, which fails `clippy -D warnings`.
- **Fix:** Added the semicolons.
- **Files modified:** `crates/twins-core/tests/group_test.rs`
- **Verification:** `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- **Committed in:** `640b31b`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Neither changes behavior or scope. The first keeps the TDD gate valid. The second is lint hygiene.

## Issues Encountered

None.

## Verification

- `cargo fmt --all --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo test --workspace`: 14 suites, all `ok`, no `FAILED`
- `cargo test -p twins-core --test pipeline_test cancel`: 6 of 6 pass
- `group_test`, `hash_test` and `pipeline_test` ran 10 times back to back with no failure
- Acceptance greps: `static NEVER: AtomicBool`, `fn cancel_flag(&self) -> &AtomicBool`, `full_cancellable(m, opts.cancel_flag())` and `equal_cancellable(` are all present in find.rs, and `hash::equal(` does not appear in find.rs

## Threat Mitigations

- T-01-12 (prefix digest): `full_cancellable` checks before every chunk and returns `Err` on cancel, never a `Digest`. Covered by `full_cancellable_stops_on_raised_flag`, with the contract documented on `Hasher::full_cancellable`.
- T-01-13 (interrupted file reported as unreadable): `compute_keys` drops a key computed while the flag went up, and `verify` does not report a cancelled compare. Covered by `cancel_during_full_hash_is_not_reported_as_error`.

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness

- Plan 01-07 (second Ctrl+C escape hatch) can rely on the full hash and verify stages stopping within one chunk. A single `read()` blocked on a stalled volume is still not interruptible cooperatively, as the plan's Flagged Assumptions note.
- Partial hashing still polls only between files. It reads at most 32 KiB per file, as the plan assumes.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*

## Self-Check: PASSED

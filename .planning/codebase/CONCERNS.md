---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
# Codebase Concerns

**Analysis Date:** 2026-10-02

## Feature Gaps

### Deletion Not Yet Implemented

**Problem:** The CLI only provides `scan` and `report` commands; no deletion functionality exists.

**Files:** `crates/twins-cli/src/run.rs`, `crates/twins-core/src/group/keep.rs`

**Impact:**
- Users cannot execute removal decisions without external scripts
- The `--dry-run` flag and deletion logic are hardcoded (see `meta.dry_run = false` in `run.rs:63`)
- Blocks primary use case of freeing disk space

**Status:** Explicitly acknowledged in README — "Deletion... follow in later steps."

**What to build:** Implement `rm` command that:
- Takes a group ID or file path
- Calls `group::keep` strategy to resolve which file to keep
- Executes safe deletion (verify before removing, fail on permission errors)
- Returns exit code indicating success/failure per file

---

### No Persistent Cache

**Problem:** Duplicate detection re-hashes all files on every scan; no cache of size/hash pairs.

**Files:** `crates/twins-core/src/group/index.rs`, `crates/twins-cli/src/run.rs`

**Impact:**
- Repeated scans of large disks take the same time as first scan
- No incremental scanning (e.g., "find new duplicates since last scan")
- Performance degrades linearly with directory size on each run

**Status:** README notes "caching... follow in later steps."

**Scaling notes:**
- For a 2TB disk with average 1MB files, hashing alone can take 5-10 minutes
- Cache could reduce this to <1 minute for unchanged files
- Requires invalidation strategy (mtime tracking, inode verification)

---

### No Configuration File Support

**Problem:** All options must be passed via CLI flags; no `.twinsrc` or config file.

**Files:** `crates/twins-cli/src/cli.rs`

**Impact:**
- Users running repetitive scans must type the same flags repeatedly
- No easy way to set organization-wide defaults
- Exclude patterns cannot be persisted

**Status:** README mentions this as planned.

---

### No Desktop/UI Application

**Problem:** CLI-only interface; no graphical twin viewer or batch deletion UI.

**Files:** All of `crates/twins-cli`

**Impact:**
- Power users and macOS users familiar with GUI tools may avoid the CLI
- Batch approval workflows (select files to keep interactively) not possible

**Status:** README: "Tauri desktop app follow in later steps."

---

## Known Edge Cases

### Very Large Files (>2GB)

**Problem:** File size is stored as `u64`, but no explicit tests for files larger than 2GB.

**Files:** `crates/twins-core/src/human.rs` (line 4-5, recently fixed), `crates/twins-core/src/fsutil.rs`

**Impact:**
- Parsing a size like `5GB` with the old code could overflow (fixed in commit c962eec)
- Current code now rejects values ≥2^64, but edge case at 2GB-4GB range untested in practice

**Status:** Fixed by c962eec (2026-09-07) to reject oversized values; bounds-checking added.

**What's left:** Add integration test scanning a mock 5GB file to verify end-to-end behavior.

---

### Empty Groups After Filtering

**Problem:** When `verify` stage runs and all but one file are removed due to byte-wise differences, the group becomes empty.

**Files:** `crates/twins-core/src/group/find.rs` (line 213-222), `crates/twins-core/src/group/keep.rs`

**Impact:**
- Empty groups could crash `Keeper::pick()` if not guarded
- Report generation could produce spurious output

**Status:** Fixed by c962eec; `Keeper::pick()` now returns `Option<&FileMeta>` instead of panicking.

**Validation:** Tests in `crates/twins-core/tests/keep_test.rs` cover empty case.

---

### Permission Denied on Readable Directories

**Problem:** A directory may be readable (traversable) but contain files user cannot read.

**Files:** `crates/twins-core/src/scan/walk.rs` (line 192-212), `crates/twins-cli/src/run.rs` (line 89-96)

**Impact:**
- `stat()` failures are caught but silently skipped if `--verbose` flag omitted
- User may not realize files were missed, especially in `/Library` or system locations
- Number of errors reported only at end (line 72-74 of run.rs)

**Mitigation:** Full Disk Access requirement documented in README; errors counted and reported.

**Recommendation:** Consider adding `--fail-on-error` flag to exit non-zero if any files are unreadable.

---

### Hardlink vs. Symlink Confusion

**Problem:** Hardlinks share `(device, inode)` and are safe to treat as one physical file. Symlinks break this assumption.

**Files:** `crates/twins-core/src/fsutil.rs` (identity tracking), `crates/twins-core/src/scan/walk.rs` (line 124: `follow_links(false)`)

**Impact:**
- Symlinks are correctly skipped (never followed), but this is policy, not enforced at type level
- If symlink-following were accidentally enabled, results would be silently wrong

**Status:** Safeguard is working; tests in `crates/twins-core/tests/fsutil_test.rs` cover hardlinks and symlinks.

---

## Performance Bottlenecks

### Thread Pool Saturation with Many Small Files

**Problem:** The walk stage creates one task per file, dispatched to a thread pool (default: 1 worker per CPU, capped at 8).

**Files:** `crates/twins-core/src/scan/walk.rs` (line 91-93, pool setup), `crates/twins-core/src/group/find.rs` (line 18-19, MAX_WORKERS = 8)

**Impact:**
- With millions of small files (e.g., node_modules), pool may starve and become a bottleneck
- Disk I/O (stat calls) is the real bottleneck, not CPU, so more workers might help
- Current cap of 8 workers is conservative (prevents SSD saturation during hashing)

**Mitigation:** Walk stage uses one thread per CPU (no cap), hashing stage capped at 8. This is reasonable.

**Scaling consideration:** For >10 million files, consider:
- Chunking: batch stat() calls per pool task
- Adaptive pool sizing: more workers for walk, fewer for hash

---

### Memory Usage with Large Candidate Sets

**Problem:** All candidates are held in memory in `Index::by_size` (HashMap).

**Files:** `crates/twins-core/src/group/index.rs`

**Impact:**
- A 2TB disk with 1MB average file size = ~2 million FileMeta structs
- Each FileMeta includes a PathBuf (variable size, typically 100-300 bytes)
- Estimated memory: 200MB-600MB for a large disk

**Status:** Not a crisis for modern machines, but notable for embedded or resource-constrained environments.

**Recommendation:** Monitor memory usage in real-world deployments; consider streaming groups to disk if needed.

---

### Hashing All Duplicates Even When Only One Group Reported

**Problem:** The find pipeline hashes all candidates at every stage. If user asks for only the top 10 duplicates, all are still hashed.

**Files:** `crates/twins-core/src/group/find.rs` (pipeline stages), `crates/twins-cli/src/run.rs`

**Impact:**
- On a disk with 10,000 duplicate groups, hashing all takes much longer than reporting the top 100
- No streaming or early exit

**Status:** Not critical for typical use cases; acceptable for the current scope.

**Improvement path:** Add `--limit N` flag to report only top N groups by reclaimable space (post-sorting, not during pipeline).

---

## Security & Safety Concerns

### Protected Locations Hardcoded

**Problem:** Protected paths are checked via string prefixes, not filesystem ACLs.

**Files:** `crates/twins-core/src/safety.rs` (lines 6-31)

**Impact:**
- If a user creates a symlink `/Library → /some/other/path`, protection is bypassed
- Cross-platform users (dual boot) may forget macOS-specific rules apply
- Relative path handling: if current dir is `/System/...`, relative paths are absolutized and still protected (safe, but subtle)

**Status:** Well-designed safeguards; symlinks are explicitly not followed (line 124 of walk.rs).

**Recommendation:** Ensure Full Disk Access permission is granted (documented in README). Consider warning if running as root.

---

### Unsafe Code Usage

**Problem:** Two `unsafe` blocks in `fsutil.rs` for `libc::statfs` call.

**Files:** `crates/twins-core/src/fsutil.rs` (lines 167-186)

**Impact:**
- If `statfs` arguments are invalid or buffer is misaligned, UB possible
- Mitigation: buffer is MaybeUninit, properly initialized via statfs before read

**Status:** SAFETY comments are present and correct. No known issues.

**Review:** Acceptable use of unsafe for platform-specific functionality with no Rust std equivalent.

---

## Fragile Areas

### Group Find Pipeline: Cancellation Handling

**Problem:** Cancellation flag (`options.cancel`) is polled at multiple points, but not uniformly.

**Files:** `crates/twins-core/src/group/find.rs` (lines 210, 298), `crates/twins-core/src/scan/walk.rs` (lines 138-143)

**Impact:**
- If cancellation is requested mid-stage, some files may be partially hashed before check
- Walk stage checks every 2^10 entries; find stage checks per file
- No guarantee of fast cancellation

**Status:** Current behavior is acceptable; cancellation is advisory, not preemptive.

**Safe modification:** Increase cancellation check frequency in find stages (e.g., every 100 items instead of per item) if performance degradation observed.

---

### Error Accumulation in Verbose Mode

**Problem:** When `--verbose` is used, errors are written directly to stderr. High-error scenarios (e.g., huge unreadable directory) can flood output.

**Files:** `crates/twins-cli/src/run.rs` (line 89-96)

**Impact:**
- No log rotation or buffering
- Output may be hard to parse with thousands of skip lines
- No way to filter errors by type (permission, file not found, etc.)

**Recommendation:** Consider structured error output (JSON with `--json --verbose`), or a summary instead of per-file logs.

---

### Test Coverage: Integration Tests Lacking

**Problem:** Most tests are unit tests; CLI integration test is minimal.

**Files:** `crates/twins-cli/tests/cli_test.rs` (158 lines)

**Impact:**
- End-to-end behavior (e.g., real temp directories, permission edge cases) not well covered
- Changes to core could break CLI without test failure

**Recommendation:** Add tests for:
- Scanning with permission errors
- Very large files (mock)
- Deep directory hierarchies
- Mixed symlinks and hardlinks

---

## Missing Critical Features

### No Dry-Run Execution

**Problem:** `dry_run` field is always false in the report (run.rs:63).

**Files:** `crates/twins-core/src/report.rs`, `crates/twins-cli/src/run.rs` (line 63)

**Impact:**
- Users must manually verify the report before deletion (when deletion is implemented)
- No preview of what would be deleted

**Status:** Acknowledged; deletion is not yet implemented, so dry-run is moot.

---

### No Batch or Resume Capability

**Problem:** If a scan is interrupted (Ctrl+C), state is lost; no way to resume.

**Files:** `crates/twins-core/src/group/find.rs`, scan options

**Impact:**
- Restarting a scan of a 2TB disk restarts from scratch
- Long-running operations must complete without interruption

**Status:** Not high priority; typical scans take 5-10 minutes, acceptable range.

---

## Dependency Risks

### No Vendored Dependencies

**Problem:** All dependencies come from crates.io; no vendoring or lock mechanism for air-gapped environments.

**Files:** `Cargo.lock`, `Cargo.toml`

**Impact:**
- Offline/air-gapped users cannot build
- Dependency unavailability (maintainer removes crate) blocks builds

**Status:** Standard Rust practice; acceptable for CLI tools.

---

### Rayon Thread Pool Error Handling

**Problem:** Rayon's thread pool silently drops panics in worker threads if not carefully handled.

**Files:** `crates/twins-core/src/scan/walk.rs` (line 91-93), `crates/twins-core/src/group/find.rs` (line 192-195)

**Impact:**
- If a worker panics, the pool may deadlock or silently drop results
- Index::map() in group/index.rs recovers from poisoned locks, but higher-level panics are not guarded

**Status:** Current code avoids panics in worker threads (uses Result types), so risk is low.

**Validation:** No unsafe code in parallel sections; all I/O errors are handled as Result.

---

## Test Coverage Gaps

### Missing Regression Tests for Recent Fixes

**Problem:** c962eec fixed oversized size parsing and empty group handling, but no test for files near u64::MAX.

**Files:** `crates/twins-core/tests/human_test.rs`, `crates/twins-core/tests/keep_test.rs`

**Impact:**
- Future refactoring of size parsing could reintroduce bug
- Empty group edge case has test, but oversized-size boundary test is absent

**Recommendation:** Add test case:
- `parse_size("18446744073709551615B")` should succeed
- `parse_size("18446744073709551616B")` should fail (overflow)

---

### No Platform-Specific Tests

**Problem:** All tests run on any OS; macOS-specific paths (Full Disk Access, /Library, bundles) not exercised on CI.

**Files:** CI runs on macos-latest (.github/workflows/ci.yml), but tests don't mock Full Disk Access denial

**Impact:**
- Real-world Full Disk Access errors won't be caught until user reports them
- Bundle detection (photoslibrary, app, etc.) is untested on live bundle

**Status:** Acceptable risk; Full Disk Access is user responsibility (documented).

**Improvement:** Mock EACCES (Permission Denied) in tests to verify error reporting.

---

## Scaling Limits

### Directory Tree Depth Limit

**Problem:** No enforced depth limit; deeply nested directories (e.g., node_modules > 50 levels deep) could exhaust stack or slow traversal.

**Files:** `crates/twins-core/src/scan/walk.rs` (walkdir crate handles this, but not explicitly bounded)

**Impact:**
- Very large monorepos or synthetic deeply nested trees could cause issues
- No explicit error if depth > X

**Status:** walkdir crate should handle this gracefully; tested implicitly via node_modules traversal.

---

### Number of Duplicate Groups Limit

**Problem:** All groups held in memory; no pagination or streaming.

**Files:** `crates/twins-core/src/group/find.rs` (line 208: `Vec::with_capacity`)

**Impact:**
- A 2TB disk with 1MB files could have 2 million groups (unlikely, but theoretical)
- Each group needs ~1KB memory (files vector + metadata)
- Estimated: 2GB peak memory for worst case

**Status:** Acceptable for typical disks (< 100K groups expected).

---

## Recommendations for Future Phases

### Before MVP (Deletion)

1. Add `rm` command with confirmation prompt
2. Test deletion on temp directories
3. Add `--dry-run` flag (print what would be deleted, don't execute)

### Before 1.0 Release

1. Add persistent cache (size + partial hash indexed by mtime/inode)
2. Add config file support (~/.twinsrc)
3. Add incremental scan mode
4. Improve error reporting (structured, filterable)

### Performance/Scaling (Post-1.0)

1. Streaming group generation (process groups as found, not all at once)
2. Adaptive parallelism (detect I/O saturation)
3. Benchmark on 5TB+ disks

---

*Concerns audit: 2026-10-02*

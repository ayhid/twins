# Pitfalls Research

**Domain:** macOS duplicate-file finder with destructive actions (trash / permanent / APFS clone), a persistent hash cache, a Tauri v2 desktop app, unattended workflows (schedule + FSEvents) and signed/notarized distribution via Homebrew
**Researched:** 2026-10-02
**Confidence:** MEDIUM overall. Facts checked directly in the twins source or in third-party source code (trash-rs) are HIGH. Apple man pages and Apple DTS forum answers are MEDIUM. Single-source web claims are LOW and labelled as such. The research seam classifies every web-fetched source as LOW unless it is corroborated, so platform-behaviour claims below that matter for safety are marked "verify in a spike".

> Core value reminder: **never lose data**. The pitfalls are ordered by how directly they threaten that.

---

## Critical Pitfalls

### Pitfall 1: Acting on a stale scan (TOCTOU between hash and delete)

**What goes wrong:**
The plan says "delete B, keep A" because A and B hashed equal at scan time. By the time the delete runs (seconds later in the CLI, minutes later in the app, hours later for a report someone reviews tomorrow), one of these may have happened:
- A (the keeper) was edited, moved, trashed by the user, or deleted by a sync client.
- B was edited, so it is no longer a duplicate.
- B's path now points at a different file. It may have been renamed over, or replaced by a symlink, or one of its parent directories was swapped for a symlink.

In each case the delete removes the only copy of some content.

**Why it happens:**
The engine is a pipeline (walk → group → hash → report), and deletion gets bolted onto the end, reusing `FileMeta` from the scan as if it were still true. `trash-rs` also canonicalises the *parent* directory before trashing (`canonicalize_paths` in `src/lib.rs`, HIGH, read from source). A parent that has been swapped for a symlink therefore redirects the trash to wherever the symlink points.

**How to avoid:**
- In the executor, before each victim:
  1. `lstat` the keeper and the victim. Require that both still exist, that both are regular files (not symlinks), and that `(volume, inode, size, mtime_ns, ctime_ns)` matches the plan.
  2. Re-verify content: run a fresh full BLAKE3 of keeper and victim that **bypasses the cache**, or a byte compare (open both, compare, keep the fds). This costs one read per pair, which is small next to the cost of losing data.
  3. Perform the action against the identity you just verified. Where possible, open the parent dir with `O_NOFOLLOW` and act relative to it, or re-`lstat` right after and compare inode.
- If any check fails, skip the **whole group**, journal `skipped: changed_since_scan`, and continue. Never "pick another keeper" on the fly.
- Plans carry a scan timestamp. The app and the workflow runner refuse plans older than N minutes without a re-scan.

**Warning signs:**
Executor functions that take `&[PathBuf]` instead of a verified plan. A `delete(path)` helper with no identity argument. Tests that never mutate a file between plan and execute.

**Phase to address:** Safe deletion (Step 2). It must be in the first deletion PR, not added later.

---

### Pitfall 2: Enforcing keep-one by path instead of by file identity

**What goes wrong:**
"At least one copy survives" gets checked as "the keeper path is not in the delete list". But several paths can name the **same file**:
- Hardlinks (same inode, `nlink > 1`). Unlinking one name is harmless but frees nothing.
- Overlapping roots reached through a symlinked root, e.g. `~/docs -> ~/Documents` plus `~/Documents`. `normalise_roots` only drops roots that are *lexically* nested (HIGH, `walk.rs:226`), and `walkdir` follows root symlinks by default (`follow_root_links: true`, HIGH, walkdir 2.5 source).
- Case or Unicode-normalisation aliases on case-insensitive APFS (`/Users/a/Foo.jpg` vs `/users/a/foo.jpg`).
- Firmlinks (`/Users/...` vs `/System/Volumes/Data/Users/...`). The `/System` prefix rule happens to block the second form today.

If the hardlink-dedup stage ever regresses, or a future feature (e.g. the app's manual selection) builds a group from paths, the "duplicate" and the "keeper" can be the same inode. The plan then deletes the only copy.

**Why it happens:**
Paths are the natural UI and JSON currency, and a hardlink looks like a duplicate.

**How to avoid:**
- Keep-one validation (`validate_plan`) runs on **identities** `(volume_uuid, inode)`:
  - the keeper's identity is not in the victim set;
  - every victim identity differs from the keeper's;
  - each victim identity appears at most once in the plan;
  - the group has at least one identity that is not a victim.
- Never offer a victim whose identity equals the keeper's, even under a different path.
- Report `nlink > 1` victims as "frees 0 bytes" (the inode survives through another name outside scope).
- Property-test the validator: random groups with injected aliases must never yield a plan that removes every identity.

**Warning signs:**
`HashSet<PathBuf>` in the plan validator. Reclaimable-space totals that are larger than `du` reports. A group whose members share an inode.

**Phase to address:** Safe deletion (Step 2), the plan validator. The app (Step 4) must call the same validator: the "last copy can never be marked" rule is UI sugar on top of it, not a replacement.

---

### Pitfall 3: The protection checks are lexical and can be bypassed (existing code)

**What goes wrong:**
`safety::is_protected` compares absolutised but **non-canonicalised** paths with `Path::starts_with` (HIGH, `safety.rs`). On a case-insensitive boot volume:
- `/library/...`, `/APPLICATIONS/...` and `/usr/LOCAL/...` all pass the check.
- `"/Volumes/Macintosh HD"` is a symlink to `/`, so `"/Volumes/Macintosh HD/Library"` passes too.
- `validate_root` uses `std::fs::metadata` (which follows symlinks), and walkdir follows root links. A symlinked root like `~/lib -> /Library` therefore gets walked.

That is harmless while scanning is read-only. Once `twins clean` exists it means deleting inside "never modified" locations.

**Why it happens:**
String-prefix rules are easy to write and test. Case-insensitivity and the `/Volumes/<boot>` alias are macOS-specific surprises.

**How to avoid:**
- Canonicalise every root (`realpath`, plus `fcntl(F_GETPATH)` on an open fd to get the on-disk casing) **before** the protection check, and re-check at execution time on the victim's canonical parent.
- Better: at startup, `stat` each protected root and record its `(dev, inode)`. During the walk and in the executor, refuse any directory whose identity matches a protected root. This is immune to case, symlinks and firmlinks.
- Set `WalkDir::follow_root_links(false)` or resolve roots explicitly, and dedupe roots by identity, not by prefix.
- Add regression tests for `/library`, `/Volumes/<boot name>/Library` and a symlinked root.

**Warning signs:**
Any safety test that only uses canonical-case literal paths (the current `safety_test.rs` does).

**Phase to address:** Safe deletion (Step 2), as a prerequisite task before the executor lands.

---

### Pitfall 4: Assuming `NSFileManager.trashItemAtURL` gives Finder "Put Back"

**What goes wrong:**
PROJECT.md specifies "trash via NSFileManager (Finder Put Back works)". It does not reliably. Programmatic trashing (both `trashItemAtURL` and `NSWorkspace recycle`) has a long-standing radar (r. 23153124): only the first item trashed in a process gets Put Back. Apple DTS (Feb 2025, macOS 15.2) found it reproduces when Finder's Trash window is open and suspects Finder and FileManager race on the Trash's `.DS_Store`. There is no workaround short of a 2-second delay between calls (MEDIUM: Apple forum thread plus sindresorhus/macos-trash#4 plus the trash-rs docs).

`trash-rs` documents the same trade-off (HIGH, read from source, crate v5.2.9):
- Its **default** `DeleteMethod::Finder` shells out to `osascript` to drive Finder. Put Back works, but it plays the Trash sound, is slower, and needs Automation (Apple Events) permission.
- `NsFileManager` is fast and silent, but Put Back is unreliable.
- `trash-rs` passes `None` for `resultingItemURL`, so you never learn where the file landed in the Trash.

**Why it happens:**
Developers test with one file and see Put Back working.

**How to avoid:**
- **Decide explicitly** and update the requirement: "Trash is the default. Restore is guaranteed by `twins undo` from the journal. Finder Put Back is best-effort."
- Call `trashItemAtURL:resultingItemURL:error:` **directly via `objc2-foundation`**, not through `trash-rs`, and write the resulting URL into the journal. That gives a deterministic `twins undo <op-id>` that moves the file back (re-verifying the original path is free and the content hash matches).
- Do **not** use the Finder/AppleScript path from a hardened-runtime app or a background agent:
  - it needs the `com.apple.security.automation.apple-events` entitlement, `NSAppleEventsUsageDescription`, and a user-approved Automation prompt that a headless agent cannot show;
  - the trash is then performed by Finder, so FSEvents `IgnoreSelf` will not filter it (see Pitfall 11);
  - Finder may show its own dialogs.
- If Put Back matters to the user, offer an opt-in "Finder-compatible trash" for interactive app use only, and test it with the Trash window open.

**Warning signs:**
A UAT step that says "verify Put Back" tested with a single file. Using `trash::delete_all` with default context.

**Phase to address:** Safe deletion (Step 2). Make this a roadmap decision point. The journal-based undo belongs in Step 2, not later.

---

### Pitfall 5: Trash semantics differ per volume, and "trash failed" turns into "permanent delete"

**What goes wrong:**
- Internal APFS: `~/.Trash`.
- External HFS+/APFS: `/Volumes/X/.Trashes/<uid>/`. The file stays on that disk and vanishes from the Trash UI when the disk is ejected. Space is not freed until the trash is emptied *with the disk mounted*.
- exFAT/FAT, SMB/NFS/AFP and some NAS mounts: **no Trash**. Finder asks "delete immediately?", and `trashItemAtURL` returns an error.
- iCloud Drive items go to iCloud's own trash, and the deletion propagates to every device.

The classic bug is a `match trash(p) { Err(_) => fs::remove_file(p) }` fallback, or a UI that offers "Delete immediately?" in the middle of a batch. Either one turns a trash run into permanent deletion.

**Why it happens:**
Developers test on the boot volume only. `--include-remote` exists, so remote groups will reach the executor.

**How to avoid:**
- The executor **never** falls back from trash to unlink. A trash failure is journaled `failed: trash_unavailable`, and the group is reported.
- Pre-flight each victim's volume (`statfs` fs type plus a probe for `.Trashes` writability) and mark groups "trash unavailable" in the plan **before** confirmation.
- Permanent mode for those volumes is interactive-only and requires typed confirmation. Workflows never get it (already Out of Scope; enforce it in types: the workflow runner's action enum has no `Permanent` variant).
- Report "space freed after emptying Trash", not "space freed".

**Warning signs:**
Any `remove_file` call reachable from the trash code path. Tests that only run on `$TMPDIR`. (`$TMPDIR` is under `/private/var/folders`, so the trash behaves like the boot volume; also check that is_protected's allow-list does not mask it.)

**Phase to address:** Safe deletion (Step 2). Workflows (auto-clean) inherit it.

---

### Pitfall 6: Getting APFS clone replacement wrong (metadata, atomicity, no-op savings)

**What goes wrong:**
"Replace duplicate B with a clone of A" sounds simple, but the clonefile(2) man page (MEDIUM, man page) says:
- The clone "has its own copy of attributes and extended attributes which are identical to those of … **src**". The clone gets **A's** metadata: A's mtime/birthtime, Finder tags and comments, quarantine and `kMDItemWhereFroms` xattrs. B's own metadata is lost silently.
- The setuid/setgid bits are cleared. ACLs are only copied with `CLONE_ACL`.
- `dst` must not exist, so you cannot clone *onto* B. A naive `unlink(B); clonefile(A, B)` leaves a window where B is gone. A crash in that window loses B's name and metadata, and loses the data too if A changed in between.
- Cross-volume pairs fail with `EXDEV`, and non-APFS volumes fail with `ENOTSUP`.
- If B has hardlinks (`nlink > 1`), replacing one name frees nothing and silently splits the hardlink set.
- If A and B are **already clones** of each other (common after Finder duplicates or `cp -c`), replacing B frees 0 bytes, and the "reclaimable" figure was a lie. The same applies when an APFS local snapshot (Time Machine) still holds B's blocks.

**Why it happens:**
clonefile reads like "cp but free", and developers do not read the attribute semantics.

**How to avoid:**
Use this procedure:
1. `clonefile(A, dir(B)/.twins-tmp-<uuid>, CLONE_NOFOLLOW)`.
2. Copy **B's** metadata onto the temp file with `copyfile(B, tmp, COPYFILE_METADATA)`, or `fcopyfile` with `COPYFILE_SECURITY | COPYFILE_XATTR | COPYFILE_STAT`. Restore B's times.
3. Re-verify the content equality of the temp file and B.
4. `renamex_np(tmp, B, RENAME_SWAP)`. The swap is atomic, and the original B now lives at the temp name.
5. Journal the swap.
6. Trash (not unlink) the swapped-out original.
7. On any failure, remove the temp file. B is untouched.

Also:
- Skip victims with `nlink > 1` or on a different volume.
- Read `ATTR_CMNEXT_PRIVATESIZE` / `ATTR_CMNEXT_CLONEID` (`getattrlist`, macOS 10.15+) to show the true savings. Mark already-cloned pairs "0 B" and skip them.

**Warning signs:**
A `clonefile` call with B's path as `dst`. Tests that only compare file contents, not xattrs or times. Reclaimable-space totals that exceed `df` deltas after a run.

**Phase to address:** Safe deletion (Step 2). Clone mode could reasonably be split into its own sub-phase after trash/permanent ship. Needs a spike to confirm `copyfile` flags and `RENAME_SWAP` behaviour on APFS.

---

### Pitfall 7: Treating the hash cache as evidence for deletion (cache poisoning leads to data loss)

**What goes wrong:**
The cache key `(dev, inode, size, mtime)` is stale in more cases than people expect:
- **`st_dev` is assigned at mount time.** The same volume gets a different `st_dev` after a reboot or re-mount, and two volumes can swap numbers (MEDIUM: Apple forum plus other projects' fixes). Results: cache misses at best, and at worst hits for a *different* volume's inode.
- **Inode reuse:** APFS inode numbers are effectively monotonic, but exFAT/FAT/SMB synthesise inode numbers (often from directory position), so they change on rename and get reused.
- **mtime granularity:** APFS has nanoseconds, HFS+ has 1 s, FAT has 2 s. A same-size edit within the same tick keeps `(size, mtime)` identical. This is git's "racy clean" problem (MEDIUM: git racy-git docs).
- **mtime-preserving tools:** `rsync -t`, `cp -p`, `tar x`, `touch -r` and some sync clients restore mtime after changing content. Only **ctime** (not user-settable) catches these.

If the cache then says two different files have equal BLAKE3, and deletion trusts it, data is lost.

**Why it happens:**
The Go v0 warm-cache benchmark (0.15 s) creates pressure to make the cache authoritative.

**How to avoid:**
- Key: `(volume_uuid, inode, size, mtime_ns, ctime_ns)`. Get `volume_uuid` via `getattrlist(ATTR_VOL_UUID)` on the mount point, and keep `st_dev` only as a fallback for volumes without a UUID.
- Only cache on filesystems with stable inodes (APFS, HFS+). Never cache on exFAT/FAT/network volumes.
- Do not cache "racy" entries: files whose mtime or ctime is within ~2 s of the hash time (git's rule).
- **The cache accelerates discovery only.** The executor re-verifies every keeper/victim pair without the cache (Pitfall 1). This turns cache bugs from "data loss" into "wasted rescan".
- Version the cache schema and include the hash algorithm in it. On any decode error, discard the whole cache. Ignore Go v0's cache file rather than migrating it.

**Warning signs:**
`CacheKey { dev: u64, … }`. The executor calling `hash_cached()`. No test that rewrites a file with `touch -r` restoring mtime.

**Phase to address:** Cache & config (Step 3). The "executor never trusts the cache" rule must already exist from Step 2, so Step 3 cannot weaken it.

---

### Pitfall 8: Deleting inside sync folders or app-managed libraries

**What goes wrong:**
- **iCloud Drive "Desktop & Documents"** turns plain `~/Desktop` and `~/Documents` into synced folders. Trashing there trashes on every device.
- **Dropbox, Google Drive and OneDrive shared folders**: deleting a "duplicate" removes it for collaborators.
- **Keeper removed remotely:** if the keeper lives in a sync folder, a deletion on another device (or by a collaborator) after twins trashed the local victim leaves zero copies outside the Trash.
- **Referenced media:** Photos libraries in "referenced files" mode, Lightroom Classic catalogs, Music/TV media folders, Final Cut and Logic projects, and DAW sample folders all reference files *outside* their bundle by path. Deleting a duplicate breaks the catalog (photos show as "missing"). Lightroom's own community advice is "only use a plugin duplicate finder" (LOW: Adobe forums, but well-known).
- **Developer trees:** duplicates across git working trees and `vendor/` directories are intentional.

**Why it happens:**
"Bundles are never opened" protects the `.photoslibrary` *inside*, but not the files it references outside.

**How to avoid:**
- Detect sync roots and label every group member with a `location_kind`. Check `~/Library/CloudStorage/*` (File Provider), `~/Library/Mobile Documents`, the Desktop/Documents iCloud state, and a legacy `~/Dropbox` containing `.dropbox`.
- Auto-clean **never** touches synced or labelled locations by default. Interactive mode shows a warning badge.
- When choosing a keeper, prefer a local non-synced copy when the victims are synced (or at least never keep only a synced copy when auto-cleaning).
- Ship default excludes or warnings for known catalog-referenced dirs (`~/Pictures`, `~/Music/Music/Media`) when an app library exists. At minimum, add a prominent "referenced by Photos/Lightroom?" caveat.
- Exclude git working trees from auto-clean scopes (detect `.git` ancestors).

**Warning signs:**
UAT only on `~/Downloads`. No `location_kind` in the report schema.

**Phase to address:** Safe deletion (Step 2) for labelling and warnings. Workflows (auto-clean defaults) enforces it.

---

### Pitfall 9: "Identical content" that is not identical (resource forks, xattrs, empty data forks)

**What goes wrong:**
BLAKE3 over the data fork treats these as duplicates:
- Classic Mac files and font suitcases whose payload is in the **resource fork** (`com.apple.ResourceFork` xattr). Their data fork may be empty or identical.
- Files that differ only in Finder tags, comments or `kMDItemWhereFroms`, which the user may care about.

Deleting one loses the fork or metadata permanently.

**How to avoid:**
- Include the resource fork in the content identity: hash `data || rsrc`, or exclude files with a resource fork from deletion.
- Surface metadata differences (tags) in the group view. Keep `--include-empty` off by default (it already is).

**Phase to address:** Safe deletion (Step 2).

---

### Pitfall 10: Full Disk Access / TCC attaches to a different process for the CLI, the app and the background agent

**What goes wrong:**
TCC grants belong to the **responsible process**:
- `twins` run from Terminal or iTerm inherits *the terminal's* FDA.
- The Tauri app needs its own grant.
- A plain `launchd` plist pointing at the CLI binary is responsible for itself. A bare CLI tool cannot easily be granted FDA, and daemons historically could not get it at all (MEDIUM: Apple DTS "Rules for Full Disk Access", forum thread 661178).

Users see "works in Terminal, finds nothing in the app, finds even less on schedule". Without FDA, a background scan silently skips `~/Desktop`/`~/Documents`/`~/Downloads` (each has its own TCC prompt, which a headless process cannot show) and reports "no duplicates".

Further traps:
- There is **no API to request FDA**. You can only detect it and deep-link to System Settings.
- Grants are tied to the code signature. Ad-hoc dev builds and identity changes reset them.
- A debugger-attached process is denied FDA.

**Why it happens:**
Developers have FDA on their terminal, so everything works on their machine.

**How to avoid:**
- Ship the background runner as a **helper app bundle inside the main app**. Give it a bundle ID that is a child of the app's (`io.opkod.twins.agent`) and the same Team ID, and register it with **`SMAppService.agent(plistName:)`** (macOS 13+). Embedded helpers launched by Service Management inherit the main app's FDA (MEDIUM: DTS guidance; **verify in a spike**: sign, grant FDA to the app only, check the agent can read `~/Library/Safari/Bookmarks.plist`).
- The agent also needs a bundle so it can post `UNUserNotificationCenter` notifications. A bare CLI cannot post them properly.
- Detect FDA at startup in the app, the agent and the CLI with the canonical probe (read a TCC-protected file such as `~/Library/Safari/Bookmarks.plist` and check for EPERM). Show a blocking explainer with an `x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles` link.
- Workflow runs record `fda: false` and `skipped_permission: N` in the journal and the notification. A run that skipped protected dirs must never be reported as "clean".
- Use one stable Developer ID identity for dev builds too, so grants survive rebuilds.

**Warning signs:**
The scheduler implemented as `launchctl load ~/Library/LaunchAgents/*.plist` pointing at `/opt/homebrew/bin/twins`. Error counts that differ between CLI and app runs of the same scope.

**Phase to address:** Workflows (background agent), but **spike it before the app phase**, because it dictates the bundle layout (helper app, CLI placement, bundle IDs), which the Tauri bundler and the cask must produce.

---

### Pitfall 11: FSEvents feedback loops, storms and acting on half-written files

**What goes wrong:**
- **Self-trigger:** auto-clean trashes a file in the watched folder, which emits an event, which triggers the workflow again. The re-run is usually a no-op, but it re-scans, re-notifies, and on large scopes thrashes the disk. Writes to the cache DB or journal re-trigger the agent too if the scope includes `~/Library`.
- **`kFSEventStreamCreateFlagIgnoreSelf` does not save you** if the trash is performed by another process (Finder via AppleScript, or a separate executor process).
- **Storms:** `npm install`, Xcode builds, Photos imports and git checkouts produce thousands of events per second. FSEvents coalesces them into `MustScanSubDirs` with `UserDropped`/`KernelDropped`, which require a full rescan. Watchman has an open bug where a root got stuck in an endless rescan loop (MEDIUM: watchman#1354 plus Apple header semantics).
- **Half-written files:** a browser download or Finder copy in progress, or an intentional "Duplicate" the user just made, gets seen mid-write or immediately. Auto-clean then trashes the user's fresh deliberate copy (with `oldest` the new file is the victim).
- **History replay:** restarting with a saved `sinceWhen` can deliver a huge burst. Event-ID wrap and `RootChanged` events mean "rescan everything".
- The Rust `notify` crate maps the dropped/coalesced flags to a `Rescan` flag that is easy to ignore.

**How to avoid:**
- Debounce per workflow with a **settle window**: wait until the watched tree has been quiet for N seconds (default 30-60 s).
- Ignore files modified within the last M seconds (not yet settled) and files with known partial-download extensions (`.crdownload`, `.download`, `.part`).
- Filter out events under `.Trash`/`.Trashes`, the app's own data dirs, and paths the current run just acted on.
- Single-flight per workflow: at most one run in progress and at most one queued.
- Rate-limit runs (e.g. at most one per 5 min) plus exponential backoff when consecutive runs find nothing.
- Treat `MustScanSubDirs`, dropped events and `Rescan` as "schedule a full, debounced scan", never as "process incrementally".
- Prefer a folder trigger that means "scan this scope soon", not "act on these paths".

**Warning signs:**
The agent's CPU stays high after a `git clone` into a watched dir. The journal shows back-to-back runs seconds apart. Users report "my copy disappeared".

**Phase to address:** Workflows (FSEvents trigger).

---

### Pitfall 12: Unattended auto-clean with an unbounded blast radius

**What goes wrong:**
A workflow's scope gets widened (`~/Downloads` becomes `~`), an exclude gets deleted, a keep strategy gets switched (`oldest` to `newest`), or the config file gets hand-edited with a typo. Hours later a scheduled run trashes 40 000 files with nobody watching. Each step was "safe" (Trash, journal), but recovery from 40 000 trashed files across volumes is miserable, and if the Trash is auto-emptied (Finder "Remove items after 30 days") it eventually becomes permanent.

**How to avoid:**
- **Circuit breakers** per unattended run: maximum files, maximum bytes, and a maximum share of the scope (defaults like 200 files / 5 GB / 10%). Exceeding a breaker turns the run into report-only and sends a notification.
- **Report-only first run:** when a workflow is created or its scope, keep strategy or outcome changes, the next run is report-only, and auto-clean arms only after the user confirms the diff.
- Every unattended run gets a run ID, and the notification offers "Restore this run" (journal undo).
- Auto-clean only trashes victims **inside** the workflow's scope. Keepers must also be inside scope and on a local, non-synced volume.
- Warn when the user enables Finder's "empty Trash after 30 days" while auto-clean is on.

**Phase to address:** Workflows. Journal undo must already exist from Step 2.

---

### Pitfall 13: Concurrent mutators (app, CLI and agent at the same time)

**What goes wrong:**
The user clicks "Clean" in the app while a scheduled workflow runs on an overlapping scope, or runs `twins clean --yes` in a terminal. Two executors with plans from different scans can each keep a different file of the same group and trash the other. Result: zero copies outside the Trash. The cache DB also sees concurrent writers.

**How to avoid:**
- A global **execution lock** (`flock` on `~/Library/Application Support/twins/exec.lock`) held for plan validation plus execution. Scans can run concurrently, executions cannot.
- Pitfall 1's per-pair re-verification is the second line of defence: if the other executor already trashed the keeper, the group is skipped.
- Cache: SQLite in WAL mode with `busy_timeout`, or write-temp-then-`rename` for a blob cache. Never an in-place overwrite.

**Phase to address:** Safe deletion (lock), Cache (DB concurrency), Workflows (agent honours the lock).

---

### Pitfall 14: A journal that cannot actually undo anything

**What goes wrong:**
The journal is written *after* the operation (a crash loses the record), it is not fsynced, it records the original path but not where the file went in the Trash, it records dry-runs indistinguishably from real runs, or it fails to serialise paths. `serde`'s `Path` serialisation **errors** on non-UTF-8 paths, which are legal on macOS and appear in old archives. A journal write error mid-batch then either aborts confusingly or gets swallowed.

**How to avoid:**
- Write-ahead: append `intent` (op_id, run_id, group_id, keeper and victim identity, hash, mode) and fsync before acting, then append `done` or `failed` with `trashed_to` (the resulting URL). On startup, reconcile `intent` entries that lack an outcome.
- Encode paths losslessly: store `path_b64` (raw `OsStr` bytes) alongside a lossy display string.
- `dry_run: true` on every dry-run record. Undo refuses dry-run records.
- If the journal cannot be written, the operation does not happen.
- Rotate the log by size and never truncate it.
- Fix the `meta.dry_run = false` hardcode in `run.rs:63` (CONCERNS.md) as part of this work.

**Phase to address:** Safe deletion (Step 2).

---

### Pitfall 15: Dry-run that takes a different code path

**What goes wrong:**
`--dry-run` gets implemented as an early `if dry_run { print; return }` before validation, or the app's preview builds its own plan. The dry-run then says "keeps A" while the real run keeps B: a different sort, a different keep-strategy fallback, or the cache vs fresh-hash difference.

**How to avoid:**
One `plan()` → `validate()` → `execute(plan, Executor)` path, where `Executor` is `Real` or `DryRun`. Dry-run runs every validation, including the pre-flight checks (trash availability, lock acquisition, identity re-stat), and journals with `dry_run: true`. Add a test asserting that the dry-run plan equals the executed plan for the same input.

**Phase to address:** Safe deletion (Step 2).

---

### Pitfall 16: Tauri IPC that lets the webview delete arbitrary paths

**What goes wrong:**
Exposing `#[tauri::command] fn trash(paths: Vec<String>)` means any webview compromise (a malicious filename rendered with `{@html}`, a dependency compromise) can trash or delete anything the app can reach. With FDA, that is everything. Other Tauri v2 pitfalls:
- an over-broad `asset:` protocol scope (`**`) for thumbnails;
- `shell` or `fs` plugins enabled "for convenience";
- blocking `async` commands on the main thread during a scan (UI freeze);
- emitting a progress event per file, so that 100k events/s choke the webview;
- serialising a 200k-group report into one IPC payload.

**How to avoid:**
- The webview only ever sends **plan IDs and selection deltas** (`group_id`, member index). The Rust side holds the plan, re-validates it, and executes through the same core routine as the CLI.
- No `fs` or `shell` plugin. Minimal v2 capabilities per window.
- Scope the asset protocol to the current scan's files, or generate thumbnails Rust-side.
- Render filenames as text only, never with `{@html}`.
- Run scans in `spawn_blocking` or a dedicated thread and stream progress through a Tauri v2 `Channel` throttled to about 10 Hz.
- Paginate or virtualise groups (send pages, not the whole report).

**Phase to address:** Desktop app (Step 4).

---

### Pitfall 17: Signing and notarization of a multi-binary Tauri bundle

**What goes wrong:**
The bundle will contain the main app, the `twins` CLI and the agent helper app:
- Nested code must be signed inside-out with the hardened runtime and a secure timestamp, or notarization rejects it. Tauri `externalBin` sidecars have had notarization failures ("The signature of the binary is invalid", status 4000; tauri#11992, open since Dec 2024, MEDIUM).
- Developers add `com.apple.security.cs.allow-jit` / `allow-unsigned-executable-memory` "because Tauri" (copied from blog posts, LOW) and weaken the hardened runtime for no reason, since WKWebView JITs in Apple's own WebContent process.
- Developers turn on **App Sandbox** thinking notarization needs it. It does not. Sandbox would break arbitrary-path scanning, FDA-style access and the background agent.
- A DMG gets notarized but not stapled, so first launch offline fails Gatekeeper.
- Version strings: macOS expects `CFBundleShortVersionString` as `x.y.z`. A semver prerelease `1.3.0-beta.2` needs an explicit numeric `bundle.macOS.bundleVersion` (Tauri supports setting it; MEDIUM).

**How to avoid:**
- Developer ID plus hardened runtime, **no sandbox**, entitlements file empty by default. Add an entitlement only with a written reason.
- Prefer building the CLI and the helper as part of the bundle via a post-build signing script you control (inside-out: helper app, then CLI, then main app) over `externalBin`, if the sidecar path keeps failing.
- After signing, verify every nested binary with `codesign --verify --deep --strict` and `spctl -a -vv`.
- Notarize with an App Store Connect API key (`APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH`), then staple both the `.app` and the `.dmg`.
- Add a CI smoke test: download the artefact, set `com.apple.quarantine`, and launch.

**Phase to address:** Release (Step 5). The bundle layout decision itself comes from the Pitfall 10 spike.

---

### Pitfall 18: semantic-release with `main` as prerelease and `stable` as release

**What goes wrong:**
- **Version ordering:** semantic-release requires that "versions released on a given branch must always be higher than the last release made on the previous branch" (MEDIUM: official docs). If `stable` is promoted by a **merge commit**, the stable tag (`v1.3.0`) lands on a commit that is not in `main`'s history. `main` keeps producing `1.3.0-beta.N`, so the beta channel is now *older* than stable, and Homebrew `@beta` users "upgrade" to a lower version.
- **Version propagation:** semantic-release is a Node tool. It does not update `Cargo.toml` (`workspace.package.version` is `1.0.0-beta.0` today), `Cargo.lock`, `tauri.conf.json` or the Svelte `package.json`. The built binary then reports the wrong `twins version`, the notarized bundle has the wrong `CFBundleVersion`, and the report JSON `meta.version` lies.
- **Build placement:** building or notarizing *after* semantic-release has tagged means a notarization failure leaves a published tag and GitHub release with no artefacts. The reverse (building before) means the version is not known yet.
- **Commit hygiene:** squash-merge titles that are not Conventional Commits produce no release, silently.
- **Version reset:** there are no tags in the repo yet, so the first `main` release will be `1.0.0-beta.1`, which matches the current manifest, but only if nobody pushes a stray `v0.x` Go-era tag.

**How to avoid:**
- `branches: [{name: 'stable'}, {name: 'main', prerelease: 'beta', channel: 'beta'}]`. Promote `stable` **only by fast-forward** to a `main` commit, enforced by branch protection or a promotion workflow. A back-merge is the fallback.
- Use `@semantic-release/exec`:
  - `prepare`: run a script that writes the version into `Cargo.toml`, `tauri.conf.json` and `package.json`, then builds, signs, notarizes and staples;
  - `publish`: upload the assets and update the tap.
  The tag is created only after `prepare` succeeds. Use `@semantic-release/git` to commit the version bump with `[skip ci]`, or derive the version at build time from an env var (`TWINS_VERSION`) and keep the manifests at a placeholder. Pick one, and test it with `--dry-run`.
- Enforce commitlint on PR titles.

**Phase to address:** Release (Step 5).

---

### Pitfall 19: Homebrew cask quirks

**What goes wrong:**
- Since Homebrew 5, casks failing Gatekeeper (unsigned or not notarized) were deprecated and then **disabled on 2026-09-01**, and `--no-quarantine` was removed (MEDIUM: Homebrew discussions and brew#20755). An un-notarized beta build is therefore uninstallable, even from a personal tap.
- `homebrew/cask` has notability rules. A personal tool belongs in an **own tap** (`ayhid/homebrew-tap`).
- Uninstalling the app leaves the SMAppService agent registered, plus LaunchAgents, `~/Library/Application Support/twins` and `~/Library/Logs/twins`.
- The CLI must be exposed with the `binary` stanza pointing inside the `.app`, which keeps a stable path. Copying it to `/opt/homebrew/bin` breaks TCC identity and code-signature continuity.
- If the app ever self-updates (Tauri updater), the cask needs `auto_updates true`, or brew and the updater fight. Simplest: no in-app updater, brew is the update channel.
- Beta and stable as one cask with two versions does not work. Use `twins` plus `twins@beta`, which `conflicts_with` each other.

**How to avoid:**
- Own tap. A `binary "#{appdir}/twins.app/Contents/MacOS/twins"` stanza (or the helper path).
- `uninstall quit:` plus a pre-uninstall step that unregisters the agent (`twins agent unregister`, or `launchctl bootout`).
- A `zap trash:` list covering Application Support, Logs, Preferences and Caches.
- `depends_on macos: ">= :ventura"` (SMAppService needs macOS 13+).
- The release job updates `version` and `sha256` in the tap via a PAT-authenticated commit.
- `brew audit --cask --strict --online` and `brew install --cask` in CI before publishing.

**Phase to address:** Release (Step 5).

---

## Moderate Pitfalls

| Pitfall | What goes wrong | Prevention | Phase |
|---|---|---|---|
| Files changing mid-scan | Size comes from the walk-time `stat`, but the content is hashed later. A growing file gets a partial hash and could group with a truncated copy | `fstat` the open fd after hashing. If size, mtime or ctime changed, drop the file from this scan (count it as `changed_during_scan`) | Step 2/3 |
| Config can silently widen scope or arm danger | Unknown keys ignored (`exclude` vs `excludes` typo means no excludes); `delete_mode = "permanent"` or `yes = true` in config makes scripts destructive; bool flags cannot un-set a config `true` | `#[serde(deny_unknown_fields)]`. **Dangerous options (permanent, --yes, --force) are flag-only, never config**. Every bool flag gets a `--no-*` twin. Expand `~` and resolve relative paths against the config file's dir. `twins config show` prints the effective merged config with the source of each value | Step 3 |
| Reclaimable space overstated | Hardlinks, existing clones and APFS local snapshots mean deleting frees less than the file size, and users lose trust | Compute with `ATTR_CMNEXT_PRIVATESIZE`. Label "frees after Trash is emptied and snapshots expire" | Step 2 (report), Step 4 (UI) |
| launchd schedule semantics | Missed `StartCalendarInterval` runs while asleep fire once on wake (coalesced); `StartInterval` drifts; `ProcessType=Background` throttles I/O so a scan takes 10x longer; an in-app tokio timer misbehaves across sleep | Use `StartCalendarInterval` via the agent plist. Persist `last_run` and skip if run within the window. Defer on battery (IOKit power source). Accept slow throttled I/O for background runs | Workflows |
| Workflow files as an untrusted format | Hand-edited or synced workflow files with a newer schema, or a graph cycle in the node model, crash or misinterpret the runner | Version the schema. Validate graph to list compile (acyclic, one trigger, one outcome). Reject unknown node types. The runner executes only the compiled linear form | Workflows |
| Notifications from a headless runner | `osascript display notification` gets attributed to Script Editor; a bare binary cannot use `UNUserNotificationCenter` | Notifications come from the helper app bundle (see Pitfall 10) | Workflows |
| `unsafe_code = "warn"` vs heavy FFI | `objc2` (NSFileManager), `clonefile`, `renamex_np`, `getattrlist`, `copyfile`, FSEvents and SMAppService add many `unsafe` blocks across modules | Put all FFI in one `twins-core::sys` (or `twins-macos`) module with `SAFETY:` comments and safe wrappers. Allow the lint only there | Step 2 onward |
| Partial hash treated as final | A refactor short-circuits after the xxHash64 head/tail stage for speed, so files that differ in the middle get deleted | Assert in the plan builder that every group passed full BLAKE3, with `--verify` byte-compare available. Add a test with files differing only in the middle | Step 2 |
| Parity with Go v0 | The rewrite changes the keep tie-break order or the journal format, and existing user scripts or habits produce different keepers | Run golden tests against recorded Go v0 outputs on a fixture tree. Document intentional deltas | Parity |

## Minor Pitfalls

- **Error flooding in verbose and background runs.** Thousands of EPERM lines (CONCERNS.md). Summarise by error kind in the journal and notification. Add `--fail-on-error` for scripts.
- **Unicode normalisation in reports and UI.** APFS preserves NFC/NFD as given. Comparing display strings across sources (FSEvents paths vs walk paths) fails. Compare by identity, display as-is.
- **Quick Look.** `qlmanage -p` is a debug tool. Use `QLPreviewPanel` through a small native shim, or accept "Reveal in Finder" (`NSWorkspace activateFileViewerSelectingURLs`) for v1.
- **Locked files (`uchg`) and immutable flags.** The trash or rename fails. Journal `failed: locked` and never `chflags` them automatically.
- **`/usr/bin/trash` (macOS 15+).** It is tempting to shell out to it, but it has Finder display glitches (LOW: mjtsai blog) and gives no resulting URL. Do not use it.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `trash::delete_all` with default `Finder` method | One-line trash with Put Back | Apple Events permission, sound, no resulting URL, breaks under the hardened runtime and the agent | Never |
| `trash` crate with `NsFileManager` | Fast, no permissions | No resulting URL, so no journal-based undo | Only as a stopgap before calling objc2 directly in Step 2 |
| Executor trusts the cached hash | Warm runs skip a re-read | Cache bug becomes data loss | Never |
| Cache keyed on `st_dev` | Trivial to implement | Misses after reboot, possible cross-volume aliasing | Never (volume UUID is ~30 LOC) |
| Lexical `starts_with` protection | Already exists | Case and symlink bypass once deletion exists | Only for read-only scan. Must be fixed before Step 2 ships |
| Delete fallback `trash` → `remove_file` | "It always works" | Silent permanent deletion | Never |
| Plain launchd plist running the CLI | No helper bundle needed | No FDA, no notifications, TCC confusion | Never for the shipped agent. Fine for a dev spike |
| `externalBin` sidecar for the CLI | Tauri handles it | Notarization failures (tauri#11992) | Acceptable if CI notarization passes. Keep a manual signing fallback |
| Version only in git tags | Nothing to sync | `twins version`, report meta and the bundle disagree | Never. Inject at build |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| NSFileManager trash | Pass `nil` for `resultingItemURL`. Assume Put Back | Capture the URL into the journal. Implement `twins undo`. Put Back is best-effort |
| clonefile(2) | Clone onto B. Assume B's metadata survives | Clone to temp, copy B's metadata, then `renamex_np(RENAME_SWAP)`, then trash the original |
| getattrlist | Ignore it | Use `ATTR_VOL_UUID` for cache keys and `ATTR_CMNEXT_PRIVATESIZE`/`CLONEID` for true savings |
| FSEvents / `notify` crate | Process events incrementally. Ignore `Rescan`/`MustScanSubDirs` | Debounce, settle, full rescan on drop flags, single-flight, rate-limit |
| SMAppService | Register the agent from a non-bundled binary. Forget to unregister on uninstall | Helper app inside `Contents/Library/LaunchAgents` + `BundleProgram`. Unregister in the cask `uninstall` |
| TCC / FDA | Ask for it in code (no API exists). Test only from Terminal | Detect with a protected-file probe and deep-link to Settings. Test the app and agent launched from Finder or launchd |
| semantic-release | Merge `main` into `stable`. Build after tagging | Fast-forward promotion. Build, sign and notarize in `prepare` |
| Homebrew | Submit to `homebrew/cask`. Ship un-notarized betas | Own tap. `twins` and `twins@beta` both notarized. `binary`/`zap`/`uninstall` stanzas |
| Tauri v2 IPC | Commands taking raw paths. `fs` plugin enabled | Plan IDs only. Minimal capabilities. Throttled `Channel` for progress |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-verify by full re-hash on huge groups | Clean of 500 GB of video takes as long as the scan | Re-verify only planned pairs. Use `--verify` byte-compare with early exit on the first differing block. Show progress for execution too | Groups with multi-GB files |
| Progress event per file over IPC | Webview freezes, memory climbs | Throttle to ~10 Hz and aggregate counters | >10k files/s walks |
| Whole report over IPC | Multi-hundred-MB JSON, app hangs | Paginate groups. Keep the report Rust-side | >50k groups |
| FSEvents storm triggers full scans | Agent pegs CPU after `npm install` | Settle window, rate limit, exponential backoff | Any dev folder in scope |
| Cache DB per-file transactions | Warm scan slower than cold | Batch writes in one transaction per N files. WAL mode | >100k files |
| Background `ProcessType` I/O throttling | Scheduled scans take an hour | Expected. Show "last run duration". Allow an "interactive priority" option | Large scopes on schedule |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Webview-callable path deletion | Any XSS or dependency compromise can wipe user data with FDA privileges | Plan-ID-only IPC, Rust-side validation, no `fs`/`shell` plugins |
| Workflow and config files writable by other tools, honoured blindly | A malicious or synced file arms auto-clean on `~` | Validate. A scope or outcome change re-arms report-only (Pitfall 12). Files must be owned by the user with mode `0600`/`0644` |
| Running as root (`sudo twins clean`) | Bypasses user-level protections, root-owned journal | Refuse to run deletion as root without an explicit `--allow-root` |
| Over-broad entitlements | Weakens the hardened runtime for no gain | No JIT or unsigned-memory entitlements unless proven necessary |
| Logging full paths in notifications | Leaks file names on the lock screen | Notifications show counts and bytes. Details stay in the app |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| "Freed 12 GB" while the space is still in Trash or snapshots | Distrust when the disk is still full | "Moved 12 GB to Trash. Empty Trash to reclaim" |
| Auto-select keeps a file in a surprising place (e.g. keeps `~/Downloads/x.pdf`, trashes `~/Documents/Taxes/x.pdf`) | User loses their organisation | Show the keeper's path prominently. Support `in-dir` preferences. Warn when the keeper is in Downloads, Desktop or temp |
| Silent partial scans without FDA | "No duplicates" when there are many | Banner showing "N folders skipped: grant Full Disk Access" |
| An auto-clean notification with no undo | Panic | "Moved 37 files to Trash (Restore)" linking to journal undo |
| Put Back promised but missing | User thinks files are lost | Document that `twins undo` and the app's History view are the restore path |

## "Looks Done But Isn't" Checklist

- [ ] **Trash:** tested with 50 files in one batch *with the Finder Trash window open*. The resulting URLs are journaled, and `twins undo` restores all 50.
- [ ] **Trash on external, exFAT and SMB volumes:** exFAT/SMB groups are refused (no silent unlink). External APFS lands in `.Trashes/<uid>`.
- [ ] **Keep-one:** a property test with hardlink, symlinked-root and case-alias members never yields a plan that removes every identity.
- [ ] **TOCTOU:** a test edits the keeper, edits a victim, and swaps a victim for a symlink between plan and execute. Each case skips the group.
- [ ] **Protection:** `/library`, `/Volumes/<boot>/Library` and a symlinked root are all refused.
- [ ] **Clone mode:** after replacement, B keeps its own mtime/birthtime, tags, xattrs and ACL. A failure mid-way leaves B intact. Already-cloned pairs are skipped.
- [ ] **Cache:** a file rewritten with `touch -r` (same size, restored mtime) is re-hashed, and the cache survives a reboot (volume UUID).
- [ ] **Dry-run:** produces an identical plan to the real run and writes `dry_run: true` records only.
- [ ] **Journal:** a non-UTF-8 filename round-trips, and a crash between intent and done is reconciled on the next start.
- [ ] **FDA:** the app and the agent, launched from Finder and from launchd respectively, can read a TCC-protected file with FDA granted to the app only.
- [ ] **FSEvents:** auto-clean in a watched folder does not re-trigger itself, a `git clone` into scope causes at most one debounced run, and a freshly duplicated file is not trashed.
- [ ] **Circuit breaker:** a run planning more than the threshold degrades to report-only.
- [ ] **Release:** the downloaded DMG launches with quarantine set. `codesign --verify --deep --strict` passes on every nested binary. `twins version` matches the tag.
- [ ] **Cask:** `brew uninstall --zap` removes the agent registration and data dirs.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Wrong files trashed | LOW (if journal is intact) | `twins undo --run <id>` using journaled resulting URLs, or manual Put Back |
| Trashed then Trash emptied | HIGH | Time Machine / APFS snapshot restore. Prevention is the only real fix (circuit breakers, no auto-empty warning) |
| Clone replacement lost B's metadata | MEDIUM | Swapped-out original in Trash (if procedure followed). Restore it with `RENAME_SWAP` back |
| Poisoned cache | LOW | `twins config clear-cache`. Executor re-verification prevented loss |
| Agent feedback loop | LOW | Disable the workflow from the menu or CLI. Rate limiter caps damage |
| Bad release (wrong version / un-notarized) | MEDIUM | Yank the GitHub release, revert the tap commit, re-run the pipeline. Never re-use a tag |
| TCC grants lost after signing change | LOW | Re-grant FDA. Keep one signing identity |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1 TOCTOU | Step 2 Safe deletion | Mutation-between-plan-and-execute tests |
| 2 Keep-one by identity | Step 2 | Property tests on the validator |
| 3 Lexical protection bypass | Step 2 (pre-task) | Case, symlink and `/Volumes/<boot>` tests |
| 4 Put Back assumption | Step 2 (decision + `twins undo`) | 50-file batch test with the Trash window open |
| 5 Per-volume trash | Step 2 | exFAT disk image and SMB tests; no `remove_file` reachable |
| 6 Clone replacement | Step 2 (or 2b) | Metadata round-trip test; failure-injection test |
| 7 Cache poisoning | Step 3 (rule set in Step 2) | `touch -r` test; reboot / volume UUID test |
| 8 Sync folders and catalogs | Step 2 labels → Workflows defaults | `location_kind` in report schema v2 |
| 9 Resource forks | Step 2 | Fixture with identical data fork and differing rsrc |
| 10 TCC/FDA attribution | **Spike before Step 4**, built in Workflows | FDA probe from agent with the grant on the app only |
| 11 FSEvents loops and storms | Workflows | Self-trigger and storm tests |
| 12 Auto-clean blast radius | Workflows | Breaker test; report-only re-arm on scope change |
| 13 Concurrent mutators | Step 2 lock, Step 3 DB, Workflows | Two executors race test |
| 14 Journal | Step 2 | Crash-reconcile and non-UTF-8 tests |
| 15 Dry-run divergence | Step 2 | Plan equality test |
| 16 Tauri IPC | Step 4 | Capability audit; no path-taking destructive commands |
| 17 Signing/notarization | Step 5 (layout from the Step 4 spike) | Quarantined-launch CI smoke test |
| 18 semantic-release | Step 5 | `--dry-run` on both branches; FF-only promotion rule |
| 19 Homebrew cask | Step 5 | `brew audit --strict`, install, uninstall `--zap` in CI |

### Roadmap implications

1. **Add a "safety hardening" task at the start of Step 2**: identity-based protection, root canonicalisation, the identity-keyed plan validator, the execution lock, and the write-ahead journal. All deletion modes build on it.
2. **Rewrite the trash requirement**: "Trash plus journal-based `twins undo`. Finder Put Back is best-effort." This is a decision for the user.
3. **Consider splitting clone mode** into its own phase after trash and permanent ship. It has the most subtle semantics.
4. **Run a bundle-layout and TCC spike before the app phase** (helper app, SMAppService, FDA inheritance, CLI placement, notarization of nested code). Its outcome constrains Steps 4, Workflows and 5.
5. **Workflows needs deeper phase research** on FSEvents debounce design, circuit breakers and launchd scheduling.

## Sources

- twins source, read directly (HIGH): `crates/twins-core/src/safety.rs`, `crates/twins-core/src/scan/walk.rs` (lines 123-124, 224-260), `crates/twins-core/src/scan/rules.rs`, `Cargo.toml` (`version = "1.0.0-beta.0"`), `.planning/codebase/CONCERNS.md`
- walkdir 2.5.0 source, `follow_root_links` default `true` (HIGH, local registry)
- trash-rs source, `src/macos/mod.rs` and `src/lib.rs` `canonicalize_paths`, crate v5.2.9 (HIGH): https://github.com/Byron/trash-rs
- Apple Developer Forums, "trashItem, recycle, but no put back option…it depends" (DTS, Feb 2025) (MEDIUM): https://developer.apple.com/forums/thread/773997
- sindresorhus/macos-trash#4 "Put back only works for the first file" (MEDIUM): https://github.com/sindresorhus/macos-trash/issues/4
- openradar 23153124 mirror (MEDIUM): https://github.com/lionheart/openradar-mirror/issues/6452
- clonefile(2) man page (MEDIUM): https://keith.github.io/xcode-man-pages/clonefile.2.html
- getattrlist(2) man page; `ATTR_CMNEXT_PRIVATESIZE` usage notes (MEDIUM/LOW): https://keith.github.io/xcode-man-pages/getattrlist.2.html
- `st_dev` instability across reboots, fixed by volume UUID (LOW, single project, consistent with Apple forum threads): https://github.com/ctxrs/ctx/pull/1075, https://developer.apple.com/forums/thread/128947
- Git racy-git documentation (MEDIUM): https://git-scm.com/docs/racy-git/2.0.5.html
- Apple DTS "The Rules for Full Disk Access" (MEDIUM, 2018-era, verify on macOS 15/26): https://developer.apple.com/forums/thread/107546
- Full disk access from a launchd daemon (MEDIUM): https://developer.apple.com/forums/thread/661178
- FSEvents coalescing and rescan semantics (LOW/MEDIUM): https://danielcosenza.com/posts/mac-fsevents/, https://github.com/facebook/watchman/issues/1354
- Tauri v2 macOS signing docs (MEDIUM): https://v2.tauri.app/distribute/sign/macos/
- tauri#11992 externalBin notarization failure (MEDIUM): https://github.com/tauri-apps/tauri/issues/11992
- Tauri custom CFBundleVersion (MEDIUM): https://github.com/tauri-apps/tauri/pull/13030
- semantic-release workflow configuration (MEDIUM): https://semantic-release.gitbook.io/semantic-release/usage/workflow-configuration
- Homebrew Gatekeeper policy and `--no-quarantine` removal (MEDIUM): https://github.com/Homebrew/brew/issues/20755, https://github.com/orgs/Homebrew/discussions/6482
- Homebrew Acceptable Casks (MEDIUM): https://docs.brew.sh/Acceptable-Casks
- Lightroom catalog damage from external duplicate finders (LOW): https://community.adobe.com/t5/lightroom-classic-discussions/delete-duplicates/td-p/11202732
- `/usr/bin/trash` on macOS 15+ (LOW): https://mjtsai.com/blog/2025/08/26/the-trash-command/

---
*Pitfalls research for: macOS safe duplicate finder (twins), subsequent milestone*
*Researched: 2026-10-02*

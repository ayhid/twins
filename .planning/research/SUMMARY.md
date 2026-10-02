# Project Research Summary

**Project:** twins
**Domain:** macOS duplicate-file finder and cleaner (Rust core + CLI, Tauri v2 + Svelte desktop app, workflow builder with a background agent, signed/notarized Homebrew cask)
**Researched:** 2026-10-02
**Confidence:** MEDIUM-HIGH (stack and architecture verified against registries and crate source; several macOS platform behaviours still need on-hardware spikes)

## Executive Summary

twins is a safety-first duplicate cleaner. The scan and hash engine already exists in `twins-core`; the remaining work adds destructive operations (Trash, permanent delete, APFS clone), a persistent hash cache and config, a desktop app, and unattended workflows. Mature tools (fclones, Czkawka, Gemini 2, dupeGuru) converge on the same cleaning flow: scan, review, select, confirm, execute, recover. twins differs by treating "never lose data" as an engine invariant rather than a UI warning. One core crate owns every decision; the CLI, app and agent are thin observers. Deletion only accepts a `ValidatedPlan` newtype that `validate()` alone can build. Every file is re-checked against the live filesystem under an execute lock right before it is acted on, and a write-ahead journal records every operation.

The stack adds to the existing crates without replacing any of them. Raise MSRV to 1.90 (needed by tauri 2.12; also brings std file locks). Trash through `objc2-foundation` `NSFileManager trashItemAtURL:resultingItemURL:` directly, not the `trash` crate. Clone through `rustix::fs::fclonefileat`. Store the cache and state in `rusqlite` (bundled, WAL), because three processes share them and redb cannot. Use TOML for config and workflows. Ship a headless `twins-agent` as a Tauri `externalBin` registered through `SMAppService`. Frontend: Svelte 5 + SvelteKit 3 SPA + `@xyflow/svelte` + `virtua`, TypeScript pinned to 6.x. Release: semantic-release 25, tauri-action, a cask in the user's tap. Signing and notarization are mandatory now that Homebrew disables casks failing Gatekeeper.

The main risks all threaten the Core Value: acting on a stale scan (TOCTOU), checking keep-one by path instead of `(volume, inode)` identity, lexical protection checks that are bypassable on case-insensitive APFS, and assuming Finder Put Back works for batch trashing (it does not; radar 23153124). The requirement should become "Trash + journal-based `twins undo`; Put Back best-effort". Automation adds FSEvents feedback loops, an unbounded auto-clean blast radius, and unverified Full Disk Access attribution for the agent, which needs a signed-bundle spike before the workflow phases depend on it.

## Key Findings

### Recommended Stack

Existing scan/hash stack (walkdir, rayon, blake3, xxhash-rust, globset, clap, serde, thiserror, libc) is unchanged. Additions:

**Core technologies:**
- Rust MSRV **1.90**, edition 2024: required by tauri 2.12.1; std `File::lock` replaces fs2/fd-lock
- `objc2` 0.6.4 / `objc2-foundation` 0.3.2: direct `trashItemAtURL` with `resultingItemURL` for the journal; no osascript, no Automation prompt
- `rustix` 1.1.5 (`fs`): safe fd-based `fclonefileat` + `renameat`. Not `reflink-copy` (silently falls back to a full copy)
- `rusqlite` 0.40.2 (`bundled`, WAL, `busy_timeout`): `cache.db` and `state.db`, multi-process safe
- `toml` 1.1.6: `config.toml` and `workflows/<id>.toml`
- `tauri` 2.12.1 with `ipc::Channel<T>` for progress (not events), throttled to 10-20 msg/s
- `notify` 8.2.0 + `notify-debouncer-full` 0.7.0: FSEvents trigger in the agent
- `croner` ~4.0 (jiff backend) + `jiff` 0.2.37: schedules and timestamps
- `objc2-service-management` 0.3.2: `SMAppService.agent` (not `tauri-plugin-autostart`)
- `objc2-user-notifications`: agent notifications (not notify-rust / mac-notification-sys)
- Svelte 5.57 + SvelteKit 3.0 (adapter-static 4, SPA) + Vite 8 + TypeScript **6.0.3** (not 7)
- `@xyflow/svelte` 1.7 (graph canvas), `virtua` 0.52 (virtualized group list)
- semantic-release 25 + `@semantic-release/exec`, tauri-action v1, tap `ayhid/homebrew-tap`

**Explicitly avoid:** `tauri-plugin-fs` in the webview, `tauri-plugin-store` (second source of truth), `tauri-plugin-updater` (the cask owns updates), `tauri-plugin-shell` (use opener), App Sandbox, redb/sled, serde_yaml.

### Expected Features

**Must have (table stakes):**
- Multi-location scan with excludes/size/type filters; staged progress with Cancel that keeps the cache consistent
- Summary with honest reclaimable bytes (excluding hardlinks, ideally clones)
- Virtualized group browser: sort, filter, Reveal in Finder, Quick Look
- Keeper-based selection (radio semantics), auto-select by strategy, folder context actions
- Last copy never markable, enforced in core
- Trash default; permanent behind typed confirmation / `--force`; dry-run = the plan preview
- Confirmation sheet; execution with per-file errors; live re-validation before acting
- JSON export, FDA onboarding, bundles/libraries never touched
- Workflows: file format, `twins run`, manual/schedule/folder-change triggers, report-only default, auto-clean Trash-only with caps, run history, enable/disable, global pause

**Should have (differentiators):**
- Keep-one as an engine invariant with no override
- Journal-based undo across restarts (`twins undo <run-id>`); capture the data from day one
- "Why kept" reason per keeper
- **Protected (reference) paths.** This fills a requirements gap: `in-dir` is a preference, not a protection
- APFS clone-aware accounting and clone mode
- Constrained typed-port graph editor with a total list projection

**Defer (v1.x / v2+):** graph canvas until the list view proves the model; undo UI if it does not fit v1; menu bar item; Finder Quick Action; duplicate folders, volume-mounted trigger, similar images (v2+).

**Anti-features:** fuzzy matching, symlink/hardlink replacement, "learning" selection, deleting the last copy, dedup inside Photos/Music, quarantine folder, unattended permanent delete, per-file workflow rules, free-form n8n canvas, script/HTTP nodes, Hazel-style actions, workflow chaining.

### Architecture Approach

All logic lives in `twins-core` and `twins-workflow`. The three shells coordinate only through an on-disk contract (config.toml, workflow TOML, cache.db/state.db in WAL, flock locks, the JSONL journal in `~/Library/Logs/twins/`), never over sockets.

**Major components:**
1. `core::observe` / `pipeline`: CancelToken, Event, Observer, Throttle; orchestration moved from `twins-cli/run.rs`
2. `core::plan`: Selection → Plan → `ValidatedPlan` (private constructor, identity-based keep-one)
3. `core::exec` + `journal` + `lock`: the single deletion routine (execute lock, re-stat, backend Trash | Permanent | Clone | DryRun, write-ahead journal)
4. `core::cache` / `config`: `CachingHasher` over SQLite, layered TOML
5. `twins-workflow`: pure sync lib (format, graph validation, compile to RunSpec, list projection, runner, trigger maths)
6. Shells: `twins-cli`; `twins-app` (Rust-held sessions, paged queries, Channel progress, no path-taking destructive commands); `twins-agent` (SMAppService LaunchAgent, scheduler with catch-up, FSEvents, serial queue, notifications)

**Patterns:** orchestration in core with shells as observers; validated-plan newtype; locked execute with per-file re-validation; graph as source of truth with the list as projection; the agent as the only trigger evaluator; async + `spawn_blocking` Tauri commands; dry-run via the same path.

### Critical Pitfalls

1. **Stale scan (TOCTOU):** before each group, lstat keeper and victims, match `(volume, inode, size, mtime, ctime)`, and re-hash bypassing the cache. Skip the whole group on drift. Plans expire.
2. **Keep-one by path:** validate on `(volume_uuid, inode)` and property-test with aliases (hardlinks, symlinked roots, case aliases, firmlinks).
3. **Lexical protection bypass (existing bug):** `/library` and `/Volumes/Macintosh HD/Library` pass `safety::is_protected`. Canonicalise, compare by `(dev, inode)`, `follow_root_links(false)`. Fix before deletion lands.
4. **Put Back assumption:** unreliable for batches. Journal undo is the contract. Never fall back to `remove_file` when Trash fails (fail closed).
5. **Unattended automation:** FSEvents loops and storms, half-written files, auto-clean blast radius. Mitigate with debounce, self-suppression, circuit breakers, and report-only re-arm on scope change.

Also carry forward: cache as deletion evidence (#7), sync folders and resource forks (#8, #9), concurrent mutators (#13), journal undoability (#14), dry-run divergence (#15), Tauri IPC exposure (#16), multi-binary notarization (#17), semantic-release promotion (#18), cask quirks (#19).

## Implications for Roadmap

### Phase 1: Core refactor (observe + pipeline)
**Rationale:** Unblocks everything; low risk; existing tests are the net.
**Delivers:** observer/cancel/throttle, pipeline in core, `meta.dry_run` fixed, MSRV 1.90.
**Avoids:** #15.

### Phase 2: Safety hardening + safe deletion
**Rationale:** Highest Core Value risk; the app and workflows both call `exec::execute`.
**Delivers:** identity-based protection (pre-task), `ValidatedPlan`, keep strategies with reasons, protected paths, execute lock, re-validation, objc2 Trash with `resultingItemURL`, permanent, DryRun, write-ahead journal, `twins clean`, ideally `twins undo`.
**Avoids:** #1-5, #9, #13-15.
**User decision:** trash requirement rewording.

### Phase 3: APFS clone mode
**Rationale:** The subtlest semantics; split out so it does not delay trash/permanent.
**Delivers:** fd-based clone + atomic rename, cross-device check, metadata round-trip, clone-aware accounting.
**Avoids:** #6.

### Phase 4: Hash cache and config
**Rationale:** Independent seams; can run in parallel with 2-3.
**Delivers:** SQLite WAL `CachingHasher` (volume UUID keyed), layered config.toml, `twins config ...`.
**Avoids:** #7.

### Phase 5: Desktop app (scan, browse, select, clean)
**Delivers:** FDA onboarding, Channel progress/cancel, summary, virtualized browser, Quick Look/Reveal, keeper selection, confirmation, execution via core; bundle layout.
**Avoids:** #16.
**Ends with spike:** signed bundle + SMAppService agent. Check FDA inheritance, notifications, Login Items flow; the fallback is a helper .app.

### Phase 6: Workflow model and runner (headless)
**Delivers:** `twins-workflow`, `twins run`, `workflow validate|list`, history, per-workflow lock, report-only default, Trash-only auto-clean with circuit breakers.
**Avoids:** #12, #13.

### Phase 7: Workflow builder UI
**Delivers:** list editor + preview run first, then the `@xyflow/svelte` typed-port canvas (may slip to v1.x).

### Phase 8: Background agent
**Delivers:** scheduler with catch-up, FSEvents (debounce, partial-download ignore, self-suppression), serial queue, notifications, FDA probe, SMAppService toggle, global pause.
**Avoids:** #10, #11, #12.

### Phase 9: Release
**Delivers:** hardened-runtime signing of all binaries, notarization, semantic-release channels (FF-only promotion), cask exposing `twins` as a binary, CI smoke tests.
**Avoids:** #17-19.

### Phase 10: Go v0 parity verification
**Rationale:** Explicit requirement; verify against `9024929` (or fold into each phase's verification).

### Phase Ordering Rationale
- 1 unblocks all; 2 precedes any acting shell; 3 is isolated for risk; 4 can run in parallel.
- The spike at the end of 5 de-risks 8 and 9 before workflows build on the bundle layout.
- 6 precedes 7 and 8 since both are shells over the workflow crate.

### Research Flags
Needs research: Phase 2 (trash threading, batch Put Back, per-volume Trash), 3 (clone metadata), 5 (SvelteKit 3 + Tauri, QLPreviewPanel, agent/TCC spike), 7 (Svelte Flow port typing), 8 (FSEvents debounce, breakers, SMAppService, notifications), 9 (multi-binary notarization, cask policy).
Standard patterns: Phases 1, 4, 6.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM-HIGH | Registry versions plus crate-source checks; SvelteKit 3 and croner 4 are very new |
| Features | MEDIUM | Competitor docs primary; user expectations are synthesis |
| Architecture | MEDIUM-HIGH | Boundaries/locking/IPC HIGH; agent packaging/FDA LOW-MEDIUM |
| Pitfalls | MEDIUM | Source-verified HIGH; Apple behaviour from DTS/radars MEDIUM |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address
- Agent FDA inheritance and notifications: Phase 5 spike (fallback: helper .app)
- Batch Put Back on macOS 26 and trash threading: Phase 2 spike
- Trash requirement wording: user decision
- Protected paths are missing from requirements: add to Phase 2
- Undo and graph canvas, v1 vs v1.x: user decision
- Schedule syntax: UI presets, raw cron in files
- Quick Look from Tauri (fallback `qlmanage -p`); SvelteKit 3 freshness (fallback Kit 2.70.x)

## Sources

### Primary (HIGH confidence)
- crates.io/npm registry metadata (2026-10-02); source of trash 5.2.9, tauri-bundler 2.10.1, redb 4.3.0, auto-launch 0.6.0, notify 8.2.0, rustix 1.1.5, walkdir 2.5
- twins source at `c962eec`; Tauri v2 docs; fclones README and `dedupe.rs`

### Secondary (MEDIUM confidence)
- Apple DTS threads and radar 23153124; sindresorhus/macos-trash; Eclectic Light; Gemini 2, Czkawka, Hazel, n8n docs; Homebrew Gatekeeper policy

### Tertiary (LOW confidence)
- Single-source claims on SMAppService FDA inheritance and agent notification identity

---
*Research completed: 2026-10-02*
*Ready for roadmap: yes*

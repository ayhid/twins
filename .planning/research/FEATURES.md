# Feature Research

**Domain:** macOS duplicate-file finder/cleaner (CLI and desktop app) with a workflow/automation builder
**Researched:** 2026-10-02
**Confidence:** MEDIUM overall. Competitor behaviour comes from primary docs (fclones README and `dedupe.rs` source, the Czkawka/Krokiet repo instructions, the Gemini 2 user guide PDF, the Hazel manual, the n8n docs), so it is solid. Statements about what users expect are synthesis, so treat them as MEDIUM. APFS clone-ID details come from secondary sources (Eclectic Light plus several OSS PRs) and are MEDIUM.

This file covers two products that share one engine:

- **A. The cleaning flow:** scan → review → select → confirm → execute → recover (CLI and app).
- **B. The automation layer:** workflows, meaning scope + trigger + outcome, built in a graph or list editor and run by a background agent.

A short section **C** covers config, cache and distribution, and **D** is a parity checklist against Go v0 (`9024929`).

---

## Feature Landscape

### A. Cleaning flow: Table Stakes (users expect these)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Multi-location scan with excludes, min size, type/extension filters | Every competitor has it: Gemini Ignore List (files, folders, extensions), Czkawka included/excluded paths + size filter, fclones `--name/--path/--exclude/-s` | LOW | Mostly exists in CLI flags. The app needs a location list with drag-and-drop and a persisted ignore list. |
| Live progress with stages, counts and **Cancel** | Gemini, Czkawka and fclones all show progress. Scans of `~` take minutes | MEDIUM | The pipeline already has stages (walk → size → partial → full). Emit a progress event per stage. Cancelling must leave the cache consistent: Czkawka keeps hashes computed before a stop, and twins should do the same. |
| Results summary: reclaimable bytes, group count, breakdown by type | Gemini's summary chart is the entry screen, and users decide whether to bother based on this number | LOW | "Reclaimable" must exclude hardlinks and (ideally) APFS clones, or the number lies. See the differentiator on clone awareness. |
| Group browser: expand/collapse, sort (wasted bytes desc by default, count, name), path/name search filter | Gemini list/grid + sort by Size/Name/Count, Czkawka sort popup + filters, Go v0 TUI `/` filter | MEDIUM | Must be virtualized: 100k+ rows is normal for `~`. Gemini caps how many results it shows ("shows only the largest"), which reads like a bug to users, so don't copy it. |
| Reveal in Finder and Quick Look per file | Go v0 `o`/`p` keys, Gemini magnifier/preview pane, Czkawka "Open Item / Open Parent Folder" | LOW | Space bar = Quick Look is the macOS convention. Use `NSWorkspace activateFileViewerSelectingURLs` and a QLPreviewPanel (or `qlmanage -p` as a fallback). |
| Auto-select by keep strategy, plus manual override per group | Gemini filters (Automatically/Newest/Oldest/Any), Czkawka Select popup (one oldest/newest/biggest/smallest, shortest/longest path, invert), fclones `--priority`, Go v0 `a`/`u` | MEDIUM | Strategies are fixed by PROJECT.md: oldest, newest, shortest-path, in-dir. Model each group as **"which copy is kept"** (radio semantics) plus "also keep" toggles, not as free removal checkboxes. That makes "zero keepers" impossible to express in the UI. |
| "Select all copies in this folder" / "Exclude this folder" context actions | Gemini "Select All Copies Within folder", Czkawka "Select All from Folder (recursive)", "Exclude Parent Folder" | LOW | This is the most common real-world gesture ("everything in ~/Downloads goes"). Its result still passes through the keep-one validation. |
| Never mark the last copy of a group | Go v0 already does this. Gemini *warns* and allows it via "Remove Last Instance" | LOW | twins makes this a hard rule rather than a warning, so it is also a differentiator (see below). Enforce it in core, not only in the UI. |
| Trash as the default, with Finder **Put Back** working | Gemini, Czkawka, dupeGuru and Nektony all default to Trash. Gemini's own docs admit Put Back "may not be available" for its removals | MEDIUM | Use `NSFileManager trashItemAtURL:resultingItemURL:`, which records Put Back metadata. `rename()` into `~/.Trash` does not. Record `resultingItemURL` in the journal. |
| Permanent delete behind strong confirmation | Gemini "Remove permanently", Czkawka Delete, fclones `remove`. jdupes requires dangerous flags **twice** | LOW | Go v0: type `permanent`, or `--force` for scripts. Keep both. |
| Dry-run that changes nothing but shows exactly what would happen | fclones `--dry-run` prints the exact commands, Go v0 journals the dry run | LOW | `meta.dry_run` is currently hardcoded false (CONCERNS). The app needs a "Preview" step that is literally the dry-run plan. |
| Confirmation sheet before execution: count, bytes, mode, destination, sample paths | Every GUI competitor has one, and it is the last line of defence | LOW | Show the mode prominently (Trash vs Permanent vs Clone). Permanent turns the sheet red and requires typing. |
| Execution progress, per-file error list, completion summary | Gemini cleanup-completion screen ("Review Trashed", "Put All Back"), fclones per-file logs | MEDIUM | Partial failure is normal (locked files, permissions, file vanished). Continue past per-file errors and report them, but abort the *group* if its keeper becomes unavailable. |
| **Re-validate the plan against the live filesystem just before acting** | fclones (`dedupe.rs`) drops files whose size changed and skips the whole group if any file was modified after the report timestamp | MEDIUM | This is critical for twins: the GUI user may browse for 20 minutes, and workflows act on a scan taken moments ago. Before each group, re-stat every member (size, mtime, dev/inode), confirm the **keeper still exists and is unchanged**, and skip the group on any drift. Without this, keep-one is enforced on a stale plan rather than on disk. |
| Export / machine-readable report | fclones JSON/CSV/fdupes, Czkawka Save, Go v0 `report` JSON | LOW | Exists (`twins report`, schema v1). The app gets an "Export JSON" action reusing the same schema. |
| Full Disk Access onboarding | Gemini asks for Home access, and every scanner of `~/Library`, Mail or Messages needs FDA | LOW | Detect the missing permission (a probe read of a TCC-protected path) and deep-link to System Settings → Privacy → Full Disk Access. Show "N folders skipped: no permission" rather than silently under-reporting. |
| Bundles and libraries never opened or deduplicated inside | Gemini warns about and special-cases Photos/iTunes libraries. Deleting inside `.photoslibrary` corrupts it | LOW | Already validated (bundles never opened). Keep it, and mention it in the UI ("Photos Library skipped"). |

### A. Cleaning flow: Differentiators (competitive advantage, aligned with "Never lose data")

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Keep-one as an engine invariant, with no override anywhere | Gemini lets users delete the last instance after one checkbox, and fclones `--match-links --symbolic-links` is documented as "very dangerous" | LOW | A single core routine validates every plan (CLI, app, workflow). The UI cannot express it, the CLI cannot be forced into it, and the error message names the group. |
| **Undo the last clean from the journal** ("Put back everything from run X") | Gemini restores only within the current session. Nektony sells a "removal history". twins can restore across restarts because the journal records the trash location | MEDIUM | Design for it in Step 2 even if the UI ships later: each journal line stores original path, `resultingItemURL`, size, hash and run id. `twins undo [run-id]` plus an app button. Restore refuses to overwrite if the original path is now occupied. |
| Explain *why* each copy is kept ("oldest mtime", "under keep dir", "fallback: oldest") | Gemini's "Automatically" is a black box, and users distrust auto-selection they can't explain | LOW | The keep strategy returns a reason enum alongside the choice. Show it as a tooltip or badge in the browser and as a field in the JSON report. |
| **Protected (reference) paths:** never removed, always count as a keeper | dupeGuru "Reference" folder state, Czkawka "Ref" checkbox, Gemini "Never Select", fclones `--keep-path` | LOW–MEDIUM | **Requirements gap:** `in-dir` is *keeper preference*, not protection. With two copies under the keep dir, `in-dir` will remove one of them, and a user who thinks "keep dir = safe" loses that file. Protected paths only *shrink* the deletion set, so they are compatible with "workflows don't override keep logic". Recommend adding them to Step 2 or Step 3 config. |
| APFS clone-aware accounting and clone mode | No mainstream GUI competitor handles this. jdupes `-B` and fclones `dedupe` create clones but don't discount existing ones. After `--link`, every rescan reports the same files as duplicates again, with 0 bytes actually freeable | MEDIUM | Read `ATTR_CMNEXT_CLONEID` / `EF_MAY_SHARE_BLOCKS` via `getattrlist`, then treat files with the same clone ID like hardlinks (one physical copy) or mark them "already deduplicated". Clone mode must also preserve the replaced file's own metadata (mode, owner, xattrs, Finder tags, dates) since its path survives. Use the fclones pattern: clone to a temp name in the same directory, then atomic `rename` over the duplicate. |
| Trash fail-closed | Gemini silently offers "permanent only" on volumes without a Trash | LOW | If trashing fails (no Trash on volume, network share), the file is skipped and reported. twins never falls back to permanent. This matters most for unattended workflows. |
| Scan transparency: "what was skipped and why" | Users distrust "0 duplicates" when the tool silently skipped folders | LOW | Counters for permission-denied, iCloud placeholders, bundles, protected system paths, remote volumes, below min size. The data already exists in the walker. |
| Finder integration: Services / Quick Action "Find duplicates in…" | Gemini ships "Scan for Duplicates with Gemini 2" in Finder Services | LOW–MEDIUM | Nice entry point. It opens the app pre-filled with the selected folders. Defer to v1.x. |
| Verified execution for unattended deletes | No competitor does this. Hash-cache collisions are a data-loss vector (see PITFALLS) | MEDIUM | For auto-clean runs, re-hash (bypassing cache) or byte-compare each to-be-trashed file against its keeper before trashing. Cost is bounded because only the deletion set is touched. |

### A. Cleaning flow: Anti-Features (deliberately NOT build)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| "Similar" / fuzzy matching (similar images, music tags, videos) | Gemini and Czkawka headline it, and it frees more space | Similar ≠ identical, so deleting one loses information. It needs perceptual hashing, thumbnails and a different review UX. It dilutes the exact-match safety story | Exact content only. If ever added, a separate report-only mode that is never auto-selected (Gemini doesn't auto-select similars either). |
| Replace with **symlinks** | dupeGuru, Czkawka and fclones offer it | Breaks when the keeper moves or is trashed. Apps and backups treat symlinks differently. Combining it with link matching is the documented fclones footgun | APFS clone mode, which gives independent files with shared blocks. |
| Replace with **hardlinks** | Gemini, dupeGuru, Czkawka and fclones `link` offer it | Editing one path silently edits the "other" file, which surprises non-experts. Many macOS apps save via replace, which breaks the link anyway | APFS clones: copy-on-write gives the hardlink space saving without the shared-mutation surprise. |
| "Learning" smart selection | Gemini markets "learns the way you select" | Opaque and non-deterministic, so the same scan can give a different plan tomorrow. Impossible to test or explain | Deterministic keep strategies + protected paths + "why kept" reason. |
| Deleting the last copy "with confirmation" | Gemini "Remove Last Instance" | Directly violates the Core Value | Not expressible anywhere. |
| Dedup inside Photos / Music / app bundles | Big wasted space lives there | Corrupts libraries. Gemini had to route through a "Gemini Duplicates" album | Skip bundles and report them as skipped. Point users to Photos' own duplicate merge. |
| Duplicate *folder* detection and folder merge | Gemini and Nektony (Pro) offer it | A different algorithm (tree hashing) with complex merge semantics, and it creates a large new deletion surface | Defer (v2+). File-level dedup covers the space. |
| Move-to-quarantine-folder mode | Gemini, Czkawka, Nektony and fclones `move` offer it | It overlaps with Trash, adds a 4th delete mode to test and journal, and the quarantine folder becomes a new duplicate source on rescans | Trash (with journal undo) covers recovery. Reconsider only for volumes without a Trash. |
| Gamification (achievements, ranks) | Gemini has it | Noise. It rewards deleting more, which is the wrong incentive for a safety-first tool | A plain "space reclaimed" history. |
| In-app TUI | Go v0 had a bubbletea browser | Already out of scope. Two interactive UIs double the selection-logic surface | The app is interactive, and the CLI stays scriptable (`--yes`, `--json`). |

---

### B. Automation / workflow builder: Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Workflow = scope + trigger + outcome (+ keep strategy) saved as a human-readable file | Hazel rules are per-folder condition/action sets, and n8n workflows are portable JSON | MEDIUM | TOML or JSON with a `schema_version`. The same file is run by the app, the CLI (`twins run <name>`) and the agent. Needs `twins workflow list | validate | show` alongside `run`. |
| **Run now** (manual trigger) on every workflow, including scheduled ones | n8n "Execute workflow", Hazel "Run rules now" | LOW | The CLI `twins run` is the same code path. |
| **Preview / test run** before enabling: shows exactly what an auto-clean *would* trash | Hazel rule preview shows which files match and why. n8n manual executions are ad-hoc and never production | MEDIUM | It is the dry-run plan rendered in the group browser. Recommend making "a successful preview exists" a precondition for switching a workflow to auto-clean. |
| Schedule trigger with human presets (hourly / daily at HH:MM / weekly on day) | Shortcuts Time of Day, Hazel's periodic re-scan. Users don't write cron | MEDIUM | Back it with launchd `StartCalendarInterval`, which coalesces runs missed during sleep into one run on wake. Document that behaviour. Advanced cron is optional. |
| Folder-change trigger with **settle/debounce delay** and partial-download ignore | Shortcuts Tahoe folder automations (Added/Modified/Removed, Ignore Subfolders). Gemini Duplicates Monitor watches cleaned folders | HIGH | FSEvents fires storms. Wait for quiet (e.g. 30–60 s), ignore `.download`, `.crdownload`, `.part` and in-progress files, and **ignore events caused by twins' own trash operations** (otherwise you get a self-trigger loop). Coalesce to one run per workflow. |
| Outcome: report-only → macOS notification with summary; auto-clean → Trash + journal using the keep strategy | Gemini Duplicates Monitor notifies with Review / Remove / Ignore. Shortcuts "Notify When Run", "Run after confirmation" | MEDIUM | Report-only is the default (PROJECT.md). Notifications must be **actionable**: "Review" opens the app on that run's results. Optionally a "Clean now" action, which is a human-confirmed clean, not automation. |
| Run history per workflow: status, trigger, duration, groups found, bytes reclaimable/trashed, errors, link to journal entries | n8n Executions list per workflow, Hazel's log. Without it, users can't trust an automation | MEDIUM | Store compact run records (a JSON-lines file or SQLite next to the cache). The journal remains the source of truth for file operations. The run id keys both and enables "undo this run". |
| Enable/disable per workflow + a global "pause all automation" switch | Hazel per-folder pause, n8n active toggle | LOW | The global kill switch should be reachable from a notification or menu bar in one click. |
| Validation with inline errors; refuse to save or enable an invalid workflow | Hazel 6 highlights problematic conditions/actions on save | LOW–MEDIUM | Errors include: scope empty, scope inside protected system paths, auto-clean with permanent mode (impossible by schema), schedule invalid, folder trigger on a network volume. |
| Runs without the app open (background agent) | Hazel runs as a background helper, and Gemini's Monitor is a menu-bar agent | HIGH | A LaunchAgent / `SMAppService` login item. It needs its own Full Disk Access consideration and must use the same core deletion routine. |
| Single-run concurrency guard | n8n queueing, plus the danger of two workflows trashing over the same tree | LOW–MEDIUM | One run per workflow at a time. A global lock (or per-path lock) for auto-clean runs so overlapping scopes can't race. Skip-and-log rather than queue unbounded. |

### B. Automation: Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Graph model with a list view over the same model** | n8n-level clarity of "what triggers what" plus Hazel-level simplicity, and nobody in the dedup space has either | HIGH | **Be opinionated: constrain the graph.** The real topology is a fixed pipeline: Trigger(s) → Scope → Filters → Find duplicates → Keep strategy → Outcome (+ Notify). Allow multiple triggers merging and multiple filters, and nothing else: no branches, no loops, no per-file routing (out of scope per PROJECT.md). Then the list view can render 100% of valid graphs, and two-way sync is a projection rather than a merge problem. Build the list view first (it is the MVP), then the canvas over the same model. |
| Auto-clean circuit breaker: per-run caps on files/bytes, downgrading to report-only when exceeded | No competitor has this. It protects against a mis-scoped workflow (e.g. scope accidentally `~`) | LOW | Defaults such as "max 500 files or 5 GB per run". If exceeded, nothing is trashed: notify "Run X found 12 000 files; review manually". Fits the Core Value directly. |
| Actionable notifications with deep link to the run | Gemini's Monitor does Review/Remove/Ignore. Shortcuts only has "run after confirmation" | MEDIUM | The `UNUserNotificationCenter` category has actions. A Tauri deep link (`twins://run/<id>`) opens results. |
| "Undo this run" from history | Hazel 6 added undo via Finder for rule changes. Gemini only undoes within a session | LOW (given journal undo) | Same mechanism as cleaning-flow undo, scoped by run id. |
| Menu bar status item: last run, next scheduled run, pause-all | Gemini Duplicates Monitor lives in the menu bar | MEDIUM | Optional. The agent can own it, which keeps the main window closed. |
| Incremental folder-change runs (check only new/changed files against an index of the scope) | Gemini Monitor reacts to "new copy detected" rather than re-scanning everything | HIGH | With the hash cache, a full rescan of a modest scope is cheap, so start with a full rescan + cache and optimize later. |

### B. Automation: Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Unattended **permanent** delete | "Trash fills up" | Violates the Core Value. Also, Hazel users often auto-empty the Trash after N days, so even Trash is weaker than it looks | Trash only (already in Out of Scope). Warn in the docs about Hazel/Finder auto-empty settings. |
| Per-file action/keep rules inside workflows | Power users want "PDFs keep newest, images keep oldest" | Already out of scope. It multiplies the safety surface and breaks the graph↔list projection | Use separate workflows with different scopes and keep strategies. |
| Free-form n8n canvas (arbitrary edges, branches, merges, loops) | "Like n8n" | Most graphs would be invalid or meaningless for dedup. Two-view sync becomes the dominant cost. Validation explodes | A constrained typed-port graph (above). |
| Script/shell/HTTP/webhook nodes, expression language | n8n and Hazel have "run script" | Turns twins into a general automation runtime with code execution in a Full-Disk-Access process, which is a security hole | Outcome emits JSON (report schema v1). Users chain it via Shortcuts/Hazel/cron calling `twins run --json`. |
| Hazel-style actions (rename, move, tag, sort into folders) | "While you're in there…" | Scope creep into a Hazel clone, with each action a new data-mutation path | Stay dedup-only. Interoperate via CLI and JSON. |
| Workflows triggering other workflows / chaining | n8n sub-workflows | No use case in dedup. Creates hidden cascades | Not supported. |
| Cron-string-only scheduling | Easy to implement | Hostile UX for a desktop app | Presets in the UI. Raw cron is accepted in the file for power users (optional). |
| Volume-mounted trigger | Natural for external drives | Out of scope for v1 (PROJECT.md) | Manual run on the volume. Revisit later. |
| Cloud sync / sharing of workflows, AI-suggested rules | Trendy | No user need for a personal tool, plus privacy and network surface | Files on disk. Users can put them in dotfiles. |

---

### C. Config, cache, distribution

| Feature | Category | Complexity | Notes |
|---------|----------|------------|-------|
| `config.toml` with flags overriding; `twins config show/path/init/clear-cache` | Table stakes (Go v0 parity) | LOW | Keys in v0: `min_size, exclude, keep, keep_dir, delete_mode, include_library, include_node_modules, include_empty, jobs`. Add `protected_paths` if adopted. The app's settings screen must read and write the *same* file. |
| Persistent hash cache, so warm rescans are near-instant | Table stakes (Czkawka, fclones, dupeGuru, Go v0: 0.15 s warm) | MEDIUM | Key on **volume UUID** + inode + size + mtime (+ ctime), not raw `dev`: `st_dev` is not stable across remounts of external volumes. Evict stale entries for missing files lazily. Czkawka warns that eager eviction thrashes removable-drive caches. Keep the partial-hash cache too (Czkawka "prehash cache"). |
| Cancel keeps hashes computed so far | Differentiator (Czkawka does it) | LOW | Write to the cache incrementally, not at scan end. |
| Signed + notarized app via Homebrew cask, CLI included | Table stakes for distribution | MEDIUM | The cask installs the app and symlinks `twins` (CLI) from the bundle. |
| In-app auto-updater | **Anti-feature for v1** | — | Conflicts with Homebrew cask ownership (`brew upgrade` vs self-update). Use cask updates only. If ever added, mark the cask `auto_updates true`. |

---

## Feature Dependencies

```
[Core deletion routine: plan → validate keep-one → live re-check → execute → journal]
    ├──requires──> [Keep strategies (oldest/newest/shortest-path/in-dir) + "why kept" reason]
    ├──requires──> [Protected paths]  (recommended; shrinks deletion set before validation)
    ├──requires──> [Trash via NSFileManager (resultingItemURL)]  ──enables──> [Journal-based undo]
    ├──requires──> [Journal schema with run_id + trash URL + hash]  ──enables──> [Undo, run history]
    └──requires──> [Dry-run = same plan, no execute]  ──enables──> [App preview, workflow test run]

[Hash cache (volume-UUID keyed)] ──enhances──> [Scan speed] ──enables──> [Folder-change trigger at acceptable cost]
[Hash cache] ──conflicts──> [Unverified auto-clean]  (cache hit ≠ proof; auto-clean re-verifies the deletion set)

[TOML config] ──requires──> [Config schema shared by CLI + app settings + workflow defaults]

[App: scan progress events] ──requires──> [Core progress/cancel API]
[App: group browser + selection] ──requires──> [Keep strategies, core plan validation]
[App: confirm + execute] ──requires──> [Core deletion routine]  (no app-side deletion logic)

[Workflow file format + runner (`twins run`)] ──requires──> [Core deletion routine, config, cache]
    ├──> [List-view editor]  ──requires──> [Workflow model]
    ├──> [Graph canvas]      ──requires──> [Workflow model constrained to list-representable graphs]
    ├──> [Schedule trigger]  ──requires──> [Background agent (launchd / SMAppService)]
    ├──> [Folder trigger]    ──requires──> [Background agent + FSEvents debounce + self-event suppression]
    ├──> [Run history]       ──requires──> [Journal run_id]
    └──> [Notifications]     ──requires──> [App deep links to a run's results]

[APFS clone mode] ──requires──> [Clone-ID detection]  (otherwise rescans re-report cloned files forever)
```

### Dependency Notes

- **Journal schema must be decided in Step 2, not Step 4/Workflows.** Undo, run history and "undo this run" all depend on each journal line carrying `run_id`, original path, `resultingItemURL`, size and hash. Retrofitting means old journals can't be undone.
- **Live re-check belongs in the core deletion routine.** The app (long dwell) and workflows (unattended) both need it. Putting it in only one shell breaks the "one routine" constraint.
- **The graph canvas depends on the list view's model, not the other way round.** If the model allows only list-representable graphs, the canvas is a rendering of the list and sync is trivial. Ship list view, runner and CLI first, then the canvas.
- **The folder-change trigger depends on the cache.** Without warm hashes, each FSEvents burst triggers a full re-hash of the scope.
- **The background agent is a hard prerequisite for schedule and folder triggers** (PROJECT.md: "run without the app open"). Its permissions model (Full Disk Access for a helper) is the riskiest dependency. Flag it for phase research.
- **Clone mode conflicts with naive duplicate counting:** shipping `--link` without clone-ID awareness makes every later scan, report and notification overstate reclaimable space, which matters most for report-only workflows that notify on every run.

---

## MVP Definition

### Launch With (v1)

Cleaning (CLI + app):
- [ ] Core deletion routine with keep-one validation, **live re-check of keeper and members**, journal (with run_id + trash URL) — Core Value
- [ ] Keep strategies oldest/newest/shortest-path/in-dir with "why kept" reason — parity + trust
- [ ] Trash (Put Back works, fail-closed), permanent (typed / `--force`), APFS clone (metadata-preserving, atomic rename) — parity
- [ ] `--dry-run`, `twins clean` (`--yes`), JSON output — parity
- [ ] Hash cache (volume-UUID keyed) + `config.toml` + `twins config …` — parity and the benchmark target
- [ ] App: locations + FDA onboarding → progress/cancel → summary → virtualized group browser (sort, filter, Quick Look, reveal) → keeper-based selection with auto-select and folder context actions → confirmation sheet → execution with error list and completion summary
- [ ] Protected paths — closes the `in-dir` safety gap cheaply

Automation:
- [ ] Workflow file format + `twins run` / `workflow validate|list`
- [ ] List-view editor; manual run; preview/test run
- [ ] Schedule + folder-change triggers via background agent (debounce, partial-download ignore, self-event suppression)
- [ ] Report-only (default) with actionable notification; auto-clean (Trash only) with circuit-breaker caps and re-verification of the deletion set
- [ ] Run history; enable/disable; global pause

Release:
- [ ] Signed/notarized app + CLI via Homebrew cask, semantic-release channels

### Add After Validation (v1.x)

- [ ] Graph canvas over the workflow model — after the list view proves the model (the user wants it, but it is pure presentation once the model is constrained)
- [ ] Journal-based **undo** UI (`twins undo`, "Put back this run"), if not squeezed into v1; the data is captured from day one
- [ ] APFS clone-ID awareness in accounting, required before or with heavy clone-mode use
- [ ] Menu bar status item
- [ ] Finder Quick Action "Find duplicates in…"
- [ ] Incremental folder-change runs (only if full rescan + cache proves too slow)

### Future Consideration (v2+)

- [ ] Duplicate folder detection/merge — new algorithm and new deletion surface
- [ ] Volume-mounted trigger — explicitly out of scope for v1
- [ ] Report-only "similar images" mode — only if exact dedup is solid and there is demand
- [ ] Remove-empty-folders-after-clean option (Gemini has it), low value

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Core deletion routine + keep-one + journal | HIGH | MEDIUM | P1 |
| Live re-check before acting (keeper exists, members unchanged) | HIGH | LOW | P1 |
| Trash with Put Back, fail-closed | HIGH | MEDIUM | P1 |
| Keep strategies + why-kept reason | HIGH | LOW | P1 |
| Protected paths | HIGH | LOW | P1 |
| Dry-run / preview | HIGH | LOW | P1 |
| APFS clone mode (metadata-preserving) | MEDIUM | MEDIUM | P1 (parity) |
| Hash cache + config | HIGH | MEDIUM | P1 |
| App scan/progress/summary | HIGH | MEDIUM | P1 |
| App group browser (virtualized, sort/filter/QL/reveal) | HIGH | MEDIUM | P1 |
| App keeper-based selection + confirm + execute | HIGH | MEDIUM | P1 |
| FDA onboarding + skipped-items transparency | MEDIUM | LOW | P1 |
| Workflow file + runner + CLI | HIGH | MEDIUM | P1 |
| List-view editor + test run | HIGH | MEDIUM | P1 |
| Background agent + schedule trigger | HIGH | HIGH | P1 |
| Folder-change trigger (debounced, loop-safe) | MEDIUM | HIGH | P1 |
| Actionable notifications + run history | HIGH | MEDIUM | P1 |
| Auto-clean circuit breaker + deletion-set re-verify | HIGH | LOW–MEDIUM | P1 |
| Graph canvas | MEDIUM | HIGH | P2 |
| Journal undo UI | HIGH | MEDIUM | P2 (data in P1) |
| Clone-ID-aware accounting | MEDIUM | MEDIUM | P2 |
| Menu bar item, Finder Quick Action | LOW–MEDIUM | MEDIUM | P3 |
| Folder dedup, similar images, volume trigger | LOW | HIGH | P3 |

**Priority key:** P1 must have for this milestone · P2 should have · P3 future

---

## Competitor Feature Analysis

| Feature | Gemini 2 (MacPaw) | Czkawka / Krokiet | dupeGuru | fclones | jdupes | Hazel / Shortcuts | **twins approach** |
|---------|-------------------|-------------------|----------|---------|--------|-------------------|--------------------|
| Match method | Exact + "similar" | name/size/hash (Blake3) + similar media | Contents / words / picture blocks | Exact (hash stages) | Exact (hash + byte) | n/a (Hazel "duplicate" = name suffix) | Exact: size → inode → partial → BLAKE3, optional byte verify |
| Keeper selection | Smart (learned), Newest/Oldest/Any, Always/Never Select folders | Select popup presets, Ref paths | Re-Prioritize ordered criteria, Reference folders | `--priority`, `--keep-path`, `-n` | `-O` param order, `-I` isolate | — | 4 fixed strategies + protected paths + why-kept reason |
| Last copy | Warns, allows ("Remove Last Instance") | Ref paths protected; otherwise user-selectable | Reference never deleted | Always keeps ≥ `-n` | Keeps one | — | Impossible, enforced in core |
| Removal modes | Trash / move to folder / permanent, optional hardlink | Trash / delete / move / hardlink / symlink | Trash / delete, replace with hard- or symlink | remove / move / link (hard/soft) / dedupe (reflink) | delete / link / `-B` clonefile (must pass twice) | Hazel: move / trash / delete | Trash (default, fail-closed) / permanent (typed) / APFS clone |
| Stale-plan protection | Not documented | Not documented | Not documented | Skips group if modified after report; drops size-changed | — | — | Re-stat all members + keeper before each group |
| Undo | Put Back within current session only | OS Trash | OS Trash | — | — | Hazel 6: undo rule changes via Finder | Journal records trash URL, so `undo` works across restarts |
| Cache | Yes | Shared hash + prehash cache, partial results kept on stop | Yes | Optional persistent | — | — | Volume-UUID keyed; incremental writes |
| Monitoring / automation | Duplicates Monitor: watches cleaned folders, notification with Review/Remove/Ignore | — (CLI for scripting) | — | CLI pipelines | CLI | Hazel: per-folder ordered rules, first-match, run-once-per-file, preview. Shortcuts (Tahoe): folder Added/Modified/Removed, run immediately or after confirmation | Workflows: scope + trigger (manual / schedule / folder) + report-only or Trash auto-clean, with preview, history, caps |
| Builder UI | Preferences tabs | Presets | Preferences | Flags | Flags | Hazel: condition/action list. n8n: node canvas, executions list, pinned test data | Constrained graph + list view over one model |

---

## D. Go v0 Parity Checklist (from `9024929:README.md`)

Use this to write explicit parity requirements:

- Commands: `twins scan`, `twins clean`, `twins report`, `twins config init|show|path|clear-cache`. Bare `twins` was an interactive menu, so decide what it does now (print help, or open the app). The TUI itself is out of scope.
- `clean` flags: `--dry-run`, `--yes`, `--keep oldest|newest|shortest-path|in-dir`, `--keep-in DIR`, `--link`, `--permanent` (type "permanent"), `--force`.
- Keep semantics: oldest = earliest mtime, then shallowest path. `in-dir` falls back to oldest. Hardlinks of the kept file are never removed.
- Safety: Trash with Put Back. At least one copy per group. Plan validated before run. Protected system paths. Dry-run journals. Journal at `~/Library/Logs/twins/operations.log` (JSON lines).
- Cache at `~/Library/Application Support/twins/cache.db`, keyed by inode, size and mtime. **Recommend volume UUID in the key** (deliberate deviation, documented).
- Config keys: `min_size` (human sizes like "1MiB"), `exclude`, `keep`, `keep_dir`, `delete_mode`, `include_library`, `include_node_modules`, `include_empty`, `jobs` (0 = CPUs, max 8). Flags override the file.
- TUI behaviours that map to app features: expand group, mark/unmark file, toggle whole group, auto-select by `--keep`, unmark all, path filter, reveal in Finder, Quick Look, `★` keeper / `✗` removal markers, "can never mark the last copy".
- Benchmark: 16 000-file Downloads, 1.5 s cold / 0.15 s warm.
- Scripting: `twins report … | jq '.summary'`, `.groups[].remove[]`. Keep these JSON paths stable in schema v1.

---

## Sources

- fclones README and `fclones/src/dedupe.rs` (pre-removal size/mtime checks, rename-to-temp link pattern): https://github.com/pkolaczk/fclones (MEDIUM, primary source)
- Czkawka/Krokiet README, `instructions/Instruction.md`, `instructions/Instruction_Krokiet.md` (reference paths, select presets, Trash/Delete/Move/Hardlink/Softlink, prehash cache, cache on stop): https://github.com/qarmin/czkawka (MEDIUM, primary)
- Gemini 2 User Guide (Smart Selection, Always/Never Select, Remove Last Instance, removal modes, Put Back limits, Duplicates Monitor): https://cdn3.macpaw.com/manuals/Gemin-2-User-Guide-29062021.pdf (MEDIUM, primary but dated 2021)
- Nektony comparison (Duplicate File Finder: Smart Select rules, merge folders, removal history): https://nektony.com/reviews/gemini-2-vs-duplicate-file-finder (LOW, vendor comparison)
- dupeGuru docs (Reference folders, Re-Prioritize, deletion options): https://dupeguru.voltaicideas.net/help/en/results.html, https://dupeguru.voltaicideas.net/help/en/reprioritize.html (MEDIUM)
- jdupes man page (`-B` dedupe via clonefile on APFS, must be given twice): https://manpages.debian.org/testing/jdupes/jdupes.1.en.html (MEDIUM)
- Hazel manual (rule logic: first match, run-once-per-file; Hazel 6 what's new: preview, undo, error highlighting): https://www.noodlesoft.com/manual/hazel/work-with-folders-rules/create-edit-rules/understand-the-logic-of-rules/, https://www.noodlesoft.com/manual/hazel/whats-new-in-hazel/ (MEDIUM)
- macOS Tahoe Shortcuts folder automation: https://sixcolors.com/post/2025/08/get-started-with-folder-automation-in-macos-tahoe/, https://support.apple.com/en-us/125148 (MEDIUM)
- n8n executions, manual/partial executions, pinned data, debug past executions: https://docs.n8n.io/workflows/executions/, https://docs.n8n.io/workflows/executions/manual-partial-and-production-executions/ (MEDIUM)
- APFS clone detection (`ATTR_CMNEXT_CLONEID`, `EF_MAY_SHARE_BLOCKS`): https://eclecticlight.co/2021/04/02/how-can-you-tell-whether-a-file-has-been-cloned-in-apfs/, https://github.com/cheapsteak/duh (MEDIUM, secondary; verify against `<sys/attr.h>` during phase research)
- Go v0 README at commit `9024929` (parity baseline): local git history (HIGH)

---
*Feature research for: macOS duplicate-file cleaner with workflow automation*
*Researched: 2026-10-02*

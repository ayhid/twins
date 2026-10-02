# Architecture Research

**Domain:** macOS duplicate-file finder. One Rust core shared by a CLI, a Tauri v2 desktop app and a background workflow runner.
**Researched:** 2026-10-02
**Confidence:** MEDIUM-HIGH overall. Crate boundaries, the deletion path, IPC and locking are HIGH: they come from the existing code and from Tauri, Rust std and SQLite behaviour that is documented and verified. Background-agent packaging and Full Disk Access inheritance are LOW-MEDIUM and need a spike (see "Open questions").

This document is about integration. It covers how the target features (safe deletion, hash cache and config, the Tauri app, workflows, the background agent) attach to the existing `twins-core` / `twins-cli` code at commit `c962eec`.

---

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              SHELLS (thin, no logic)                          │
├────────────────────┬──────────────────────────────┬──────────────────────────┤
│  twins-cli         │  twins-app (Tauri v2)        │  twins-agent             │
│  `twins` binary    │  Rust host + Svelte webview  │  headless LaunchAgent    │
│  scan/report/clean │  commands + Channel progress │  scheduler + FSEvents    │
│  config/run        │  session store, cancel map   │  serial run queue        │
│  terminal observer │  SMAppService register/status│  user notifications      │
└─────────┬──────────┴──────────────┬───────────────┴────────────┬─────────────┘
          │                         │                            │
          │         ┌───────────────┴────────────────┐           │
          ├────────►│  twins-workflow                │◄──────────┤
          │         │  graph model, TOML store,      │           │
          │         │  compile graph → RunSpec,      │           │
          │         │  list projection, runner,      │           │
          │         │  trigger types + next-fire     │           │
          │         └───────────────┬────────────────┘           │
          │                         │                            │
          ▼                         ▼                            ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                                 twins-core                                    │
├──────────────────────────────────────────────────────────────────────────────┤
│  observe   CancelToken, Event, Observer, Throttle                             │
│  pipeline  ScanSpec → walk → find → Groups  (moved out of twins-cli/run.rs)   │
│  plan      Selection → Plan → ValidatedPlan  (keep-one invariant lives here)  │
│  exec      execute(ValidatedPlan): lock, re-stat, backend, journal            │
│            backends: Trash (NSFileManager) | Permanent | Clone | DryRun       │
│  journal   write-ahead JSONL, run ids, trashed-to URLs                        │
│  cache     CachingHasher (impl Hasher) over SQLite WAL                        │
│  config    TOML, layered defaults < file < flags                              │
│  lock      named flock locks;  paths  app-support/logs/locks dirs             │
│  existing  scan/ group/ hash report safety fsutil human                      │
└──────────────────────────────────────────────────────────────────────────────┘
          │                         │                            │
          ▼                         ▼                            ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                       ON-DISK STATE (shared contract)                         │
│  ~/Library/Application Support/twins/                                         │
│     config.toml   workflows/<id>.toml   cache.db (WAL)   state.db (WAL)       │
│     locks/execute.lock   locks/workflow-<id>.lock                             │
│  ~/Library/Logs/twins/operations.log  (journal, JSON lines)                   │
└──────────────────────────────────────────────────────────────────────────────┘
```

The three shells never talk to each other over a socket. They coordinate through the on-disk contract: files, SQLite in WAL mode and `flock` locks. The one runtime link is the app registering or unregistering the agent with `SMAppService`, and the app watching the data directory to see what the agent did.

### Component Responsibilities

| Component | Owns | Does NOT own | Implementation |
|-----------|------|--------------|----------------|
| `twins-core::observe` | `CancelToken`, `Event` enum, `Observer` trait, rate-limiting adapter | Rendering progress | `Arc<AtomicBool>` newtype. Sync trait object. No async runtime. |
| `twins-core::pipeline` | Orchestrating walk → index → find → groups for a `ScanSpec` | Output formatting | The logic that sits in `twins-cli/src/run.rs` today, moved down. |
| `twins-core::plan` | Turning a selection or keep strategy into a `ValidatedPlan`. Every invariant check. | Executing anything | A newtype with a private constructor, so it can only be built by `validate()`. |
| `twins-core::exec` | The single deletion routine: execute lock, re-stat of snapshots, calls to backends, journal writes | Prompts and confirmations (those belong to the shell) | Takes `ValidatedPlan` and nothing else. Backends sit behind a trait. |
| `twins-core::journal` | Append-only write-ahead record of intent and outcome, plus run ids | Undo UI | JSON lines, one `write` per line, `flock` while appending |
| `twins-core::cache` | Persistent partial and full digests keyed by file identity and stat | Grouping logic | A `CachingHasher` that wraps the existing `Hasher` trait. `rusqlite` with WAL. |
| `twins-core::config` | Loading, merging and serialising `config.toml` | CLI flag parsing | `serde` plus the `toml` crate. Layered merge. |
| `twins-workflow` | Workflow file format, graph validation, compiling graph → `RunSpec`, list-view projection and edits, the runner, pure trigger maths (next fire time) | Watching files, timers, notifications, Tauri | A pure sync library. Unit-testable without macOS services. |
| `twins-cli` | Argument parsing, terminal progress, typed confirmations, `twins run <wf>` | Any decision about what to delete | `clap`. A terminal `Observer`. |
| `twins-app` | Webview IPC, scan sessions held in Rust, cancel map, paged group queries, agent registration UI, the builder's backend | Trigger evaluation (the agent owns it) | Tauri v2 commands plus `ipc::Channel<Event>` |
| `twins-agent` | Evaluating schedule and folder-change triggers, running workflows serially, notifying the user | Workflow semantics (it calls `twins-workflow`) | A resident launchd agent registered via `SMAppService.agent(plistName:)` with `ProcessType=Background` |

---

## Recommended Project Structure

```
Cargo.toml                         # workspace members: core, workflow, cli, agent, app
crates/
├── twins-core/src/
│   ├── lib.rs
│   ├── scan/ group/ hash.rs report.rs safety.rs fsutil.rs human.rs   # existing
│   ├── observe.rs        # CancelToken, Event, Observer, Throttle<O>
│   ├── pipeline.rs       # ScanSpec → ScanOutcome (moved from twins-cli/run.rs)
│   ├── plan/
│   │   ├── mod.rs        # Selection, Plan, GroupPlan (keep set ≥1, remove set)
│   │   └── validate.rs   # invariants → ValidatedPlan | PlanError
│   ├── exec/
│   │   ├── mod.rs        # execute(), Mode, Origin, ExecOutcome
│   │   ├── revalidate.rs # re-stat (dev, ino, size, mtime, ctime) vs snapshot
│   │   └── backend/      # trash.rs (objc2-foundation), clone.rs (clonefile),
│   │                     # permanent.rs, dry_run.rs. The only unsafe code lives here.
│   ├── journal.rs        # JSONL write-ahead journal
│   ├── cache/            # CachingHasher, schema, migrations
│   ├── config.rs         # Config, layered merge, defaults
│   ├── lock.rs           # NamedLock (flock), try / blocking-with-timeout
│   └── paths.rs          # support dir, logs dir, locks dir, workflows dir
├── twins-workflow/src/
│   ├── model.rs          # Workflow, Node, NodeKind, Edge, schema version
│   ├── store.rs          # list/load/save/delete under workflows/, atomic write
│   ├── validate.rs       # graph grammar (ports, stage order, single pipeline)
│   ├── compile.rs        # Workflow → RunSpec {scan: ScanSpec, keep, outcome}
│   ├── list.rs           # graph → ordered rule list; list edits → graph edits
│   ├── trigger.rs        # Trigger enum, next_fire(after, tz), catch-up rule
│   ├── runner.rs         # run(RunSpec, ctx) → RunRecord (calls core)
│   └── history.rs        # RunRecord persistence in state.db
├── twins-cli/src/        # main.rs cli.rs + commands/{scan,report,clean,config,run}.rs
├── twins-agent/src/      # main.rs scheduler.rs watcher.rs queue.rs notify.rs
└── twins-app/
    ├── Cargo.toml  tauri.conf.json  build.rs
    ├── capabilities/default.json      # own commands only; no fs/shell plugins
    ├── launchd/<bundle-id>.agent.plist
    ├── src/  main.rs state.rs dto.rs agent.rs commands/{scan,groups,clean,workflows,agent}.rs
    └── ui/   # Svelte + Vite (static build → frontendDist)
```

Bundle layout produced by the Tauri bundler:

```
Twins.app/Contents/
├── MacOS/twins-app                 # main executable
├── MacOS/twins                     # CLI, via bundle.externalBin (cask exposes it with `binary`)
├── MacOS/twins-agent               # agent, via bundle.externalBin
└── Library/LaunchAgents/<bundle-id>.agent.plist   # via bundle.macOS.files
       BundleProgram = Contents/MacOS/twins-agent
       RunAtLoad = true, KeepAlive = true, ProcessType = Background
```

### Structure Rationale

- **The deletion path lives in `twins-core`, not in a separate crate.** The constraint "every deletion path goes through one core routine" is easiest to enforce when `exec::execute` only accepts `plan::ValidatedPlan` and the two share a crate. A private constructor is then a compile-time guarantee. Keep all `objc2`, `clonefile` and other `unsafe` code in `exec/backend/` and put `#[allow(unsafe_code)]` on that one module, so the workspace `unsafe_code = warn` lint still guards everything else.
- **`twins-workflow` is a separate crate.** It needs `toml`, a cron/time crate and graph code that the CLI's scan path does not. It must stay free of FSEvents, timers and Tauri so the CLI (`twins run`), the app (builder and "Run now") and the agent all link the same compiler and runner.
- **`twins-agent` is its own binary, not a `twins agent` subcommand.** It is the only consumer of `notify`, the scheduler and the notification API. A distinct executable also gets its own code-signing identifier and its own name in Login Items and the Full Disk Access list. If the agent were the CLI binary, granting FDA to "twins" would also grant it to every Terminal invocation's identity.
- **The cache lives in core behind the existing `Hasher` trait.** `group/hasher.rs` already says "so a persistent cache can be layered on top". The pipeline needs no changes: `pipeline` builds a `CachingHasher` when the config enables it.

---

## Architectural Patterns

### Pattern 1: Move orchestration into core, keep shells as observers

**What:** Today `twins-cli/src/run.rs` wires `scan::walk`, `Index`, `group::find`, `group::plan` and `report::build` together itself. That logic moves to `twins_core::pipeline`. Shells pass an `Observer` and a `CancelToken` and render events. Both `scan::Options::cancel` and `group::Options::cancel` already take `Arc<AtomicBool>`, so the token only wraps what exists.
**When:** First, before any new feature. The app and the runner both need exactly this pipeline.
**Trade-offs:** Slightly more API surface in core. Without this, three copies of the orchestration drift apart, including the `dry_run` meta bug the codebase map already flagged.

```rust
// twins-core/src/observe.rs
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed) }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
    pub(crate) fn flag(&self) -> Arc<AtomicBool> { Arc::clone(&self.0) }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Walk { files: u64, candidates: u64 },
    Stage { stage: Stage, done: u64, total: u64 },
    FileSkipped { path: PathBuf, reason: String },
    Planned { groups: u64, reclaimable: u64 },
    Exec { done: u64, total: u64 },
    Removed { path: PathBuf, mode: Mode, trashed_to: Option<PathBuf> },
    GroupSkipped { digest: String, reason: String },   // re-validation failed
    Finished { outcome: OutcomeKind },                  // Completed | Cancelled | Failed
}

pub trait Observer: Send + Sync { fn on_event(&self, e: &Event); }

// Throttle<O>: forwards Walk/Stage/Exec at most every N ms, everything else at once.
```

`Event` derives `Serialize`, so the Tauri shell can push it straight into a `Channel` and the CLI can print it. Core never depends on Tokio. Rayon pools stay as they are.

### Pattern 2: Validated-plan newtype (keep-one enforced by the type system)

**What:** A `Plan` is a set of `GroupPlan { group, keep: Vec<FileMeta> (≥1), remove: Vec<FileMeta> }`. Only `plan::validate(plan, &Policy) -> Result<ValidatedPlan, PlanError>` can produce the type `exec::execute` accepts. The keep set is a `Vec` rather than the single `keep` the current `Action` has, because the app lets users unmark more than one copy.
**Invariants checked in `validate`:**
1. Every group keeps at least one physical identity that no `remove` entry has. This is keep-one counted on `(dev, ino)`, not on paths.
2. `keep` and `remove` are disjoint, both are subsets of the group, and no path appears in two groups.
3. No `remove` entry is a hardlink of a kept file. This matches the existing `Keeper::choose` rule.
4. No path is protected (`safety::is_protected`), and every path is absolute.
5. The mode is allowed for the origin: `Origin::Workflow` with any mode other than `Trash` (or `DryRun`) is rejected. "Automation only ever trashes" therefore holds in core, not by shell convention.
6. `Clone` mode requires the keeper and the target to be on the same APFS volume (same `dev`).
**Trade-offs:** The app's selection has to be converted into a `Plan` on the Rust side. That is intended (see Anti-Pattern 2).

```rust
pub struct ValidatedPlan { inner: Plan, origin: Origin, mode: Mode } // no pub fields, no pub ctor
pub fn validate(plan: Plan, origin: Origin, mode: Mode) -> Result<ValidatedPlan, PlanError>;
pub fn execute(plan: ValidatedPlan, ctx: &ExecContext<'_>) -> Result<ExecOutcome, ExecError>;
```

### Pattern 3: Execute under a lock, re-validate every file just before acting

**What:** `execute` takes the global `locks/execute.lock` (blocking, with a timeout). It then writes a `run_begin` journal record and, for each group:
1. Re-stats each kept file and compares `(dev, ino, size, mtime, ctime)` with the snapshot. On any mismatch or a missing keeper, it skips the whole group (`GroupSkipped`).
2. Re-stats each `remove` file in the same way and skips the file on mismatch.
3. Writes an `intent` record, calls the backend, then writes an `outcome` record that includes `trashed_to` (the `resultingItemURL` from `NSFileManager.trashItemAtURL`).
4. Checks the `CancelToken` between groups, never in the middle of a file operation.

**Why this is the real concurrency guarantee:** Two runs (app plus agent, or two workflows with overlapping scopes) can each plan from their own scan. If run A trashes the file that run B chose as keeper, B's step-1 check fails and B skips that group. The lock makes "check, then act" atomic with respect to other twins processes. Re-stat cannot stop non-twins processes racing, but it shrinks the window to milliseconds.
**DryRun** goes through the same function with the `DryRun` backend. It journals `would_remove`, so "`--dry-run` journals and changes nothing" falls out naturally.
**Clone mode** must be atomic per file: `clonefile(keeper, dir/.twins-tmp-XXXX)`, copy the permissions, owner and xattrs that need preserving, `rename` over the target, and journal. Never unlink first.

### Pattern 4: Long-running Tauri commands = async command + `spawn_blocking` + `Channel` + cancel map

**What:** The frontend generates a `runId`, creates a `Channel<Event>` and invokes `start_scan({ runId, spec }, onEvent)`. The Rust command registers a `CancelToken` under `runId` in managed state, runs `pipeline::scan` inside `tauri::async_runtime::spawn_blocking`, stores the resulting `ScanSession` (groups held in Rust) under a `sessionId`, and returns a summary. `cancel_run(runId)` flips the token.
**Why `Channel` and not events:** The Tauri docs say the event system "is not designed for low latency or high throughput situations", that payloads are always JSON strings, and that async listeners may receive events out of order. Channels are "designed to be fast and deliver ordered data" and are what Tauri uses internally for download progress and child-process output. Keep global events (`app.emit`) for low-rate broadcast signals only, such as `workflows-changed` and `run-history-updated`.

```rust
#[tauri::command]
async fn start_scan(
    state: tauri::State<'_, AppState>,
    run_id: RunId,
    spec: ScanRequest,
    on_event: tauri::ipc::Channel<Event>,
) -> Result<ScanSummary, CmdError> {
    let cancel = state.runs.register(run_id);
    let spec = spec.into_core(&state.config()?)?;            // flags/config merge in Rust
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let obs = Throttle::new(ChannelObserver(on_event), Duration::from_millis(80));
        twins_core::pipeline::scan(&spec, &obs, &cancel)
    }).await??;
    state.runs.remove(run_id);
    Ok(state.sessions.insert(outcome))                        // returns sessionId + summary
}
```

**Group browsing is paged:** `query_groups(sessionId, offset, limit, filter, sort)` returns one page of DTOs. Never send all groups in one payload. Scans of a home folder can produce tens of thousands of groups. The webview uses a virtual list.

### Pattern 5: Graph is the source of truth, the list is a total projection of a constrained graph

**What:** A workflow is one pipeline expressed as a typed graph with fixed stage kinds:
`Trigger+ → Scope+ → Filter* → Keep(1) → Outcome+`. "Find duplicates" is implicit and not a node. Edges may only connect a stage to the same or the next stage kind (port typing). Validation rejects anything else: cycles, branches that would give different scopes to different outcomes, more than one keep. Because of that grammar, every valid graph has exactly one canonical ordered list, and every list edit maps to a deterministic graph edit with new nodes auto-laid-out. The list view is then never read-only and never lossy.
**Compile step:** `compile(&Workflow) -> RunSpec { scan: ScanSpec, keep: Strategy, outcome: Outcome, triggers: Vec<Trigger> }`. Scopes are unioned and filters merged into `scan::Options`. The runner and the agent only ever see the `RunSpec`, never the graph.
**Trade-offs:** This is less general than n8n. That is deliberate: PROJECT.md scopes workflows to scope, trigger and report-or-clean, and keeps keep/action logic out. If someone wants two different pipelines, they create two workflows.

File format: TOML, the same language as `config.toml`, hand-editable, with comments preserved where practical. UI layout lives in the same file under `ui` sub-tables, and the runner ignores it.

```toml
version = 1
id = "downloads-weekly"
name = "Downloads, weekly tidy"

[[nodes]]
id = "t1"; kind = "trigger.schedule"; every = "week"; at = "Mon 09:00"
ui = { x = 40, y = 80 }

[[nodes]]
id = "s1"; kind = "scope"; paths = ["~/Downloads"]; exclude = ["*.part"]; min_size = "1MiB"

[[nodes]]
id = "k1"; kind = "keep"; strategy = "oldest"

[[nodes]]
id = "o1"; kind = "outcome.clean"     # Trash only; outcome.report is the default

[[edges]]
from = "t1"; to = "s1"
# ...
```

Writes are atomic: write to `<id>.toml.tmp` in the same directory, `fsync`, then `rename`. The agent watches `workflows/` and may otherwise read a half-written file.

### Pattern 6: One trigger evaluator, the agent

**What:** Only `twins-agent` evaluates `schedule` and `folder-change` triggers. It does so even while the app is open. The app and CLI run workflows only on explicit user action ("Run now", `twins run`). The agent keeps a serial run queue (one run at a time, so scans do not fight over the disk), a scheduler (a min-heap of next fire times computed by `twins_workflow::trigger::next_fire`) and a `notify` FSEvents watcher per folder-change workflow, plus one on `workflows/` itself to hot-reload definitions.
**Why:** If the app also evaluated triggers, every scheduled run would fire twice whenever the app was open, and both runs would contend for the same execute lock.
**Catch-up rules (stored in `state.db`):** For schedules, record `last_fired` per workflow. After sleep or reboot, if one or more fire times were missed, run once (anacron-style), never N times. For folder changes, on agent start mark every folder-change workflow dirty and run each once after the debounce window. v1 uses `notify`, which starts streams at "since now". Upgrading to persistent FSEvents event IDs (`sinceWhen`) can come later.
**Debounce plus self-suppression:** Coalesce FSEvents with a quiet period of minutes, not seconds (downloads in progress keep writing). Ignore events for paths the agent's own run just moved to the Trash. Re-running is idempotent anyway (a second scan finds nothing), but the suppression avoids a pointless second scan after every auto-clean.

---

## Data Flow

### Interactive clean (app)

```
[Svelte: Scan button] --invoke start_scan(runId, spec, Channel)--> [twins-app command]
     ▲                                                         │ spawn_blocking
     │ Channel<Event> (throttled ~80 ms, ordered)              ▼
     └──────────────────────────────── core::pipeline::scan(spec, obs, cancel)
                                                               │
                                     ScanSession{groups} held in AppState, sessionId returned
[Svelte: group list] --query_groups(sessionId, page)--> page of GroupDto (ids, not authority)
[Svelte: selection]  --preview_plan(sessionId, selection{groupId → removeFileIds})-->
                       core::plan::validate → summary | PlanError (shown inline)
[Svelte: Confirm]    --execute_plan(sessionId, selection, mode, runId, Channel)-->
                       rebuild Plan from session (Rust-side FileMeta) → validate → exec::execute
                       → journal → Event::Removed/GroupSkipped → Channel → UI
```

The frontend only ever sends ids (group index, file index within the session). The Rust side turns them back into `FileMeta` snapshots it already holds. Paths from the webview are never used as deletion targets.

### CLI clean

```
twins clean PATH --keep newest [--dry-run|--permanent --force|--clone] [--yes]
  → config::load() merged with flags → ScanSpec
  → pipeline::scan(spec, TerminalObserver, ctrl-c CancelToken)
  → group::plan(strategy) → plan::validate(Origin::Cli, mode)
  → shell prompt (typed confirmation for permanent unless --force; y/N unless --yes)
  → exec::execute → journal → summary
```

### Workflow run (any host)

```
host (CLI `twins run X` | app "Run now" | agent trigger)
  → twins_workflow::store::load(X) → validate → compile → RunSpec
  → try_lock locks/workflow-<id>.lock  (held: record "skipped: already running", exit)
  → pipeline::scan(RunSpec.scan, host observer, cancel)
  → group::plan(RunSpec.keep)
  → outcome.report : RunRecord{groups, reclaimable} → state.db → host notifies
    outcome.clean  : plan::validate(Origin::Workflow{id}, Mode::Trash) → exec::execute
                     → RunRecord{removed, skipped, bytes, journal run_id} → state.db → notify
```

### Agent ↔ app coordination (no sockets)

```
App toggle "Background workflows"
  → SMAppService.agent(plistName: "<bundle-id>.agent.plist").register()
  → status: .enabled | .requiresApproval (→ SMAppService.openSystemSettingsLoginItems()) | .notFound
App edits workflow → atomic write to workflows/<id>.toml
  → agent's watcher on workflows/ reloads it → reschedules
Agent finishes run → RunRecord into state.db (WAL) → user notification
  → app (when open) watches state.db / runs dir → emits `run-history-updated` → UI refresh
```

### Key Data Flows

1. **Progress:** core `Event` → `Throttle` → shell adapter. Terminal: a single-line redraw on stderr. Tauri: `Channel<Event>`. Agent: no UI, it logs to `~/Library/Logs/twins/agent.log` and feeds the `RunRecord`.
2. **Cancellation:** shell-owned `CancelToken`. In the CLI it is the Ctrl-C handler, in the app the `cancel_run(runId)` command, in the agent a SIGTERM from launchd or a workflow being deleted mid-run. Core polls it between files in walk and hash and between groups in execute. Cancelling during execute stops cleanly and journals `run_end{cancelled}`. The keeper is never touched, so a partial run still satisfies keep-one.
3. **Hash cache:** `pipeline` wraps `DirectHasher` in `CachingHasher`. On lookup it looks for key `(dev, ino, size, mtime_ns, ctime_ns)`. On a hit it returns the stored partial or full digest. On a miss it computes the digest and buffers the row. At the end of the run it flushes the buffered rows in one transaction. Including `ctime` matters because `touch -m` can restore an old mtime after a content change, but nothing outside the kernel sets ctime.
4. **Config:** `defaults < config.toml < workflow node params < CLI flags`. One `config::resolve()` in core, so the CLI, the app and workflows agree on what `min_size` means.

---

## Concurrency and Locking

| Resource | Mechanism | Scope | Behaviour on contention |
|----------|-----------|-------|-------------------------|
| Deleting files (any mode) | `locks/execute.lock`, exclusive `flock` | Global, held only during `exec::execute` | Block with a timeout (for example 10 min). On timeout, fail the run with `ExecError::Busy`. |
| Same workflow overlapping itself | `locks/workflow-<id>.lock`, `try_lock` | Per workflow, held for the whole run | Skip, record `skipped: already running` |
| Scans | none | Read-only, may run concurrently | The agent's serial queue plus `ProcessType=Background` keeps it from hurting interactive scans |
| Hash cache | SQLite WAL, `busy_timeout` 5 s, batched write transaction | Multi-process | Readers never block. Writers serialise briefly. A failed flush is non-fatal: log it and continue without caching. |
| Run history / trigger state | `state.db`, SQLite WAL | Multi-process | Same as the cache |
| Journal | `O_APPEND` plus `flock` around each record write | Multi-process | Short critical section |
| Workflow files | atomic `rename` | Writer: app or user. Readers: all. | Readers never see partial files |

- **Use `flock` (std `File::lock` / `try_lock`), not pidfiles.** The OS releases a flock when the process dies, so a crashed agent never leaves a stale lock. These methods were stabilised in **Rust 1.89**. The workspace MSRV is 1.85, so either raise `rust-version` to 1.89 or use the `fs4` crate. Recommendation: raise the MSRV, since nothing external pins 1.85.
- **Do not use redb or sled for the cache.** redb is single-process: a second process opening the file gets `DatabaseAlreadyOpen`. The CLI, app and agent will open the cache at the same time, so SQLite in WAL mode (`rusqlite` with the `bundled` feature) is the right store.
- Keep `cache.db` (disposable, wiped by `twins config clear-cache`) separate from `state.db` (run history, last-fired times, which must not be wiped).

---

## Background Agent: launchd LaunchAgent vs Tauri autostart vs SMAppService

| Option | What it launches | Fits "runs without the app open"? | Verdict |
|--------|------------------|----------------------------------|---------|
| `tauri-plugin-autostart` (LaunchAgent or AppleScript) | The **whole app**, with its webview | Only if the app is always running. That is heavy, and closing the window must not quit it. | Reject. It solves "open at login", not "headless runner". |
| Hand-written plist in `~/Library/LaunchAgents` + `launchctl bootstrap` | Any binary | Yes | Legacy. It is a file outside the bundle that survives uninstall and points at a path that can move. macOS 13+ shows "background item added" alerts and lists it under the developer name anyway. |
| One plist per workflow (`StartCalendarInterval` / `WatchPaths`) | The runner per workflow | Yes | Reject. Bundled plists cannot change at runtime because they are signed. Generated ones multiply the legacy problems, and launchd's `WatchPaths` is coarse. |
| **`SMAppService.agent(plistName:)` with a bundled plist (macOS 13+)** | `twins-agent` in `Contents/MacOS` | Yes | **Use this.** The plist ships inside the signed bundle in `Contents/Library/LaunchAgents`. `BundleProgram` is relative to the bundle, so it follows the app when moved. It is removed with the app, appears under Login Items, and the app can query `status` and handle `requiresApproval`. |

Implementation notes:
- Rust bindings: `objc2-service-management` (part of the `objc2` family the trash backend already needs through `objc2-foundation`). Call `register`, `unregister` and `status` from the **app** only. `SMAppService` resolves the plist from the calling process's main bundle, so the CLI cannot be assumed to work for this.
- Plist keys: `Label`, `BundleProgram = Contents/MacOS/twins-agent`, `RunAtLoad = true`, `KeepAlive = true`, `ProcessType = Background` (throttles CPU and I/O). `AssociatedBundleIdentifiers` is optional.
- Ship it with `bundle.macOS.files = { "Library/LaunchAgents/<bundle-id>.agent.plist": "./launchd/agent.plist" }`. Ship the binaries with `bundle.externalBin` (built with the `-aarch64-apple-darwin` / `-x86_64-apple-darwin` suffix). The release pipeline must sign every executable with the hardened runtime before notarisation.
- The agent is idle most of the time: no rayon pools until a run starts, and one FSEvents stream per watched root.

**Full Disk Access (unresolved, LOW confidence):** TCC grants FDA to a responsible process. A process launched by launchd is its own responsible process. Since macOS 11.4 (the fix for CVE-2021-30713), helpers do **not** automatically inherit TCC grants from the main app. Some reports say `SMAppService`-registered components share the app's grant. Apple's documentation does not settle this. Design for both outcomes:
1. The agent probes access at start and before each run (for example, a stat of a TCC-protected path) and records a `needs_fda` status in `state.db`.
2. The app reads that status and walks the user through granting FDA to the right item.
3. The workflow runner reports "folder not readable" as a run failure, not as "no duplicates".

Also note that the CLI run from Terminal uses **Terminal's** FDA grant, not the app's. Document this.

---

## Scaling Considerations

The tool has one user, so the scale that matters is files and groups.

| Scale | Architecture adjustments |
|-------|--------------------------|
| ~16k files (Downloads benchmark) | Current design. A warm cache must reach about 0.15 s, so cache lookups must be batched (one prepared statement, no transaction per file). |
| ~1M files (whole home folder) | `FileMeta` per candidate stays in memory (fine). Groups must be paged to the webview. Cache writes are batched, and the database gets `VACUUM`/pruning of rows for missing inodes on `clear-cache` or periodically. |
| Many workflows (10s) | Serial agent queue. Overlapping scopes share the cache, so a second workflow over the same folder is mostly cache hits. |

### Scaling Priorities

1. **First bottleneck:** IPC payload size for groups in the app, fixed by paging and id-only DTOs. Next is cache write amplification, fixed by one transaction per run.
2. **Second bottleneck:** journal growth. Rotate `operations.log` by size and keep N files. Search for a run id across the rotated files.

---

## Anti-Patterns

### Anti-Pattern 1: Orchestration in the shells
**What people do:** Each shell wires walk → find → plan → report itself. `twins-cli/src/run.rs` does this today.
**Why it's wrong:** The app and the runner copy it and drift. The `dry_run: false` hardcode in `run.rs` is an early example.
**Do this instead:** `twins_core::pipeline` plus `Observer`. Shells only render events and ask for confirmation.

### Anti-Pattern 2: The webview as the authority on what to delete
**What people do:** Send full paths from the Svelte selection to an `execute(paths)` command.
**Why it's wrong:** A UI bug, stale state or an injected script can delete arbitrary files, and keep-one cannot be checked without the group context.
**Do this instead:** Send session-relative ids, rebuild the `Plan` in Rust from snapshots held there, and `validate` it. Grant the webview no `fs` or `shell` plugin capabilities. Every file operation goes through typed twins commands.

### Anti-Pattern 3: High-frequency progress over Tauri events, or unthrottled
**What people do:** `app.emit("progress", ..)` for every file.
**Why it's wrong:** Events are JSON strings and not built for throughput. Async listeners can reorder them, and the IPC floods.
**Do this instead:** A per-invocation `Channel<Event>` with a core-side `Throttle` at about 80–100 ms, plus a final unthrottled `Finished`.

### Anti-Pattern 4: Evaluating triggers in more than one process
**What people do:** The app runs schedules while open and the agent runs them while it is closed.
**Why it's wrong:** Runs fire twice, hand-off logic gets fragile, and the two compete for the execute lock.
**Do this instead:** The agent is the only trigger evaluator. The app only does manual runs and edits files.

### Anti-Pattern 5: Treating Finder "Put Back" as the undo mechanism
**What people do:** Rely on `trashItemAtURL` so users can undo through Finder.
**Why it's wrong:** Apple DTS confirmed in February 2025 a bug open for more than ten years: after rapid consecutive trashes, only the first item gets Put Back, especially with the Trash window open, because of `.DS_Store` contention. Batch cleans hit exactly this case.
**Do this instead:** Journal `original_path → trashed_to (resultingItemURL)` for every file. A later `twins undo <run-id>` can restore from the journal. Put Back is a bonus, not the contract.

### Anti-Pattern 6: Trusting the scan snapshot at execute time
**What people do:** Delete whatever the plan lists.
**Why it's wrong:** Another twins run, the user or a sync client may have moved, edited or deleted the keeper since the scan.
**Do this instead:** Under the execute lock, re-stat both the keepers and the targets against `(dev, ino, size, mtime, ctime)` and skip the group on any keeper mismatch.

### Anti-Pattern 7: An async runtime in core
**What people do:** Make `pipeline::scan` an `async fn` so Tauri can `.await` it.
**Why it's wrong:** The work is CPU and blocking I/O already parallelised with rayon. Tokio in core forces it on the CLI and agent and gains nothing.
**Do this instead:** A sync core. Tauri wraps calls in `spawn_blocking`. The agent uses plain threads plus channels, or a small runtime of its own if `notify` integration needs one.

### Anti-Pattern 8: Free-form workflow graphs
**What people do:** Allow any DAG, as n8n does, and then try to render it as a list.
**Why it's wrong:** The list view becomes lossy or read-only, and arbitrary graphs invite per-file keep/action logic, which is out of scope.
**Do this instead:** A typed stage grammar where every valid graph has one canonical list (Pattern 5).

### Anti-Pattern 9: Pidfile locks or lock files with `create_new`
**Why it's wrong:** A crash leaves a stale lock and the agent never runs again.
**Do this instead:** `flock` through std `File::try_lock`, which the kernel releases when the process exits.

---

## Integration Points

### External (macOS) Services

| Service | Integration | Notes |
|---------|-------------|-------|
| Trash | `NSFileManager.trashItemAtURL:resultingItemURL:error:` through `objc2-foundation` | Record `resultingItemURL` in the journal. Put Back is unreliable for batches (Anti-Pattern 5). Must run on the calling thread; no main-thread requirement is known, but verify in a spike. |
| APFS clone | `libc::clonefile` → temp file in the same directory → `rename` | Same volume only (validated). Preserve the target's mode, owner and xattrs as needed. |
| Background agent | `SMAppService.agent(plistName:)` through `objc2-service-management` | macOS 13+. User approval in Login Items. Called from the app. |
| Folder changes | `notify` 8.x (FSEvents backend) plus your own debounce, or `notify-debouncer-full` 0.7 | `notify` starts at "since now". Persistent event ids (`sinceWhen`) need direct FSEvents bindings (possible later upgrade). FSEvents can miss events on files the user does not own. |
| Notifications | Agent: user notifications for run results. App: in-window toasts. | The API depends on packaging. A bare executable in `Contents/MacOS` may not be able to use `UNUserNotificationCenter`. Spike this together with FDA. |
| TCC / Full Disk Access | Probe and report. Never assume. | See the Background Agent section. |
| Finder reveal / Quick Look | App commands (`NSWorkspace.activateFileViewerSelectingURLs`, Quick Look panel or `qlmanage -p`) | App-only. Keep them out of core. |
| launchd QoS | `ProcessType = Background` in the agent plist | Lowers CPU and I/O priority for unattended runs |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| shells ↔ `twins-core` | Direct Rust calls: `pipeline::scan`, `plan::validate`, `exec::execute`, `config::resolve` | The only way to delete is `exec::execute(ValidatedPlan)` |
| shells ↔ `twins-workflow` | Direct Rust calls: `store`, `compile`, `runner::run(RunSpec, RunContext)` | `RunContext` carries the origin, `CancelToken`, `Observer` and a `Notifier` trait implemented per host |
| `twins-workflow` → `twins-core` | Direct calls. The workflow crate builds `ScanSpec`, `Strategy` and `Origin::Workflow` | It cannot request permanent or clone mode, because core rejects them |
| Svelte ↔ Tauri host | Commands (request/response), `Channel<Event>` (per-call stream), global events (low-rate broadcast) | Generate TS types with `tauri-specta` (still RC, so pin exact versions) or `ts-rs`. Pin the version either way. |
| app ↔ agent | On-disk contract (workflow files, `state.db`, locks) plus `SMAppService` register/status | No socket in v1. If "run now via agent" is ever needed, add a Darwin `notify_post` signal or an XPC `MachServices` entry then. |
| CLI ↔ agent | On-disk contract only | `twins run` uses the same per-workflow lock, so it never overlaps a scheduled run |

---

## Suggested Build Order (dependency-driven)

1. **Core refactor: `observe` + `pipeline`.** Move orchestration out of `run.rs`, add `CancelToken` / `Event` / `Observer` / `Throttle`, and fix `meta.dry_run`. Everything below depends on it. Low risk, and existing CLI tests are the safety net.
2. **Safe deletion: `plan` (keep-set model plus `validate`), `exec` (re-stat, execute lock, backends), `journal`, `twins clean`.** Add the `lock` and `paths` modules here because the execute lock is part of the routine. Raise the MSRV to 1.89 for std file locks. Trash, clone and permanent backends plus DryRun. This is the riskiest code for the Core Value. Test it heavily with tempdirs, including racing-keeper tests.
3. **Cache and config: `cache` (SQLite WAL `CachingHasher`), `config` (layered TOML), `twins config ...`.** These plug into existing seams (`Hasher` trait, `scan::Options`) and do not depend on step 2. They come after it in the PROJECT order, but could run in parallel with it if needed.
4. **App shell: `twins-app` scan → browse → select → clean.** Session store, paged queries, `Channel` progress, cancel map, `preview_plan` / `execute_plan` through core. Bundle layout decisions (`externalBin`, `macOS.files`) are made here.
   - **Spike, at the end of step 4 and before step 6:** a signed and notarised bundle containing `twins-agent`, registered with `SMAppService`. Verify (a) the Login Items approval flow, (b) whether FDA granted to the app covers the agent, (c) whether the agent can post user notifications as a bare executable or needs a nested helper `.app`. The result may change the bundle layout, so learn it before the workflow phases build on it.
5. **Workflow model and runner: `twins-workflow` (format, grammar, compile, list projection, runner, history) and `twins run <wf>`.** Headless and CLI-first, so the semantics are tested without a UI. Per-workflow lock and `state.db`.
6. **Builder UI in the app.** Graph canvas (Svelte Flow / `@xyflow/svelte`) and list view over the same `twins-workflow` model. The Rust side validates. Files are written atomically.
7. **Background agent: `twins-agent`.** Scheduler with catch-up, FSEvents watcher with debounce and self-suppression, serial queue, notifications, FDA probe. App toggle and status through `SMAppService`.
8. **Release.** Sign every executable (app, CLI, agent) with the hardened runtime, notarise, and ship a cask that exposes `Contents/MacOS/twins` as a `binary`.

**Ordering rationale:** 1 unblocks 2, 4 and 5. Step 2 must exist before the app or workflows can act at all, because both call `exec::execute`. Step 5 comes before 6 and 7 because the builder and the agent are both shells over the workflow crate. The agent is last among features because it depends on the riskiest platform unknowns, which the spike at the end of step 4 resolves early.

**Phases likely to need deeper research:** step 2 (trash API behaviour and threading, clonefile metadata preservation), step 4 spike / step 7 (FDA, notifications, `SMAppService` edge cases on macOS 26), step 6 (Svelte Flow capabilities for constrained port typing).
**Standard patterns, little research needed:** steps 1, 3 and 5.

---

## Open Questions (carry into phase research)

- Does an `SMAppService` agent whose `BundleProgram` is in `Contents/MacOS` inherit the app's Full Disk Access grant on macOS 13–26? (LOW confidence. Sources conflict.)
- Can a bare `Contents/MacOS/twins-agent` post notifications through `UNUserNotificationCenter` attributed to the app, or does it need a nested `LSBackgroundOnly` helper `.app`? (Unverified.)
- Does `trashItemAtURL` need to run on the main thread or a specific queue when called from rayon or `spawn_blocking` threads? (Unverified. Test it in step 2.)
- Should the schedule syntax be cron or a friendlier "every / at" form? (That is a features question, but it affects `trigger.rs`.)

## Sources

- Tauri v2, Calling the Frontend (events vs channels, ordering, JSON payloads): https://v2.tauri.app/develop/calling-frontend/ (official, MEDIUM as fetched)
- Tauri v2, macOS Application Bundle (`bundle.macOS.files` → `Contents/`): https://v2.tauri.app/distribute/macos-application-bundle/
- Tauri v2, Sidecars / `externalBin` (target-triple suffix): https://v2.tauri.app/develop/sidecar/
- Tauri autostart plugin (launches the whole app; LaunchAgent or AppleScript): https://v2.tauri.app/plugin/autostart/
- Tauri `spawn_blocking`: https://docs.rs/tauri/latest/tauri/async_runtime/fn.spawn_blocking.html
- Apple, SMAppService: https://developer.apple.com/documentation/servicemanagement/smappservice
- Apple Developer Forums, launching an SMAppService agent (BundleProgram, behaves like a launchd agent): https://developer.apple.com/forums/thread/750528
- Apple Developer Forums, trashItem / recycle and Put Back (DTS, Feb 2025, r.23153124): https://developer.apple.com/forums/thread/773997
- Michael Tsai, macOS 11.4 breaks FDA inheritance for helper tools: https://mjtsai.com/blog/2021/06/01/macos-11-4-breaks-full-disk-access-for-helper-tools/
- Apple Developer Forums, The Rules for Full Disk Access: https://developer.apple.com/forums/thread/107546
- objc2-service-management: https://docs.rs/objc2-service-management/ ; smappservice-rs: https://github.com/gethopp/smappservice-rs
- Rust `File::lock` stabilisation (1.89): https://github.com/rust-lang/rust/pull/136794
- redb multi-process limitation: https://github.com/cberner/redb/issues/678 , https://github.com/cberner/redb/blob/master/docs/design.md
- notify / notify-debouncer-full: https://docs.rs/notify/latest/notify/ , https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/
- FSEvents persistent event ids (`sinceWhen`): https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html
- tauri-specta (v2 still RC; pin versions): https://github.com/specta-rs/tauri-specta
- Existing code: `crates/twins-core/src/group/{hasher.rs,find.rs,keep.rs}`, `crates/twins-core/src/scan/options.rs`, `crates/twins-cli/src/run.rs` (commit `c962eec`)

---
*Architecture research for: macOS duplicate-file finder (Rust core, CLI, Tauri app, background workflow agent)*
*Researched: 2026-10-02*

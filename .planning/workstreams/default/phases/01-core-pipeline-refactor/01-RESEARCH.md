# Phase 1: Core Pipeline Refactor - Research

**Researched:** 2026-10-02
**Domain:** Rust library refactor: moving scan orchestration into `twins-core`, a staged observer/cancel API, SIGINT handling in a CLI, MSRV bump
**Confidence:** HIGH (nearly everything is in-repo code read this session, or crate source read this session)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Carried forward from research (not re-discussed)
- **D-01:** Core API follows `.planning/research/ARCHITECTURE.md` Pattern 1:
  - a `twins_core::observe` module with `CancelToken` (wrapping `Arc<AtomicBool>`), a serializable `Event` enum, an `Observer` trait (`Send + Sync`) and a `Throttle<O>` wrapper;
  - a `twins_core::pipeline::scan(spec, &observer, &cancel)` entry point.

  Shells only render events. Core never depends on Tokio, and the rayon pools stay as they are. — **Reversibility:** costly — Phase 2 (clean), Phase 5 (Tauri `Channel<Event>`) and Phase 6/8 (runner, agent) all build on this API.

#### Ctrl+C / cancellation
- **D-02:** A cancelled scan exits with code **130** (128+SIGINT). It stays distinct from 1 (I/O) and 2 (usage).
- **D-03:** A **second Ctrl+C** while the first cancel is unwinding exits immediately with 130. This is the escape hatch if a read blocks on a stuck volume. It is safe because a scan writes nothing.
- **D-04:** On cancel the CLI clears the progress line and prints a single `scan cancelled` line on **stderr**. Nothing goes to stdout: no partial text report and no partial JSON.
- **D-05:** Cancellation must stop the walk and every hashing stage promptly, within about a second on a large tree. The existing `group::Options::cancel` / `FindError::Cancelled` and the walk's cancel option are driven by the one `CancelToken`.

#### Progress display
- **D-06:** Single-line stderr progress with a **step counter**, for example `[1/4] walk  48 210 files`, `[3/4] partial hash  1 234 / 5 000`. Numbers use the existing human formatting.
- **D-07:** **Size grouping is a real stage**: core emits its event (with the candidate count), so observers and tests see walk → size grouping → partial hash → full hash in order. The CLI may show it only briefly.
- **D-08:** With `--verify`, byte comparison is a **fifth step** (`[5/5] verify`), and the step total becomes 5 so the counter stays honest.
- **D-09:** Progress shows whenever **stderr is a TTY, including with `--json`**, because stdout stays pure JSON. This changes today's behavior, where `--json` silenced progress. Progress stays silent when stderr is not a TTY.
- **D-10:** A test observer receives the same stage events in the same order as the terminal observer (success criterion 2).

#### dry_run semantics
- **D-11:** `meta.dry_run` means "this report describes a clean that changed nothing". A plain `twins scan` / `twins report` reports **`false`**, which keeps the output byte-identical to today and matches Go v0, where the flag came from `clean --dry-run`.
- **D-12:** Core carries an explicit **run mode** (for example a `RunMode` enum covering scan and dry-run, extended by Phase 2 for real cleans) on the pipeline spec. `meta.dry_run` is **derived from it**, never hardcoded in a shell. — **Reversibility:** costly — Phase 2's `clean` and `--dry-run` path will build on this enum.
- **D-13:** **No schema change.** Schema v1 stays as is, with no `meta.mode` field in this phase; an explicit mode field can come with `clean` in Phase 2.
- **D-14:** The wiring is proven by **core-level tests**: running the pipeline in dry-run mode on a temp tree must give `meta.dry_run == true`, and a scan must give `false`. No hidden CLI `--dry-run` flag. A CLI-level dry-run test follows in Phase 2.

#### Toolchain & CI
- **D-15:** Set `rust-version = "1.90"` in `[workspace.package]`. **No `rust-toolchain.toml`**, so local development stays on stable.
- **D-16:** CI keeps the existing stable job (fmt, clippy `-D warnings`, test, release build) and **adds an MSRV job on 1.90** that runs build and test.
- **D-17:** New lints surfaced by 1.90 or a newer clippy pedantic get **fixed in code**. Use a targeted `#[allow]` with a justifying comment only where a fix would hurt clarity, per project conventions.

### Claude's Discretion
- Signal plumbing for Ctrl+C (the `ctrlc` crate vs `signal-hook`). The only constraint is that core sees nothing but a `CancelToken`.
- The exact `Event` variant set for this phase (scan events only vs also stubbing the clean-related variants from research), the throttle interval (research suggests about 80 ms), and the exact module layout (`observe.rs`, `pipeline.rs`).
- How the walk exposes per-file progress to the observer, as long as the step-counter output above is achievable.

### Deferred Ideas (OUT OF SCOPE)
- An explicit `meta.mode` field in the JSON report: consider it with `twins clean` in Phase 2.
- A CLI-level `--dry-run` end-to-end test: Phase 2, once `clean --dry-run` exists.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-01 | The scan pipeline runs inside `twins-core` and reports staged progress to any caller (CLI, app, agent) through an observer | Pattern 1 (pipeline module), Pattern 2 (observe module), Pattern 3 (walk progress hook), Pattern 4 (stage-start markers in `group::find`); characterization tests in Wave 0 guard "same groups, same JSON" |
| CORE-02 | User can cancel a running scan or clean, and the operation stops promptly without leaving partial state | Cancellation gap analysis (Pitfalls 1-3): walk stat phase never polls cancel, `hash::full`/`hash::equal` cannot be interrupted mid-file; Pattern 5 (in-file cancel); Pattern 6 (signal-hook double Ctrl+C); exit 130 mapping |
| CORE-03 | The JSON report's `dry_run` field reflects the real mode of the run instead of being hardcoded | `RunMode` on `ScanSpec`, `Meta.dry_run` derived in core; core tests for both modes; grep gate that `twins-cli/src` no longer mentions `dry_run` |
| CORE-04 | The workspace builds on Rust 1.90 (the MSRV Tauri needs) with existing tests passing | Clippy probe at `rust-version = "1.90"` found exactly 2 new errors (`manual_is_multiple_of`, both in code being replaced); `globset 0.4.20` already declares MSRV 1.88, so the current `1.85` is already false; CI MSRV job recipe |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Directives the planner must verify compliance with (from `/Users/ayoub/projects/twins/.claude/CLAUDE.md` and the user's global `~/.claude/CLAUDE.md`):

- **Architecture:** `twins-core` holds all logic; the CLI, app and workflow runner are thin shells. (This phase is the enforcement of that rule for scanning.)
- **Safety / Core Value:** never lose data; `--dry-run` changes nothing. (Scan writes nothing, so cancel-anywhere is safe.)
- **Tech stack:** Rust edition 2024, workspace lints `unsafe_code = "warn"`, `missing_docs = "warn"`, clippy `all` + `pedantic` at warn, `module_name_repetitions = "allow"`. CI runs `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check`.
- **Docs:** every public function, type, module and public field gets a doc comment; `# Errors` section on fallible public functions.
- **Errors:** `thiserror` enums named `{Domain}Error`; CLI uses `anyhow::Result`.
- **Unsafe:** `#[allow(unsafe_code)]` at function level with an explanatory comment. (Recommended plan uses **no** new `unsafe`: `signal_hook::flag::register*` are safe functions.)
- **Visibility:** `pub use` re-exports in `mod.rs`; prefer private `mod` + selective `pub use`; private fields with `#[must_use]` accessors.
- **Builders:** consuming `self -> Self` setters with `#[must_use]`.
- **Derives:** `Debug, Clone, PartialEq, Eq, Hash` as appropriate; `serde::Serialize` for JSON-bound types.
- **Tests:** `tests/{module}_test.rs` per crate; shared fixtures in `crates/twins-core/tests/fixtures/mod.rs`; CLI tests with `assert_cmd` + `predicates` + `tempfile`.
- **Git:** no Claude/AI attribution in commits or PRs (user global instruction). Commit pattern from `.dev-workflow.json`: `type(scope): description (<ID>)`.
- **Workflow:** file changes go through a GSD command (`/gsd-execute-phase` for this phase).

## Summary

The refactor is mechanically small but has four non-obvious traps that the CONTEXT decisions already imply and that the current code does not satisfy:

1. **Cancellation is not actually prompt today.** The walk polls the cancel flag only in the serial directory listing; the parallel `lstat` phase over every collected file never polls it. Hashing polls between files, but `hash::full` and `hash::equal` stream a whole file with no check, so one large video (or a slow external disk) blocks cancellation for seconds to minutes. Meeting D-05 ("within about a second") needs a per-chunk check inside the read loops and a per-file check in the walk's stat loop.
2. **There is no "stage started" signal and no walk progress during directory listing.** `group::find` only reports progress after each file finishes, so an empty stage emits nothing and the event order D-07/D-10 require (walk → size grouping → partial → full [→ verify]) is not observable. The walk only reports through `visit`, which fires during the stat phase, not during the slow directory listing.
3. **Error-type plumbing decides exit codes.** `run::exit_code` maps any `scan::ScanError` (including `ScanError::Cancelled`) to exit 2 by downcasting. Wrapping core errors in a new pipeline error silently changes exit codes unless the CLI matches the new type, and cancellation must be special-cased to 130 with the exact `scan cancelled` line (no `twins: ` prefix).
4. **The current MSRV is already wrong.** `globset 0.4.20` declares `rust-version = 1.88`, so the workspace cannot build on its declared 1.85. Raising to 1.90 fixes that, and the only lint fallout (probed this session) is two `clippy::manual_is_multiple_of` errors in `run.rs`'s `Progress` struct, which this phase deletes anyway.

**Primary recommendation:** Build `observe.rs` (`CancelToken`, `Stage`, `Event`, `Observer`, `Throttle`) and `pipeline.rs` (`ScanSpec` with `RunMode`, `ScanOutcome`, `PipelineError`, `scan()`) in `twins-core`. Harden cancellation inside `scan::walk`, `group::find` and `hash` with additive, non-breaking changes. Then hollow out `twins-cli/src/run.rs` into a spec builder, a `TerminalObserver` wrapped in `Throttle` (80 ms), and a `signal-hook` SIGINT registration that uses the crate's documented double-Ctrl+C idiom. Lock "same output" with characterization tests written **before** the refactor.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Orchestration walk → index → find → plan → report | Core library (`twins-core::pipeline`) | — | CLAUDE.md: core holds all logic. The app (Phase 5) and runner (Phase 6) reuse it verbatim |
| Stage/progress events, throttling | Core library (`twins-core::observe`) | Shell renders | `Event` must be `Serialize` for the Tauri `Channel<Event>` (ARCHITECTURE.md Pattern 4); throttling in core means every shell gets sane rates |
| Cancellation state | Core library (`CancelToken`) | Shell owns the trigger | Core only polls a token; signals, IPC commands and launchd SIGTERM are shell concerns |
| SIGINT handling, exit code 130, `scan cancelled` message | CLI shell (`twins-cli`) | — | D-02/D-03/D-04 are terminal UX; core must not install signal handlers (it is a library linked into the app) |
| Progress line rendering (`[k/N] label  n / m`) | CLI shell | Core supplies `step`/`steps` | Formatting is presentation; core supplies the step index so app and CLI count the same way |
| Digit grouping `48 210` | Core library (`human`) | — | Pure formatting helper next to `human_size`; reusable by app and tests |
| `meta.dry_run` | Core library (derived from `RunMode`) | — | D-12: never set by a shell |
| Toolchain / MSRV / CI | Build config (`Cargo.toml`, `.github/workflows/ci.yml`) | — | D-15/D-16 |

## Standard Stack

### Core (already in the workspace, no change)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rayon | 1.12.0 | Walk and hashing pools | Already used; D-01 says pools stay as they are [VERIFIED: cargo metadata this session] |
| serde | 1.0.229 (derive) | `Serialize` on `Event`/`Stage`/`Outcome` | Already a `twins-core` dependency [VERIFIED: crates/twins-core/Cargo.toml:15] |
| thiserror | 2.0.20 | `PipelineError` | Project convention [VERIFIED: crates/twins-core/Cargo.toml:17] |
| std `std::io::IsTerminal` | std (1.70+) | TTY detection on stderr | Already used in `run.rs:3,32` [VERIFIED: crates/twins-cli/src/run.rs:3,32] |

### New (CLI only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| signal-hook | 0.4.4 (`default-features = false`) | SIGINT → set the `CancelToken` flag; second SIGINT → `_exit(130)` | Only in `twins-cli`. Core never links it [VERIFIED: crates.io API + crate source 0.4.4 read this session; MSRV 1.66] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| signal-hook 0.4.4 | ctrlc 3.5.2 | `ctrlc` runs the handler on a dedicated thread "each time we receive a Ctrl+C signal" (so it could clear the progress line before `process::exit(130)`), but it pulls `nix 0.31` plus, on Apple targets, `dispatch2 0.3` (objc2 family) and spawns a thread. signal-hook needs only `libc` (already in the lockfile) and `signal-hook-registry`, and its `flag` module documents the exact double-Ctrl+C idiom D-03 asks for. Both are safe APIs. [VERIFIED: ctrlc 3.5.2 source `src/lib.rs` and `Cargo.toml` read this session] |
| In-file cancel via a `Hasher` default method | Change `Hasher::full` signature | A signature change breaks `SpyHasher` in `group_test.rs:118` and the Phase 4 `CachingHasher` design; a defaulted trait method is additive |
| `done == 0` stage-start marker in `group::Progress` | New `on_stage` callback on `group::Options` | Both work; the marker needs no new API and is documented on `Progress`. Choose the marker |

**Installation:**
```bash
cargo add -p twins-cli signal-hook@0.4.4 --no-default-features
```
(With `resolver = "3"` and `rust-version = "1.90"`, Cargo's resolver prefers MSRV-compatible versions; 0.4.4 declares 1.66.) [CITED: doc.rust-lang.org/cargo resolver v3 MSRV-aware resolution — ASSUMED detail, see A3]

**Version verification (run this session):**
- `signal-hook`: max stable 0.4.4, published 2026-04-04, `rust_version` 1.66 [VERIFIED: crates.io API]
- `signal-hook-registry`: 1.4.8, published 2025-12-25, `rust_version` 1.26 [VERIFIED: crates.io API]
- `ctrlc` (alternative): 3.5.2, published 2026-02-10, `rust_version` 1.69.0 [VERIFIED: crates.io API]

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| signal-hook | crates.io | ~8 yrs (first publish 2018-06-22) | ~4.5M/wk | github.com/vorner/signal-hook | [OK] | Approved |
| ctrlc | crates.io | ~11 yrs (first publish 2015-07-16) | ~1.8M/wk | github.com/Detegr/rust-ctrlc | [OK] | Alternative only, not installed |

Run via `gsd-tools query package-legitimacy check --ecosystem crates ctrlc signal-hook` this session. Rust crates have no `postinstall`; signal-hook has an optional `cc` build-dependency only behind a non-default feature [VERIFIED: signal-hook-0.4.4/Cargo.toml read this session].

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Current Code: Verified Facts the Plan Depends On

Every value below was read from the file this session. Quotes are verbatim.

**Orchestration to move** — `crates/twins-cli/src/run.rs:18-77` [VERIFIED]. Key lines:
```rust
// run.rs:32
    let progress = Progress::new(!json && std::io::stderr().is_terminal());
// run.rs:56-64
    let keeper = Keeper::new(Strategy::default(), None);
    let actions = group::plan(&groups, &keeper);
    let meta = Meta {
        roots,
        files: stats.files,
        candidates: stats.candidates,
        strategy: keeper.strategy(),
        dry_run: false,
    };
// run.rs:65
    let r = report::build(&actions, &meta, SystemTime::now());
```
Note `roots` in `Meta` is the **user-supplied** list (`run.rs:20` `let roots = roots(&args.paths)?;`, then `scan::Options::new(roots.clone())` at `run.rs:22`), not the walk's normalised roots. The pipeline must keep that, or `"roots"` in the JSON changes.

**Error count and verbose skip lines** — `run.rs:43-51`, `run.rs:71-74`, `run.rs:89-96` [VERIFIED]:
```rust
    let errors = AtomicU64::new(stats.errors);
            errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("{n} files could not be read (use --verbose to list them)");
        let _ = writeln!(w, "skip {}: {err}", path.display());
```
The "could not be read" summary is printed only in text mode (`run.rs:69-75`). Keep both behaviours.

**Exit codes** — `run.rs:145-155` [VERIFIED]:
```rust
pub fn exit_code(err: &anyhow::Error) -> i32 {
    if err.downcast_ref::<scan::ScanError>().is_some()
        || err
            .downcast_ref::<twins_core::human::ParseSizeError>()
            .is_some()
    {
        2
    } else {
        1
    }
}
```
and `main.rs:25` `eprintln!("twins: {}", run::describe(&err));` [VERIFIED: crates/twins-cli/src/main.rs:22-28].

**Walk cancel** — `scan/options.rs:23` `pub(crate) cancel: Option<Arc<AtomicBool>>,` and the builder `pub fn cancel(self, cancel: Arc<AtomicBool>) -> Self` at `options.rs:118` [VERIFIED: crates/twins-core/src/scan/options.rs:13-24,115-123]. The only poll is in `collect_files` [VERIFIED: walk.rs:137-144]:
```rust
    for entry in iter {
        if opts
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            return Err(ScanError::Cancelled);
        }
```
The parallel stat phase does **not** poll [VERIFIED: walk.rs:102-109]:
```rust
    for root in &roots {
        let files = collect_files(root, opts, &rules, &counters, &report)?;
        pool.install(|| {
            files.par_iter().for_each(|path| {
                handle_file(path, opts, &counters, &visit, &report);
            });
        });
    }
```
Public signature to keep stable (tests call it 8 times in `scan_test.rs`) [VERIFIED: walk.rs:77-81]:
```rust
pub fn walk<V, E>(opts: &Options, visit: V, on_error: E) -> Result<Stats, ScanError>
where
    V: Fn(FileMeta) + Sync,
    E: Fn(&Path, &EntryError) + Sync,
```
`ScanError::Cancelled` is `#[error("scan cancelled")]` [VERIFIED: crates/twins-core/src/scan/mod.rs:44-46]. `Stats` fields: `dirs`, `files`, `candidates`, `skipped`, `errors` [VERIFIED: walk.rs:19-31].

**Find stages and cancel** — [VERIFIED: crates/twins-core/src/group/find.rs:23-39]:
```rust
pub enum Stage {
    /// Head and tail fingerprint.
    Partial,
    /// Full content digest.
    Full,
    /// Byte-by-byte comparison.
    Verify,
}
            Self::Partial => "partial hash",
            Self::Full => "full hash",
            Self::Verify => "verify",
```
`Progress { stage, done, total }` [VERIFIED: find.rs:44-51]. `FindError::Cancelled` is `#[error("scan cancelled")]` [VERIFIED: find.rs:66-73]. Progress callback type `type ProgressFn<'a> = Box<dyn Fn(Progress) + Send + Sync + 'a>;` [VERIFIED: find.rs:75]. Progress is emitted only **after** each key is computed [VERIFIED: find.rs:297-303]:
```rust
        .map(|m| {
            if opts.cancelled() {
                return None;
            }
            let r = key(m);
            opts.progress(stage, done.fetch_add(1, Ordering::Relaxed) + 1, total);
            Some((m.identity(), r))
        })
```
Verify runs serially over buckets and only polls between buckets [VERIFIED: find.rs:209-222, 320-338].

**Hashing has no in-file cancel** — [VERIFIED: crates/twins-core/src/hash.rs:13, 107-119]:
```rust
const BUFFER_SIZE: usize = 256 * 1024;
pub fn full(path: &Path) -> Result<Digest, HashError> {
    let mut f = File::open(path).map_err(err("open", path))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        let n = f.read(&mut buf).map_err(err("read", path))?;
```
`hash::equal` has the same shape [VERIFIED: hash.rs:125-140]. `HashError { op: &'static str, path: PathBuf, source: io::Error }` [VERIFIED: hash.rs:18-28].

**Hasher trait** — [VERIFIED: crates/twins-core/src/group/hasher.rs:8-20]: `pub trait Hasher: Send + Sync { fn partial(&self, m: &FileMeta) -> Result<u64, HashError>; fn full(&self, m: &FileMeta) -> Result<Digest, HashError>; }`, implemented by `DirectHasher` and by the test-only `SpyHasher` (`group_test.rs:118`).

**Report meta** — [VERIFIED: crates/twins-core/src/report.rs:18-30, 108]: `pub struct Meta { pub roots: Vec<PathBuf>, pub files: u64, pub candidates: u64, pub strategy: Strategy, pub dry_run: bool }` and `dry_run: meta.dry_run,` inside `build`. `report::VERSION` is `pub const VERSION: u32 = 1;` [VERIFIED: report.rs:15].

**Human formatting** — `human.rs` exposes only `human_size` and `parse_size` [VERIFIED: crates/twins-core/src/human.rs:9,34]. **There is no digit-grouping helper today**, despite CLAUDE.md saying human.rs formats numbers. D-06's `48 210` needs a new `human::group_digits`.

**Toolchain** — `Cargo.toml:10` `rust-version = "1.85"` [VERIFIED]. CI has one job, `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`, then fmt, clippy `-D warnings`, test, release build, with env `RUSTFLAGS: -D warnings` [VERIFIED: .github/workflows/ci.yml:1-21].

## Architecture Patterns

### System Architecture Diagram

```
 user: twins scan/report [flags] [paths]          (later: Tauri command, workflow runner)
              │
              ▼
 ┌──────────── twins-cli (shell) ─────────────────────────────────────────┐
 │ clap args ──► build ScanSpec (roots|$HOME, min size, excludes, jobs,   │
 │               verify, strategy=oldest, mode=RunMode::Scan)             │
 │ CancelToken ◄── signal-hook: SIGINT#1 sets flag, SIGINT#2 _exit(130)   │
 │ Observer = Throttle(80ms, TerminalObserver{tty?, verbose})             │
 └───────────────┬────────────────────────────────────────────────────────┘
                 │ pipeline::scan(&spec, &observer, &cancel)
                 ▼
 ┌──────────── twins-core::pipeline ──────────────────────────────────────┐
 │ StageStarted(Walk 1/N) ─► scan::walk (+progress hook, polls cancel     │
 │                           in dir listing AND parallel stat)            │
 │        │ FileMeta ─► Index::add            FileSkipped events ──┐      │
 │ cancelled? ─► Err(Cancelled)                                    │      │
 │ StageStarted(SizeGrouping 2/N) + Progress(candidates)           │      │
 │ group::find(idx, opts{cancel, on_progress→Events, on_error})    │      │
 │   Partial(3/N) ─► Full(4/N) ─► [Verify(5/5)]  (start marker     │      │
 │   done=0 per stage; per-file + per-chunk cancel polls)          │      │
 │ cancelled? ─► Err(Cancelled)                                    │      │
 │ Keeper(strategy) ─► group::plan ─► Meta{dry_run = mode.is_dry_run()}   │
 │ Finished{Completed|Cancelled|Failed} (always, never throttled)  │      │
 └───────────────┬─────────────────────────────────────────────────┴──────┘
                 │ Ok(ScanOutcome{actions, meta, stats, errors})  /  Err(PipelineError)
                 ▼
 CLI: Ok  → report::build(now) → stdout JSON|text; text mode prints "N files could not be read"
      Err(Cancelled) → clear line, stderr "scan cancelled", exit 130, stdout empty
      Err(Scan(..)) / ParseSizeError → "twins: …", exit 2;   other → exit 1
```

### Recommended Project Structure

```
crates/twins-core/src/
├── lib.rs            # + pub mod observe; pub mod pipeline;
├── observe.rs        # NEW: CancelToken, Stage, Event, Outcome, Observer, Throttle<O>
├── pipeline.rs       # NEW: RunMode, ScanSpec (builder), ScanOutcome, PipelineError, scan()
├── human.rs          # + group_digits(u64) -> String
├── hash.rs           # + full_cancellable / equal_cancellable (chunk-level poll)
├── group/find.rs     # + stage-start marker (done=0), cancel-aware hashing, verify polls per file
├── group/hasher.rs   # + defaulted Hasher::full_cancellable
└── scan/walk.rs      # + pub(crate) walk_observed (progress hook), cancel poll in stat phase
crates/twins-core/tests/
├── observe_test.rs   # NEW: CancelToken, Throttle, Event JSON shape
├── pipeline_test.rs  # NEW: stage order, dry_run both modes, cancel at each stage, parity with manual wiring
└── (existing files extended: hash_test, group_test, scan_test, human_test)
crates/twins-cli/src/
├── main.rs           # cancelled → "scan cancelled", ExitCode 130
├── run.rs            # shrinks to: spec from args, run pipeline, print report
└── progress.rs       # NEW: TerminalObserver + pure line formatter (#[cfg(test)] unit tests)
crates/twins-cli/tests/cli_test.rs   # + characterization tests (Wave 0), --json non-TTY stderr silent, #[ignore] SIGINT test
.github/workflows/ci.yml              # + msrv job on 1.90.0
```

### Pattern 1: `pipeline::scan` owns the orchestration

**What:** One function that does exactly what `run.rs:18-77` does today, minus printing, plus events and cancellation.
**When to use:** Every shell. The CLI must not call `scan::walk`, `group::find`, `group::plan` or construct `report::Meta` after this phase.

```rust
// twins-core/src/pipeline.rs — recommended shape (new API; names are proposals)
/// What kind of run produced this report. Phase 2 adds real clean modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum RunMode {
    /// Read-only scan (`twins scan`, `twins report`).
    #[default]
    Scan,
    /// A clean that only simulated its actions.
    DryRun,
}

impl RunMode {
    /// Whether `report::Meta::dry_run` must be true.
    #[must_use]
    pub fn is_dry_run(self) -> bool { matches!(self, Self::DryRun) }
}

/// Everything a scan needs. Built by shells from flags (later: config, workflow).
#[derive(Debug, Clone)]
pub struct ScanSpec {
    walk: scan::Options,      // roots, min size, excludes, include_*, workers
    hash_workers: usize,      // `--jobs` also drives group::Options::workers today
    verify: bool,
    strategy: Strategy,
    keep_dir: Option<PathBuf>,
    mode: RunMode,
}
// consuming #[must_use] setters: verify(), hash_workers(), strategy(), keep_dir(), mode()

/// Result of a completed scan. Groups stay in Rust (Phase 5 pages them).
#[derive(Debug, Clone)]
pub struct ScanOutcome { actions: Vec<Action>, meta: Meta, stats: Stats, errors: u64 }
impl ScanOutcome {
    /// Builds the schema-v1 report stamped with `now`.
    #[must_use]
    pub fn report(&self, now: SystemTime) -> Report { report::build(&self.actions, &self.meta, now) }
    // + #[must_use] accessors: actions(), meta(), stats(), errors()
}

/// Fatal errors of a pipeline run.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Invalid roots, bad globs, I/O on a root.
    #[error(transparent)]
    Scan(ScanError),            // never holds ScanError::Cancelled — mapped below
    /// Hashing pool could not be built.
    #[error(transparent)]
    Find(FindError),            // never holds FindError::Cancelled
    /// The CancelToken was raised.
    #[error("scan cancelled")]
    Cancelled,
}
// From<ScanError>: Cancelled => PipelineError::Cancelled, other => Scan(other); same for FindError.

/// # Errors
/// [`PipelineError::Cancelled`] when `cancel` is raised; otherwise walk or pool failures.
pub fn scan(spec: &ScanSpec, observer: &dyn Observer, cancel: &CancelToken)
    -> Result<ScanOutcome, PipelineError>;
```

Inside `scan`, wrap the body in an inner function and emit `Event::Finished` on **every** exit path (completed, cancelled, failed), so a shell can always clear its progress line:

```rust
pub fn scan(spec: &ScanSpec, observer: &dyn Observer, cancel: &CancelToken)
    -> Result<ScanOutcome, PipelineError> {
    let result = run(spec, observer, cancel);
    observer.on_event(&Event::Finished { outcome: Outcome::from(&result) });
    result
}
```

Between stages, check `cancel.is_cancelled()` explicitly and return `PipelineError::Cancelled`: `group::find` returns `Ok` when cancel is set but there are no representatives, so a cancel that lands between the walk and the hashing would otherwise be ignored [VERIFIED: find.rs:293-316 — the `None => return Err(FindError::Cancelled)` branch only runs if `reps` is non-empty].

Meta construction stays byte-identical to today, with `dry_run` derived:
```rust
let keeper = Keeper::new(spec.strategy, spec.keep_dir.clone());
let actions = group::plan(&groups, &keeper);
let meta = Meta {
    roots: spec.walk.roots().to_vec(),   // user-supplied roots, as run.rs did
    files: stats.files,
    candidates: stats.candidates,
    strategy: keeper.strategy(),
    dry_run: spec.mode.is_dry_run(),
};
```

### Pattern 2: `observe` module

```rust
// twins-core/src/observe.rs — recommended shape
/// Shared cancellation flag. Cloning shares the flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    #[must_use] pub fn new() -> Self { Self::default() }
    pub fn cancel(&self) { self.0.store(true, Ordering::SeqCst) }
    #[must_use] pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
    /// The underlying flag, for signal handlers that can only set an `AtomicBool`.
    #[must_use] pub fn flag(&self) -> Arc<AtomicBool> { Arc::clone(&self.0) }
}

/// Pipeline stage, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage { Walk, SizeGrouping, PartialHash, FullHash, Verify }
// Display: "walk", "size grouping", "partial hash", "full hash", "verify"
// From<group::Stage>: Partial→PartialHash, Full→FullHash, Verify→Verify

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome { Completed, Cancelled, Failed }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[non_exhaustive]
pub enum Event {
    /// A stage begins. `step` is 1-based; `steps` is 4, or 5 with verify. Never throttled.
    StageStarted { stage: Stage, step: u8, steps: u8 },
    /// Advancement inside a stage. `total` is `None` while walking. Throttled.
    Progress { stage: Stage, done: u64, total: Option<u64> },
    /// A file was dropped (unreadable, vanished, hash collision). Never throttled.
    FileSkipped { path: String, reason: String },
    /// The run ended, whatever the outcome. Never throttled; always last.
    Finished { outcome: Outcome },
}

/// Receives pipeline events from any thread.
pub trait Observer: Send + Sync {
    /// Called for every event; must be cheap and must not block for long.
    fn on_event(&self, event: &Event);
}
```

Recommendation on the variant set (Claude's discretion): ship **scan events only**. `#[non_exhaustive]` lets Phase 2 add `Planned`, `Exec`, `Removed`, `GroupSkipped` without breaking shells. Stubbing clean variants now creates dead API that Phase 2 may want to shape differently.

**Throttle** (`Throttle<O: Observer>`): forwards `StageStarted`, `FileSkipped` and `Finished` immediately; forwards `Progress` when at least `interval` has elapsed since the last forwarded `Progress`, **or** when `total == Some(done)` (so the final count is shown). The first `Progress` always passes. Use an `AtomicU64` of nanoseconds since a base `Instant` captured in `new`, claimed with `compare_exchange`, so hashing threads never contend on a `Mutex`. Interval: **80 ms** (≈12 Hz, inside the 10-20 msg/s the research recommends for Tauri channels [CITED: .planning/research/STACK.md:35]).

### Pattern 3: Walk progress without breaking `scan::walk`

Add a crate-private sibling and make `walk` delegate to it:

```rust
// scan/walk.rs
pub fn walk<V, E>(opts: &Options, visit: V, on_error: E) -> Result<Stats, ScanError>
where V: Fn(FileMeta) + Sync, E: Fn(&Path, &EntryError) + Sync,
{ walk_observed(opts, visit, on_error, |_| {}) }

/// Same as [`walk`], calling `on_progress` with a counters snapshot as regular files are seen.
pub(crate) fn walk_observed<V, E, P>(opts: &Options, visit: V, on_error: E, on_progress: P)
    -> Result<Stats, ScanError>
where V: Fn(FileMeta) + Sync, E: Fn(&Path, &EntryError) + Sync, P: Fn(&Stats) + Sync;
```

Call `on_progress` in `collect_files` each time `counters.files` is bumped (that is the slow, serial directory listing, where users wait today with no feedback) and in `handle_file` after `counters.candidates` is bumped. The pipeline maps it to `Event::Progress { stage: Walk, done: stats.files, total: None }`; the Throttle drops almost all of them. Calling `Counters::snapshot` per file is five relaxed loads; cheap, but if profiling shows cost, pass only `files` as a `u64`.

### Pattern 4: Observable stage boundaries in `group::find`

Emit a start marker `opts.progress(stage, 0, total)` at the start of every stage, **before** any work and even when `total == 0`:
- in `compute_keys` before the `par_iter` (covers Partial and Full; `refine` is always called for both [VERIFIED: find.rs:198-206]);
- before the verify loop when `opts.verify` is set (total = number of full buckets).

Document on `Progress`: "Each stage begins with one event where `done == 0`." The pipeline turns `done == 0` into `Event::StageStarted { stage, step, steps }` and everything else into `Event::Progress`. Because the marker is sent from the calling thread before the parallel section, it is always ordered before that stage's worker events. The existing `reports_progress_per_stage` test uses `rfind` on the last event per stage, so it keeps passing [VERIFIED: group_test.rs:203-222].

Size grouping (D-07): the pipeline itself emits `StageStarted { stage: SizeGrouping, step: 2, .. }` and `Progress { stage: SizeGrouping, done: stats.candidates, total: Some(stats.candidates) }` right after the walk and before calling `group::find` (which does the actual bucketing in `idx.candidates()` as its first statement [VERIFIED: find.rs:199]). Using `stats.candidates` keeps the number identical to the report's `summary.candidates`. Do **not** call `idx.candidates()` twice just to count: it clones every `FileMeta` [VERIFIED: index.rs:49-59].

Step numbering: `steps = if spec.verify { 5 } else { 4 }`; Walk=1, SizeGrouping=2, PartialHash=3, FullHash=4, Verify=5.

### Pattern 5: Prompt cancellation inside files and in the stat phase

1. **Walk stat phase**: in the `par_iter` closure, return early when the flag is set; after `pool.install`, return `Err(ScanError::Cancelled)` if set. Also check once per root before `collect_files`.
2. **In-file hashing**: add `hash::full_cancellable(path, cancel: &AtomicBool)` and `hash::equal_cancellable(a, b, cancel)`, polling once per 256 KiB chunk; on cancel, return a `HashError { op: "cancelled", path, source: io::ErrorKind::Interrupted.into() }`. Keep `full`/`equal` as wrappers passing a never-set flag, so the public API and `hash_test.rs` are unchanged.
3. **Hasher trait**: add a defaulted method so existing implementors (`SpyHasher`, the future `CachingHasher`) compile unchanged:
   ```rust
   /// Full digest that may stop early when `cancel` is set.
   /// # Errors
   /// When the file cannot be read, or `cancel` was raised mid-file.
   fn full_cancellable(&self, m: &FileMeta, cancel: &AtomicBool) -> Result<Digest, HashError> {
       let _ = cancel;
       self.full(m)
   }
   ```
   `DirectHasher` overrides it with `hash::full_cancellable`.
4. **`compute_keys`**: after `let r = key(m);`, check `opts.cancelled()` again and return `None` if set. This matters: an interrupted hash returns `Err`, which would otherwise be **reported as a skipped file** through `on_error` (`find.rs:313`) instead of becoming `FindError::Cancelled`.
5. **`verify`**: poll `opts.cancelled()` per file inside the bucket loop and use `hash::equal_cancellable`; make `find_in_pool` return `Cancelled` if the flag is set after `verify` returns.

Forward-compatibility with CACHE-03 ("a cancelled scan keeps the hashes it already computed"): because an interrupted file returns `Err`, never a digest of a prefix, the Phase 4 `CachingHasher` cannot cache a truncated digest. Note this in the doc comment.

### Pattern 6: Ctrl+C in the CLI with signal-hook

```rust
// twins-cli — before calling pipeline::scan
use signal_hook::consts::signal::SIGINT;
let cancel = CancelToken::new();
let flag = cancel.flag();
// Order matters: the shutdown action must be registered first. On the first SIGINT
// the flag is still false (no exit), then the second action sets it. On the second
// SIGINT the flag is true, so the process _exit(130)s immediately.
signal_hook::flag::register_conditional_shutdown(SIGINT, 130, Arc::clone(&flag))?;
signal_hook::flag::register(SIGINT, flag)?;
```
Source: signal-hook 0.4.4 `src/flag.rs` doc on `register_conditional_shutdown`: "The last one is handling double CTRL+C … one can combine this with [`register`]. On the first run, the flag is `false` and this doesn't terminate. But then the flag is set to true during the first run and „arms" the shutdown on the second run. Note that it matters in which order the actions are registered (the shutdown must go first)." The exit uses `libc::_exit` ("doesn't call the at-exit hooks") [VERIFIED: signal-hook-0.4.4/src/flag.rs:166-196 and src/low_level/mod.rs:47-57, read this session]. Both functions are safe (`pub fn`), so no `unsafe` enters the CLI. Use `signal_hook::consts::signal::SIGINT` (re-exported from `libc`) [VERIFIED: signal-hook-0.4.4/src/lib.rs:361-391].

In `main.rs`, check for cancellation before the generic error path:
```rust
Err(err) if err.downcast_ref::<PipelineError>().is_some_and(PipelineError::is_cancelled) => {
    eprintln!("scan cancelled");          // progress line was already cleared on Event::Finished
    ExitCode::from(130)
}
```
and extend `exit_code` so `PipelineError::Scan(_)` maps to 2 (as `ScanError` does today) and `PipelineError::Find(_)` to 1.

### Pattern 7: Terminal observer

- Enabled iff `std::io::stderr().is_terminal()`, **regardless of `--json`** (D-09).
- `StageStarted` → remember `(step, steps, label)` and draw `[{step}/{steps}] {label}`.
- `Progress` → `\r\x1b[K[{step}/{steps}] {label}  {n}` + ` files` when `total` is `None`, `  {done} / {total}` otherwise, with `human::group_digits`. Approved examples: `[1/4] walk  48 210 files`, `[3/4] partial hash  1 234 / 5 000`.
- `FileSkipped` → if `--verbose`, clear the line first (`\r\x1b[K`), then `skip {path}: {reason}` (same text as `run.rs:94`); otherwise ignore. Always counted by the pipeline in `ScanOutcome::errors`.
- `Finished` → clear the line (`\r\x1b[K`) if enabled. This replaces `Progress::finish` and also fixes today's leftover line on error paths (the `?` at `run.rs:41/52` returns before `progress.finish()` [VERIFIED: run.rs:34-54]).
- Keep the line formatter a pure function `fn line(step, steps, stage, done, total) -> String` so it can be unit-tested without a TTY.

### Anti-Patterns to Avoid

- **Building `report::Meta` in a shell.** That is how `dry_run: false` got hardcoded. After this phase `grep -rn "dry_run\|Meta {" crates/twins-cli/src` must return nothing.
- **Signal handling in core.** `twins-core` is linked into the Tauri app; installing a SIGINT handler there would hijack the app's signals. Core sees only a `CancelToken`.
- **Throttling `StageStarted`/`Finished`.** That breaks D-10 ordering and leaves a stale progress line.
- **`PathBuf` in `Event`.** serde's `PathBuf` impl fails at runtime on non-UTF-8 paths ("path contains invalid UTF-8 characters"), which would break the Tauri channel; use `String` via `to_string_lossy`, as `report::display` already does [VERIFIED: report.rs:140-142]. [ASSUMED: exact serde error text]
- **Tuple variants in an internally tagged enum.** `#[serde(tag = "type")]` on an enum with a tuple variant is a compile error; a newtype variant around a primitive fails at runtime. Use struct variants only [CITED: serde.rs/enum-representations.html].
- **`rename_all` alone for camelCase fields.** On an enum it renames variants only; field names inside struct variants need `rename_all_fields` [CITED: serde.rs/container-attrs.html]. (Every field proposed for this phase is a single word, so it changes nothing today; add it anyway so Phase 2 fields like `trashed_to` serialize as `trashedTo` for the app.)
- **Two-pass `idx.candidates()`** to count for size grouping: doubles memory transiently on a 1M-file scan.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SIGINT handling / double Ctrl+C | `libc::sigaction` with a custom handler | `signal_hook::flag::{register_conditional_shutdown, register}` | Async-signal-safety is subtle; the crate's handlers are documented as safe and the idiom is exactly D-03; avoids new `unsafe` |
| TTY detection | `libc::isatty` | `std::io::IsTerminal` | Already used; std, no unsafe |
| Event JSON | Manual `serde_json::json!` building | `#[derive(Serialize)]` with `tag`/`rename_all`/`rename_all_fields` | Stable schema for the Tauri channel |
| Thread pools / parallel iteration | New pools or channels | Existing rayon pools in `walk` and `find` | D-01: pools stay as they are |
| Lock-free rate limiting | `Mutex<Instant>` in a hot path | `AtomicU64` nanos + `compare_exchange` | Hashing threads call the observer per file |

**Key insight:** almost everything this phase needs already exists in core (cancel flags, progress callbacks, error callbacks). The work is wiring, three small cancellation fixes, and making stage boundaries explicit; resist redesigning `walk`/`find`.

## Runtime State Inventory

This is a refactor phase. Scan is read-only and twins persists nothing yet (no cache, config, journal or agent until later phases).

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None. twins writes no files in v1-beta step 1 (verified: no write paths in `crates/twins-core/src` besides stdout/stderr writers in `report.rs`) | None |
| Live service config | None (no services) | None |
| OS-registered state | None (no launchd/agent yet) | None |
| Secrets/env vars | `HOME` read for default root (`run.rs:83`) and `~/Library` (`scan/options.rs:125-129`); `TWINS_CONFIG` is removed in tests (`cli_test.rs:11`) but read nowhere | Keep the `HOME` fallback in the CLI spec builder; no rename |
| Build artifacts | `Cargo.lock` gains `signal-hook` + `signal-hook-registry`; CI cache key changes with the new toolchain job | Commit `Cargo.lock`; `Swatinem/rust-cache` keys include the rustc version automatically [ASSUMED] |

## Common Pitfalls

### Pitfall 1: Cancellation that is only checked between files
**What goes wrong:** Ctrl+C during the full hash of a 20 GB file or a slow external disk takes tens of seconds to minutes.
**Why it happens:** `hash::full` loops over the whole file with no poll [VERIFIED: hash.rs:107-119]; the walk's parallel stat phase never polls [VERIFIED: walk.rs:102-109].
**How to avoid:** Pattern 5. Poll per 256 KiB chunk and per file in the stat loop.
**Warning signs:** a test with a cancel flag raised before `hash::full_cancellable` still reads the whole file; a manual Ctrl+C on a tree of large files lags.

### Pitfall 2: Interrupted hash reported as an unreadable file
**What goes wrong:** On cancel, the verbose CLI prints `skip …: cancelled …` lines, and the error count rises.
**Why it happens:** `compute_keys` reports any `Err` from the key function via `on_error` [VERIFIED: find.rs:313].
**How to avoid:** Re-check `opts.cancelled()` after computing the key and drop the result (Pattern 5, step 4).

### Pitfall 3: Cancel between stages ignored
**What goes wrong:** Ctrl+C right after the walk finishes, on a tree with no size collisions, prints a full report and exits 0.
**Why it happens:** `find` only returns `Cancelled` from inside `compute_keys` for non-empty inputs.
**How to avoid:** The pipeline checks `cancel.is_cancelled()` after the walk, after `find`, and before building the outcome.

### Pitfall 4: Exit codes drift when errors are wrapped
**What goes wrong:** `twins scan /System` now exits 1 instead of 2, or a cancel exits 2 with `twins: scan cancelled`.
**Why it happens:** `exit_code` downcasts to `scan::ScanError` [VERIFIED: run.rs:145-155]; a new `PipelineError` wrapper hides it, and `ScanError::Cancelled` would map to 2.
**How to avoid:** Match `PipelineError` explicitly: `Cancelled` → 130 with the bare `scan cancelled` line, `Scan(_)` → 2, `Find(_)` → 1, `ParseSizeError` → 2. Add a CLI test asserting `.code(2)` for `/System` (today's test only checks `.failure()` [VERIFIED: cli_test.rs:139-158]).

### Pitfall 5: "Same JSON" broken by normalised roots
**What goes wrong:** `"roots"` in the report changes from the given path to an absolutised/deduplicated path.
**Why it happens:** The walk normalises roots internally; a refactor may read them back from the walk.
**How to avoid:** `Meta.roots = spec.walk.roots().to_vec()` (the accessor exists [VERIFIED: scan/options.rs:44-48]). Characterization test passes a **relative** root to catch this.

### Pitfall 6: Progress line left on screen or mixed into verbose output
**What goes wrong:** On error or cancel, the last `[3/4] …` stays on the terminal and `scan cancelled` lands on the same line; verbose `skip` lines interleave with the progress line.
**How to avoid:** Clear on `Event::Finished` (always emitted), clear before printing each verbose skip line.

### Pitfall 7: MSRV declared but never tested
**What goes wrong:** A new dependency or std API newer than 1.90 slips in; the declared MSRV is a lie (as `1.85` already is today because `globset 0.4.20` declares 1.88 [VERIFIED: `cargo metadata` this session, `globset 0.4.20 rust_version 1.88`]).
**How to avoid:** The MSRV CI job (D-16). Locally, clippy is MSRV-aware through `rust-version`; at 1.90 it already flags `manual_is_multiple_of` (stabilised `is_multiple_of`) [VERIFIED: clippy run on a scratch copy with `rust-version = "1.90"`: exactly 2 errors, `crates/twins-cli/src/run.rs:114:28` and `:120:29`; after fixing them, "No issues found"].

### Pitfall 8: SIGINT race in an integration test
**What goes wrong:** A CLI test that sends SIGINT can hit the process before the handler is registered (killed by the default action, `status.code() == None`), or after the scan already finished (exit 0).
**How to avoid:** Prove cancellation deterministically at core level (observer cancels the token on a given `StageStarted`). Keep the CLI SIGINT test `#[ignore]` and run it in phase verification.

## Code Examples

### Deterministic cancel test (core)

```rust
// crates/twins-core/tests/pipeline_test.rs
struct CancelOn { stage: Stage, token: CancelToken, seen: Mutex<Vec<Event>> }
impl Observer for CancelOn {
    fn on_event(&self, e: &Event) {
        if let Event::StageStarted { stage, .. } = e && *stage == self.stage { self.token.cancel(); }
        self.seen.lock().unwrap().push(e.clone());
    }
}

#[test]
fn cancel_at_partial_hash_stops_before_full_hash() {
    let t = Tree::build(&[("a.bin", &mib(1)), ("b.bin", &mib(1))]);
    let token = CancelToken::new();
    let obs = CancelOn { stage: Stage::PartialHash, token: token.clone(), seen: Mutex::default() };
    let spec = ScanSpec::new(scan::Options::new(vec![t.root().into()]).home(t.root().into()));
    let err = pipeline::scan(&spec, &obs, &token).unwrap_err();
    assert!(err.is_cancelled());
    let seen = obs.seen.into_inner().unwrap();
    assert!(!seen.iter().any(|e| matches!(e, Event::StageStarted { stage: Stage::FullHash, .. })));
    assert!(matches!(seen.last(), Some(Event::Finished { outcome: Outcome::Cancelled })));
}
```
(`let`-chains in `if let … && …` are stable on edition 2024 since Rust 1.88, so they are allowed at MSRV 1.90 [ASSUMED: stabilisation version].)

### dry_run derived from mode (CORE-03, D-14)

```rust
#[test]
fn dry_run_in_meta_follows_run_mode() {
    let t = Tree::build(&[("a.bin", &mib(1)), ("b.bin", &mib(1))]);
    let base = ScanSpec::new(scan::Options::new(vec![t.root().into()]).home(t.root().into()));
    let none = CancelToken::new();
    let scan = pipeline::scan(&base.clone().mode(RunMode::Scan), &NoopObserver, &none).unwrap();
    let dry = pipeline::scan(&base.mode(RunMode::DryRun), &NoopObserver, &none).unwrap();
    assert!(!scan.report(SystemTime::UNIX_EPOCH).dry_run);
    assert!(dry.report(SystemTime::UNIX_EPOCH).dry_run);
}
```

### Stage order (D-07, D-08, D-10)

```rust
fn stages(seen: &[Event]) -> Vec<(Stage, u8, u8)> {
    seen.iter().filter_map(|e| match e {
        Event::StageStarted { stage, step, steps } => Some((*stage, *step, *steps)),
        _ => None,
    }).collect()
}
// without verify: [(Walk,1,4),(SizeGrouping,2,4),(PartialHash,3,4),(FullHash,4,4)], last event Finished{Completed}
// with verify:    [...,(Verify,5,5)] and every steps == 5
// with an empty tree: the same four StageStarted events still appear in order
```

### CI MSRV job (D-16)

```yaml
  msrv:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.90.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --workspace --all-targets
      - run: cargo test --workspace
```
`dtolnay/rust-toolchain` selects a version through the ref (`@1.89.0` style) [CITED: github.com/dtolnay/rust-toolchain README]; the `1.90.0` and `1.90` refs both exist [VERIFIED: `gh api repos/dtolnay/rust-toolchain/branches/1.90.0` this session]. The job inherits the workflow-level `RUSTFLAGS: -D warnings`, which is intended.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `x % n == 0` | `x.is_multiple_of(n)` | `is_multiple_of` stable in Rust 1.87; clippy `manual_is_multiple_of` fires once MSRV ≥ 1.87 | The two hits are in code this phase deletes [VERIFIED: clippy probe] |
| Nested `if let` | `if let … && …` let-chains | Edition 2024, Rust 1.88 [ASSUMED] | Usable now that MSRV is 1.90; clippy pedantic may suggest collapsing |
| `fs2`/`fs4` file locks | `std::fs::File::lock` | Rust 1.89 [CITED: .planning/research/ARCHITECTURE.md] | Not needed this phase; unblocked for Phase 2 |
| signal-hook `cleanup` module | `flag::register_conditional_shutdown` pattern | signal-hook 0.2+ (doc in 0.4.4 lib.rs) [VERIFIED: crate source] | Use the flag pattern |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | serde's `PathBuf` `Serialize` errors on non-UTF-8 paths with "path contains invalid UTF-8 characters" | Anti-Patterns | Low: recommendation (use `String`) is safe either way |
| A2 | `let`-chains are stable on edition 2024 from Rust 1.88 | Code Examples, State of the Art | Low: if not, write nested `if let`; the MSRV CI job would catch it |
| A3 | Cargo resolver 3 prefers MSRV-compatible dependency versions when adding deps | Standard Stack | Low: signal-hook 0.4.4 declares 1.66 anyway |
| A4 | `Swatinem/rust-cache@v2` includes the rustc version in its cache key | Runtime State Inventory | Low: worst case a cold cache in the MSRV job |
| A5 | Reading sparse files on APFS is fast enough that a generated tree of large sparse files keeps `twins scan` busy for several seconds (for the `#[ignore]` SIGINT test) | Validation Architecture | Low: only affects an ignored, manual test; increase file count/size if it finishes too quickly |
| A6 | Size-grouping progress line wording (`[2/4] size grouping  5 000 candidates`) | Pattern 7 | Low: cosmetic; not in the approved examples, confirm during verification |
| A7 | Digit grouping uses a plain ASCII space (copied from the approved examples), not U+202F | Pattern 7 | Low: cosmetic |

## Open Questions

1. **Clippy on 1.90 vs D-16.**
   - What we know: success criterion 5 says "passes tests and clippy (pedantic) on Rust 1.90"; D-16 says the MSRV job runs build and test, and the stable job runs clippy. Stable clippy honours `rust-version = "1.90"` for MSRV-gated lints (verified by the probe).
   - What's unclear: whether criterion 5 needs a clippy run on the 1.90 toolchain itself.
   - Recommendation: follow D-16 literally in CI. During phase verification, run `cargo +1.90.0 clippy --workspace --all-targets -- -D warnings` once locally (after `rustup toolchain install 1.90.0 --profile minimal --component clippy`) and record the result. If it is noisy because the older clippy lacks newer allow names, record that and rely on stable clippy.

2. **Should `--jobs` keep driving both pools?**
   - What we know: today `args.jobs` goes to `scan::Options::workers` and `group::Options::workers` [VERIFIED: run.rs:29,45].
   - Recommendation: keep both (parity). `ScanSpec` carries `hash_workers` separately so the app can tune them later.

3. **SIGTERM handling.**
   - Not required by Phase 1. Registering `flag::register(SIGTERM, flag)` would make `kill` cancel gracefully too; leave it for the agent phase unless trivial to add alongside SIGINT.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc / cargo (stable) | build, tests | ✓ | 1.98.1 | — |
| clippy (stable) | lint gate | ✓ | bundled with 1.98.1 | — |
| rustup | installing the 1.90 toolchain locally | ✓ | 1.28.2 | — |
| Rust 1.90.0 toolchain | local MSRV check (optional) | ✗ (only `stable-aarch64-apple-darwin` installed) | — | CI MSRV job; or `rustup toolchain install 1.90.0 --profile minimal` |
| `gh` CLI | none required | ✓ | — | — |
| `kill` (for the ignored SIGINT test) | CLI cancel test | ✓ (macOS base system) | — | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** Rust 1.90.0 toolchain locally (CI covers it).

Baseline: `cargo test --workspace` passes on stable today (6 CLI tests, 52 core tests across 8 files) [VERIFIED: run this session].

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]`; `assert_cmd` 2.2.2 + `predicates` 3 + `tempfile` 3.27.0 for the CLI |
| Config file | none (Cargo defaults) |
| Quick run command | `cargo test -p twins-core --test pipeline_test --test observe_test` |
| Full suite command | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CORE-01 | Scan/report output (groups, schema v1 JSON, text) unchanged after the move | characterization (CLI) | `cargo test -p twins-cli --test cli_test` | ✅ file; ❌ new full-JSON/text characterization tests (Wave 0, written against current code first) |
| CORE-01 | Pipeline gives the same groups/report as the manual walk+find+plan wiring | integration (core) | `cargo test -p twins-core --test pipeline_test` | ❌ Wave 0 |
| CORE-01 | Stage events in order walk → size grouping → partial → full (→ verify, steps=5), also for an empty tree; `Finished` last | integration (core) | `cargo test -p twins-core --test pipeline_test stage` | ❌ Wave 0 |
| CORE-01 | Throttle forwards non-progress immediately, limits progress, forwards `done == total` | unit | `cargo test -p twins-core --test observe_test` | ❌ Wave 0 |
| CORE-01 | `Event` JSON shape (`{"type":"stageStarted","stage":"partialHash","step":3,"steps":4}`) | unit | `cargo test -p twins-core --test observe_test json` | ❌ Wave 0 |
| CORE-01 | `group_digits(48210) == "48 210"`; progress line formatter output | unit | `cargo test -p twins-core --test human_test`; `cargo test -p twins-cli --bin twins` | ❌ Wave 0 |
| CORE-01 | `--json` with non-TTY stderr prints nothing on stderr | CLI | `cargo test -p twins-cli --test cli_test json_stderr` | ❌ Wave 0 |
| CORE-02 | Cancel raised at each `StageStarted` (Walk, PartialHash, FullHash, Verify) → `PipelineError::Cancelled`, no later stage, `Finished{Cancelled}` | integration (core) | `cargo test -p twins-core --test pipeline_test cancel` | ❌ Wave 0 |
| CORE-02 | Walk returns `ScanError::Cancelled` with a pre-set flag (no test exists today) | unit | `cargo test -p twins-core --test scan_test cancel` | ❌ Wave 0 |
| CORE-02 | `hash::full_cancellable` / `equal_cancellable` stop with a pre-set flag and match `full`/`equal` otherwise | unit | `cargo test -p twins-core --test hash_test cancel` | ❌ Wave 0 |
| CORE-02 | Interrupted hash is not reported through `on_error` | unit | `cargo test -p twins-core --test group_test cancel` | ❌ Wave 0 |
| CORE-02 | Exit-code mapping: cancelled → 130, `/System` → 2 | CLI | `cargo test -p twins-cli --test cli_test exit_code` | ❌ Wave 0 (130 via unit test on the mapping fn; 2 via process) |
| CORE-02 | Real SIGINT on a large sparse tree: exit 130 within ~1 s, stderr `scan cancelled\n`, stdout empty | CLI, `#[ignore]` | `cargo test -p twins-cli --test cli_test -- --ignored sigint` | ❌ Wave 0 |
| CORE-02 | Second Ctrl+C exits immediately; progress visible with `--json` on a TTY | manual | run `twins scan ~` in a terminal | manual-only (needs a TTY and a human) |
| CORE-03 | `RunMode::DryRun` → `report.dry_run == true`; `RunMode::Scan` → `false` | integration (core) | `cargo test -p twins-core --test pipeline_test dry_run` | ❌ Wave 0 |
| CORE-03 | CLI no longer builds `Meta` or mentions `dry_run` | static | `! grep -rnE "dry_run|Meta \{" crates/twins-cli/src` | n/a |
| CORE-04 | Workspace builds/tests on 1.90 | CI | `cargo +1.90.0 test --workspace` (CI `msrv` job) | ❌ CI job (Wave 0/1) |

### Sampling Rate
- **Per task commit:** `cargo test -p twins-core` (≈1 s) or the touched crate's tests, plus `cargo clippy --workspace --all-targets -- -D warnings`
- **Per wave merge:** full suite command above
- **Phase gate:** full suite green; `cargo test -p twins-cli --test cli_test -- --ignored`; one manual terminal run checking progress, `--json` progress, single and double Ctrl+C; MSRV CI job green

### Wave 0 Gaps
- [ ] `crates/twins-cli/tests/cli_test.rs`: full-structure JSON characterization (mask `scanned_at`, compare everything else incl. `roots` with a **relative** root, `keep`, `remove`, `files`, `summary`) and exact text-output characterization, **committed and green before** `run.rs` changes
- [ ] `crates/twins-cli/tests/cli_test.rs`: `.code(2)` for `/System`, bad `--min-size`, bad `--exclude` (locks exit codes before the error-type change)
- [ ] `crates/twins-core/tests/scan_test.rs`: walk cancellation with a pre-set flag
- [ ] `crates/twins-core/tests/pipeline_test.rs`, `observe_test.rs`: new files (reuse `fixtures/mod.rs` `Tree`, `mib`)
- [ ] `.github/workflows/ci.yml`: `msrv` job
- Framework install: none needed

## Security Domain

`security_enforcement` is enabled (ASVS level 1). This phase adds no network, auth, storage or deletion; the attack surface is local process input (argv, signals) and terminal output.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no (protected-root checks unchanged; identity-based fix is Phase 2, SAFE-01/02) | `safety::is_protected` unchanged |
| V5 Input Validation | yes | clap parsing; `parse_size`; glob compile errors → exit 2 (unchanged) |
| V6 Cryptography | no (BLAKE3 used as a content fingerprint, unchanged) | `blake3` crate |
| V7 Error Handling & Logging | yes | Errors to stderr only; cancel leaves no partial stdout; `FileSkipped` paths printed only with `--verbose` |
| V14 Configuration | yes | MSRV pinned in CI; no new `unsafe` |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Partial report consumed by a script after Ctrl+C (`twins report > out.json`) | Tampering / integrity | Nothing written to stdout unless the pipeline returns `Ok`; exit 130 lets scripts detect it |
| Terminal escape injection via file names in progress/verbose lines | Tampering (terminal) | Progress lines contain only counts and fixed labels; verbose `skip` lines print paths as today (pre-existing behaviour, unchanged; note for a later hardening pass) |
| Unsafe signal handler code | Elevation / DoS | signal-hook's safe `flag` API; no hand-written `sigaction`; core installs no handlers |
| Resource exhaustion from per-file events (app later) | DoS | `Throttle` at 80 ms in core |

## Sources

### Primary (HIGH confidence)
- In-repo source read this session: `crates/twins-cli/src/{main.rs,cli.rs,run.rs}`, `crates/twins-core/src/{lib.rs,hash.rs,human.rs,report.rs}`, `crates/twins-core/src/scan/{mod.rs,options.rs,walk.rs}`, `crates/twins-core/src/group/{mod.rs,find.rs,hasher.rs,index.rs,keep.rs}`, tests `cli_test.rs`, `group_test.rs`, `scan_test.rs`, `report_test.rs`, `human_test.rs`, `fixtures/mod.rs`, `Cargo.toml`, `.github/workflows/ci.yml`
- `cargo metadata` (dependency `rust_version` fields), `cargo test --workspace`, clippy probe at `rust-version = "1.90"` on a scratch copy
- signal-hook 0.4.4 crate source (`src/flag.rs`, `src/lib.rs`, `src/low_level/mod.rs`, `Cargo.toml`)
- ctrlc 3.5.2 crate source (`src/lib.rs`, `Cargo.toml`)
- crates.io API: signal-hook, signal-hook-registry, ctrlc, nix, dispatch2, objc2 versions/MSRV
- `gsd-tools query package-legitimacy check --ecosystem crates ctrlc signal-hook`

### Secondary (MEDIUM confidence)
- serde.rs/enum-representations.html (internally tagged restrictions)
- serde.rs/container-attrs.html (`rename_all` vs `rename_all_fields`)
- github.com/dtolnay/rust-toolchain README (version selection) + `gh api` branch check
- `.planning/research/ARCHITECTURE.md` Patterns 1 and 4, `.planning/research/STACK.md`, `.planning/research/PITFALLS.md` #15, `.planning/codebase/CONCERNS.md`

### Tertiary (LOW confidence)
- Items A1-A7 in the Assumptions Log

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. One new crate, source read, MSRV and legitimacy checked.
- Architecture: HIGH. Follows the locked Pattern 1; every hook point was located in the current source.
- Pitfalls: HIGH for the code-derived ones (1-5, 7); MEDIUM for test-timing (8).

**Research date:** 2026-10-02
**Valid until:** 2026-11-01 (stable domain; recheck signal-hook version at execution time)

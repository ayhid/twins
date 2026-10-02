# Phase 1: Core Pipeline Refactor - Pattern Map

**Mapped:** 2026-10-02
**Files analyzed:** 22 (5 new source, 11 modified source, 2 new tests, 4 extended tests, plus config/CI)
**Analogs found:** 21 / 22 (all analog paths verified git-tracked with `git ls-files`)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/twins-core/src/observe.rs` (NEW) | model + utility (CancelToken, Stage, Event, Outcome, Observer, Throttle) | event-driven | `crates/twins-core/src/group/find.rs` (Stage/Progress/callbacks, lines 21-51, 75-86, 170-183) + `crates/twins-core/src/report.rs` (serde derives, lines 8, 33-40) | role-match |
| `crates/twins-core/src/pipeline.rs` (NEW) | service (orchestrator) | batch / request-response | `crates/twins-cli/src/run.rs` lines 18-77 (the code being moved) | exact (logic) |
| `crates/twins-core/src/lib.rs` (MOD) | config (module list) | n/a | itself | exact |
| `crates/twins-core/src/human.rs` (MOD, + `group_digits`) | utility | transform | `human_size` in same file, lines 7-21 | exact |
| `crates/twins-core/src/hash.rs` (MOD, + `full_cancellable`/`equal_cancellable`) | utility | file-I/O streaming | `hash::full` / `hash::equal`, lines 103-140 | exact |
| `crates/twins-core/src/group/hasher.rs` (MOD, + defaulted `full_cancellable`) | trait/service | file-I/O | same file, lines 7-34 | exact |
| `crates/twins-core/src/group/find.rs` (MOD, stage-start marker, cancel re-check, verify polls) | service | batch (parallel) | same file, lines 198-227, 283-338 | exact |
| `crates/twins-core/src/scan/walk.rs` (MOD, `walk_observed`, cancel in stat phase) | service | file-I/O batch | same file, lines 77-111, 137-144 | exact |
| `crates/twins-core/src/scan/options.rs` (maybe MOD, read-only use of `cancel`) | config/builder | n/a | same file, lines 115-123 | exact |
| `crates/twins-cli/src/run.rs` (MOD, hollowed out) | controller (shell) | request-response | itself | exact |
| `crates/twins-cli/src/progress.rs` (NEW) | component (TerminalObserver + pure `line()` formatter) | event-driven | `run.rs` `Progress` struct, lines 98-141 | exact (replaced) |
| `crates/twins-cli/src/main.rs` (MOD, 130 exit path) | controller (entry) | request-response | itself, lines 22-28; `run::exit_code` run.rs 143-155 | exact |
| `crates/twins-cli/Cargo.toml` (MOD, + signal-hook) | config | n/a | itself | exact |
| `Cargo.toml` (MOD, `rust-version = "1.90"`) | config | n/a | itself line 10 | exact |
| `.github/workflows/ci.yml` (MOD, + msrv job) | config | n/a | existing `test` job | exact |
| `crates/twins-core/tests/observe_test.rs` (NEW) | test | unit | `crates/twins-core/tests/human_test.rs`, `group_test.rs` | role-match |
| `crates/twins-core/tests/pipeline_test.rs` (NEW) | test | integration | `crates/twins-core/tests/group_test.rs` (SpyHasher, Mutex-collected progress, lines 102-134, 203-222) + `scan_test.rs` lines 1-23 | role-match |
| `crates/twins-core/tests/hash_test.rs` (EXT) | test | unit | itself | exact |
| `crates/twins-core/tests/group_test.rs` (EXT) | test | unit | itself | exact |
| `crates/twins-core/tests/scan_test.rs` (EXT, cancel test) | test | unit | itself, `visited()` lines 12-23 | exact |
| `crates/twins-core/tests/human_test.rs` (EXT) | test | unit | itself | exact |
| `crates/twins-cli/tests/cli_test.rs` (EXT, characterization, exit codes, `#[ignore]` SIGINT) | test | e2e | itself, lines 9-13, 59-89, 139-158 | exact |
| signal-hook SIGINT registration (inside `run.rs`/`main.rs`) | utility | event-driven | none in codebase | no analog |

## Pattern Assignments

### `crates/twins-core/src/pipeline.rs` (service, batch)

**Analog:** `crates/twins-cli/src/run.rs` lines 18-77 — move this body nearly verbatim, replacing printing with observer events and `dry_run: false` with `spec.mode.is_dry_run()`.

**Spec building (becomes `ScanSpec` fields / CLI builder)** — run.rs 20-29:
```rust
let roots = roots(&args.paths)?;
let min_size = parse_size(&args.min_size)?;
let opts = scan::Options::new(roots.clone())
    .min_size(min_size)
    .include_empty(args.include_empty)
    .include_library(args.include_library)
    .include_node_modules(args.include_node_modules)
    .include_remote(args.include_remote)
    .exclude(args.exclude.clone())
    .workers(args.jobs);
```
`roots()` (run.rs 79-87, `$HOME` fallback) stays in the CLI. `--jobs` feeds both `scan::Options::workers` and `group::Options::workers` (keep both; `ScanSpec.hash_workers`).

**Core orchestration to move** — run.rs 33-65:
```rust
let idx = Index::new();
let stats = scan::walk(&opts, |m| { idx.add(m); progress.tick("walk", None); },
    |path, err| skip(&stderr, args.verbose, path, err))?;
let errors = AtomicU64::new(stats.errors);
let find_opts = group::Options::default()
    .workers(args.jobs)
    .verify(args.verify)
    .on_progress(Box::new(|p| progress.stage(p)))
    .on_error(Box::new(|path, err| { errors.fetch_add(1, Ordering::Relaxed); skip(...); }));
let groups = group::find(&idx, &find_opts)?;
drop(find_opts);
let keeper = Keeper::new(Strategy::default(), None);
let actions = group::plan(&groups, &keeper);
let meta = Meta { roots, files: stats.files, candidates: stats.candidates,
    strategy: keeper.strategy(), dry_run: false };
let r = report::build(&actions, &meta, SystemTime::now());
```
In the pipeline: `walk` → `scan::walk_observed` with `.cancel(cancel.flag())` on the options; `skip` → `Event::FileSkipped { path: path.to_string_lossy().into_owned(), reason: err.to_string() }`; `on_progress` → map `done == 0` to `StageStarted`, else `Progress`; `group::Options::cancel(cancel.flag())`; explicit `cancel.is_cancelled()` checks after walk and after find (Pitfall 3). `Meta.roots = spec.walk.roots().to_vec()` (accessor at `scan/options.rs` 44-48) — never the normalised roots. `report::build` moves to `ScanOutcome::report(now)`; `SystemTime::now()` stays in the shell.

**Error enum pattern** — copy `FindError` shape (find.rs 64-73):
```rust
/// Fatal error of [`find`].
#[derive(Debug, thiserror::Error)]
pub enum FindError {
    /// The run was cancelled through [`Options::cancel`].
    #[error("scan cancelled")]
    Cancelled,
    /// The worker pool could not be created.
    #[error("thread pool: {0}")]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}
```
`PipelineError { Scan(ScanError), Find(FindError), Cancelled }` with hand-written `From` impls that collapse `ScanError::Cancelled` (scan/mod.rs 44-46) and `FindError::Cancelled` into `PipelineError::Cancelled` (do NOT use `#[from]`, it would keep the inner Cancelled). Add `#[must_use] pub fn is_cancelled(&self) -> bool`.

**Builder pattern for `ScanSpec`** — copy `scan::Options` (options.rs 10-55, 115-123):
```rust
#[must_use]
pub fn min_size(self, min_size: u64) -> Self {
    Self { min_size, ..self }
}
/// Flag polled during the walk; setting it aborts with
/// [`super::ScanError::Cancelled`].
#[must_use]
pub fn cancel(self, cancel: Arc<AtomicBool>) -> Self {
    Self { cancel: Some(cancel), ..self }
}
```
Private fields + `#[must_use]` accessors (`roots()` at options.rs 44-48 is the accessor model).

---

### `crates/twins-core/src/observe.rs` (model/utility, event-driven)

**Analog:** `crates/twins-core/src/group/find.rs` 21-51 (Stage enum + Display + Progress struct) and report.rs serde usage.

**Stage enum + Display** (find.rs 21-40) — copy for `observe::Stage` (add `Walk`, `SizeGrouping`; labels "walk", "size grouping", "partial hash", "full hash", "verify"):
```rust
/// Pipeline step, for progress reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage { Partial, Full, Verify }
impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Partial => "partial hash",
            Self::Full => "full hash",
            Self::Verify => "verify",
        })
    }
}
```
Add `Serialize` + `#[serde(rename_all = "camelCase")]`, and `impl From<group::Stage> for Stage`.

**Serde import** (report.rs 8): `use serde::{Deserialize, Serialize};` — Event needs only `Serialize`, with `#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]`, struct variants only, `path: String` (not `PathBuf`; report.rs already uses lossy strings, `report::display` ~line 140).

**Callback / Send+Sync convention** (find.rs 75-76): `Box<dyn Fn(Progress) + Send + Sync + 'a>` — the `Observer` trait mirrors it: `pub trait Observer: Send + Sync { fn on_event(&self, event: &Event); }` (same shape as `Hasher: Send + Sync`, hasher.rs 8).

**Atomic flag pattern** (find.rs 9-10, 170-174):
```rust
fn cancelled(&self) -> bool {
    self.cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed))
}
```
`CancelToken(Arc<AtomicBool>)` with `new`, `cancel`, `is_cancelled`, `flag() -> Arc<AtomicBool>` feeds the existing `scan::Options::cancel` and `group::Options::cancel` setters unchanged.

**Throttle:** no analog; follow RESEARCH Pattern 2 (`AtomicU64` nanos since base `Instant`, `compare_exchange`, 80 ms; pass-through for StageStarted/FileSkipped/Finished and `total == Some(done)`).

---

### `crates/twins-core/src/hash.rs` (utility, file-I/O streaming)

**Analog:** same file, `full` lines 103-119 and `equal` 121-140.
```rust
pub fn full(path: &Path) -> Result<Digest, HashError> {
    let mut f = File::open(path).map_err(err("open", path))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        let n = f.read(&mut buf).map_err(err("read", path))?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(Digest(*hasher.finalize().as_bytes()))
}
```
New `full_cancellable(path, cancel: &AtomicBool)`: same loop with `if cancel.load(Ordering::Relaxed) { return Err(HashError { op: "cancelled", path: path.to_path_buf(), source: io::ErrorKind::Interrupted.into() }) }` at the top of each iteration. Make `full` = `full_cancellable(path, &AtomicBool::new(false))`; same for `equal`/`equal_cancellable`. Keep `/// # Errors` doc sections in the same style.

---

### `crates/twins-core/src/group/hasher.rs` (trait)

**Analog:** same file lines 7-34. Add a defaulted trait method after `full` (so `SpyHasher` in `tests/group_test.rs` 118-134 compiles unchanged), and override in `DirectHasher`:
```rust
fn full(&self, m: &FileMeta) -> Result<Digest, HashError> {
    hash::full(m.path())
}
```
→ `fn full_cancellable(&self, m, cancel) -> ... { hash::full_cancellable(m.path(), cancel) }`.

---

### `crates/twins-core/src/group/find.rs` (service, parallel batch)

**Analog:** same file.
- Stage-start marker: in `compute_keys` (lines 283-305) emit `opts.progress(stage, 0, total)` before `reps.par_iter()`; in `find_in_pool` (lines 207-212) emit `opts.progress(Stage::Verify, 0, total)` before the loop when `opts.verify`. Doc it on `Progress` (lines 42-51).
- Cancel re-check after `let r = key(m);` (line 301): return `None` if `opts.cancelled()` so interrupted hashes never hit `opts.report` (line 313).
- Full stage: line 205 `|m| opts.hasher.full(m)` → `full_cancellable(m, flag)` (needs a never-set `AtomicBool` when `opts.cancel` is `None`).
- `verify` (lines 320-338): poll `opts.cancelled()` per file, use `hash::equal_cancellable`; after verify, `if opts.cancelled() { return Err(FindError::Cancelled) }`.
- `reports_progress_per_stage` (group_test.rs 203-222) uses `rfind`, keeps passing.

---

### `crates/twins-core/src/scan/walk.rs` (service, file-I/O batch)

**Analog:** same file. Rename body of `walk` (lines 77-111) to `pub(crate) fn walk_observed<V, E, P>(..., on_progress: P)` with `P: Fn(&Stats) + Sync`; `walk` delegates with `|_| {}`. Keep doc + `# Errors` (lines 69-76).
Cancel poll to copy (lines 137-144):
```rust
if opts.cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
    return Err(ScanError::Cancelled);
}
```
Apply it inside the `files.par_iter().for_each` closure (lines 104-108, early return) and after `pool.install`, plus once per root before `collect_files`. Call `on_progress(&counters.snapshot())` where `counters.files` / `counters.candidates` are bumped (`Counters::snapshot`, lines 47-55).

---

### `crates/twins-core/src/human.rs` (utility, transform)

**Analog:** `human_size` lines 7-21:
```rust
/// Renders a byte count such as `1.5 MiB`.
#[must_use]
pub fn human_size(n: u64) -> String {
```
Add `/// Renders 48210 as `48 210`.` `#[must_use] pub fn group_digits(n: u64) -> String` (plain ASCII space). Update module doc (line 1) since it no longer covers only sizes.

---

### `crates/twins-cli/src/progress.rs` (component, event-driven)

**Analog:** `run.rs` `Progress` lines 98-141 (deleted by this phase; it holds the two `manual_is_multiple_of` hits at 114/120).
```rust
fn line(phase: &str, done: u64, total: Option<u64>) {
    let mut w = std::io::stderr().lock();
    let _ = match total {
        Some(t) => write!(w, "\r\x1b[K{phase}: {done}/{t}"),
        None => write!(w, "\r\x1b[K{phase}: {done} files"),
    };
    let _ = w.flush();
}
fn finish(&self) {
    if self.enabled {
        let mut w = std::io::stderr().lock();
        let _ = write!(w, "\r\x1b[K");
        let _ = w.flush();
    }
}
```
New: pure `fn line(step, steps, stage, done, total) -> String` producing `[1/4] walk  48 210 files` / `[3/4] partial hash  1 234 / 5 000` (unit-tested in `#[cfg(test)] mod tests`); `TerminalObserver { enabled: stderr().is_terminal() /* regardless of --json, D-09 */, verbose, current: Mutex<(u8,u8,Stage)> }`; clear on `Finished`; verbose skip text copies run.rs 89-96:
```rust
if let Ok(mut w) = stderr.lock() {
    let _ = writeln!(w, "skip {}: {err}", path.display());
}
```
(clear line with `\r\x1b[K` before writing it). Wrap in `twins_core::observe::Throttle::new(.., Duration::from_millis(80))`.

---

### `crates/twins-cli/src/run.rs` (controller, request-response)

**Analog:** itself. Keep `roots()` (79-87), `describe()` (157-163), output block (66-75):
```rust
let mut out = std::io::stdout().lock();
if json {
    report::write_json(&mut out, &r)?;
} else {
    report::write_text(&mut out, &r)?;
    let n = errors.load(Ordering::Relaxed);
    if n > 0 && !args.verbose {
        eprintln!("{n} files could not be read (use --verbose to list them)");
    }
}
```
`n` becomes `outcome.errors()`. Delete `Meta`, `Index`, `Keeper`, `group::find`, `scan::walk` usage (grep gate: `! grep -rnE "dry_run|Meta \{" crates/twins-cli/src`).
Extend `exit_code` (143-155) by downcasting `PipelineError`: `Cancelled` → 130, `Scan(_)` → 2, `Find(_)` → 1; keep `ParseSizeError` → 2.

### `crates/twins-cli/src/main.rs` (entry)

**Analog:** itself lines 22-28:
```rust
match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
        eprintln!("twins: {}", run::describe(&err));
        ExitCode::from(u8::try_from(run::exit_code(&err)).unwrap_or(1))
    }
}
```
Insert a guard arm before the generic one: cancelled → `eprintln!("scan cancelled"); ExitCode::from(130)` (no `twins: ` prefix, nothing on stdout). Add `mod progress;` next to `mod cli; mod run;` (lines 3-4).

---

### Config: `Cargo.toml`, `crates/twins-cli/Cargo.toml`, `.github/workflows/ci.yml`

- `Cargo.toml` line 10: `rust-version = "1.85"` → `"1.90"`. Add `signal-hook = { version = "0.4.4", default-features = false }` to `[workspace.dependencies]` (alphabetical, like the existing list) and `signal-hook.workspace = true` under `[dependencies]` in `crates/twins-cli/Cargo.toml` (matches `anyhow.workspace = true` style).
- CI: duplicate the existing `test` job shape (`runs-on: macos-latest`, checkout@v4, `dtolnay/rust-toolchain@…`, `Swatinem/rust-cache@v2`) as `msrv` with `dtolnay/rust-toolchain@1.90.0`, steps `cargo build --workspace --all-targets` and `cargo test --workspace`.

---

### Tests

**`crates/twins-core/tests/pipeline_test.rs` / `observe_test.rs`** — header and fixtures as in `scan_test.rs` 1-10:
```rust
//! Tests for the directory walk and its exclusion rules.
mod fixtures;
use std::sync::Mutex;
use fixtures::{Tree, mib};
use twins_core::scan::{self, Options, ScanError};
```
Collect events with a `Mutex<Vec<_>>` like group_test.rs 206-214 (`seen.lock().unwrap().push(p)` then `into_inner()`). Test-only trait impl modelled on `SpyHasher` (group_test.rs 102-134) → `CancelOn` / recording `Observer`. Use `.home(t.root().into())` on `scan::Options` as other tests do. Parity test: compare `pipeline::scan` actions vs manual `scan::walk`+`group::find`+`group::plan`.

**`crates/twins-core/tests/scan_test.rs`** — new cancel test: build `Options::new(..).cancel(Arc::new(AtomicBool::new(true)))`, call `scan::walk` and assert `matches!(err, ScanError::Cancelled)` (cf. `visited()` lines 12-23).

**`crates/twins-cli/tests/cli_test.rs`** — use `twins()` (lines 9-13) and `fixture()` (19-34); characterization test pattern lines 59-89 (parse stdout as `serde_json::Value`, assert fields; mask `scanned_at`). Exit-code test: extend `protected_root_and_bad_flags_fail_with_a_message` (139-158) style with `.code(2)`. `--json` non-TTY: `.assert().success().stderr("")`. SIGINT test `#[ignore]`.

## Shared Patterns

### Doc comments + `# Errors`
**Source:** `crates/twins-core/src/group/find.rs` 185-191; `hash.rs` 103-107
**Apply to:** every new pub item in `observe.rs`, `pipeline.rs`, `hash.rs`, `human.rs`, `walk.rs` (`missing_docs` warns, CI `-D warnings`).
```rust
/// Runs the pipeline over the index and returns duplicate groups sorted by
/// reclaimable space. ...
///
/// # Errors
/// [`FindError::Cancelled`] when the cancel flag is raised.
pub fn find(idx: &Index, opts: &Options<'_>) -> Result<Vec<Group>, FindError> {
```

### Module exposure
**Source:** `crates/twins-core/src/lib.rs` (`pub mod` list, alphabetical) and `group/mod.rs` 5-13 (private `mod` + `pub use`)
**Apply to:** `lib.rs` gets `pub mod observe;` and `pub mod pipeline;` in alphabetical order. `walk_observed` stays `pub(crate)` (not re-exported in `scan/mod.rs` line 10).

### Cancellation polling
**Source:** `scan/walk.rs` 137-144, `group/find.rs` 170-174
**Apply to:** walk stat phase, hash chunk loops, find verify loop, pipeline between stages. Always `Ordering::Relaxed` for loads.

### Error enums
**Source:** `scan/mod.rs` 16-47, `group/find.rs` 64-73 — `#[derive(Debug, thiserror::Error)]`, doc per variant, `#[error("scan cancelled")]` text reused for `PipelineError::Cancelled`.

### Justified allows
**Source:** `scan/options.rs` 12 `#[allow(clippy::struct_excessive_bools)] // independent opt-in switches`; `human.rs` 13 `#[allow(clippy::cast_precision_loss)] // display only`
**Apply to:** any lint suppression needed after the 1.90 bump (D-17).

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| SIGINT wiring (signal-hook in `twins-cli`) | utility | event-driven | No signal handling exists; use RESEARCH Pattern 6 (`register_conditional_shutdown(SIGINT, 130, flag)` THEN `register(SIGINT, flag)`) |
| `Throttle<O>` in `observe.rs` | utility | event-driven | No rate limiting exists; use RESEARCH Pattern 2 |

## Metadata

**Analog search scope:** `crates/twins-core/src/**`, `crates/twins-cli/src/**`, `crates/*/tests/**`, `Cargo.toml`, `.github/workflows/ci.yml`
**Files scanned:** 20
**Note:** `group::Strategy` (keep.rs 13) derives no `Serialize`; nothing in this phase needs it to.
**Pattern extraction date:** 2026-10-02

# Phase 1: Core Pipeline Refactor - Context

**Gathered:** 2026-10-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Move scan orchestration (walk → size index → partial hash → full hash → optional verify → plan → report) out of `crates/twins-cli/src/run.rs` into `twins-core`. Any caller can then follow staged progress through an observer and cancel through a token. The phase also fixes the hardcoded `meta.dry_run` and raises the minimum Rust version to 1.90. `twins scan` and `twins report` output (groups, schema v1 JSON) must stay identical. There is no deletion, `clean`, cache, config or app work in this phase.

Requirements: CORE-01, CORE-02, CORE-03, CORE-04.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from research (not re-discussed)
- **D-01:** Core API follows `.planning/research/ARCHITECTURE.md` Pattern 1:
  - a `twins_core::observe` module with `CancelToken` (wrapping `Arc<AtomicBool>`), a serializable `Event` enum, an `Observer` trait (`Send + Sync`) and a `Throttle<O>` wrapper;
  - a `twins_core::pipeline::scan(spec, &observer, &cancel)` entry point.
  
  Shells only render events. Core never depends on Tokio, and the rayon pools stay as they are. — **Reversibility:** costly — Phase 2 (clean), Phase 5 (Tauri `Channel<Event>`) and Phase 6/8 (runner, agent) all build on this API.

### Ctrl+C / cancellation
- **D-02:** A cancelled scan exits with code **130** (128+SIGINT). It stays distinct from 1 (I/O) and 2 (usage).
- **D-03:** A **second Ctrl+C** while the first cancel is unwinding exits immediately with 130. This is the escape hatch if a read blocks on a stuck volume. It is safe because a scan writes nothing.
- **D-04:** On cancel the CLI clears the progress line and prints a single `scan cancelled` line on **stderr**. Nothing goes to stdout: no partial text report and no partial JSON.
- **D-05:** Cancellation must stop the walk and every hashing stage promptly, within about a second on a large tree. The existing `group::Options::cancel` / `FindError::Cancelled` and the walk's cancel option are driven by the one `CancelToken`.

### Progress display
- **D-06:** Single-line stderr progress with a **step counter**, for example `[1/4] walk  48 210 files`, `[3/4] partial hash  1 234 / 5 000`. Numbers use the existing human formatting.
- **D-07:** **Size grouping is a real stage**: core emits its event (with the candidate count), so observers and tests see walk → size grouping → partial hash → full hash in order. The CLI may show it only briefly.
- **D-08:** With `--verify`, byte comparison is a **fifth step** (`[5/5] verify`), and the step total becomes 5 so the counter stays honest.
- **D-09:** Progress shows whenever **stderr is a TTY, including with `--json`**, because stdout stays pure JSON. This changes today's behavior, where `--json` silenced progress. Progress stays silent when stderr is not a TTY.
- **D-10:** A test observer receives the same stage events in the same order as the terminal observer (success criterion 2).

### dry_run semantics
- **D-11:** `meta.dry_run` means "this report describes a clean that changed nothing". A plain `twins scan` / `twins report` reports **`false`**, which keeps the output byte-identical to today and matches Go v0, where the flag came from `clean --dry-run`.
- **D-12:** Core carries an explicit **run mode** (for example a `RunMode` enum covering scan and dry-run, extended by Phase 2 for real cleans) on the pipeline spec. `meta.dry_run` is **derived from it**, never hardcoded in a shell. — **Reversibility:** costly — Phase 2's `clean` and `--dry-run` path will build on this enum.
- **D-13:** **No schema change.** Schema v1 stays as is, with no `meta.mode` field in this phase; an explicit mode field can come with `clean` in Phase 2.
- **D-14:** The wiring is proven by **core-level tests**: running the pipeline in dry-run mode on a temp tree must give `meta.dry_run == true`, and a scan must give `false`. No hidden CLI `--dry-run` flag. A CLI-level dry-run test follows in Phase 2.

### Toolchain & CI
- **D-15:** Set `rust-version = "1.90"` in `[workspace.package]`. **No `rust-toolchain.toml`**, so local development stays on stable.
- **D-16:** CI keeps the existing stable job (fmt, clippy `-D warnings`, test, release build) and **adds an MSRV job on 1.90** that runs build and test.
- **D-17:** New lints surfaced by 1.90 or a newer clippy pedantic get **fixed in code**. Use a targeted `#[allow]` with a justifying comment only where a fix would hurt clarity, per project conventions.

### Claude's Discretion
- Signal plumbing for Ctrl+C (the `ctrlc` crate vs `signal-hook`). The only constraint is that core sees nothing but a `CancelToken`.
- The exact `Event` variant set for this phase (scan events only vs also stubbing the clean-related variants from research), the throttle interval (research suggests about 80 ms), and the exact module layout (`observe.rs`, `pipeline.rs`).
- How the walk exposes per-file progress to the observer, as long as the step-counter output above is achievable.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope
- `.planning/ROADMAP.md` § "Phase 1: Core Pipeline Refactor": goal and the 5 success criteria
- `.planning/REQUIREMENTS.md`: CORE-01 to CORE-04
- `.planning/PROJECT.md`: Core Value (never lose data), constraints, and the "core holds all logic, shells are thin" architecture

### Architecture research
- `.planning/research/ARCHITECTURE.md` § "Pattern 1: Move orchestration into core, keep shells as observers": the `CancelToken` / `Event` / `Observer` / `Throttle` sketch and the `pipeline::scan` signature
- `.planning/research/ARCHITECTURE.md` § "Pattern 4": how the Tauri shell will consume `Event` through `Channel` (a constraint: `Event` must be `Serialize`)
- `.planning/research/SUMMARY.md` § "Phase 1: Core refactor": deliverables, and pitfall #15 (dry-run divergence)
- `.planning/research/PITFALLS.md` #15: dry-run divergence

### Codebase maps
- `.planning/codebase/ARCHITECTURE.md`, `.planning/codebase/CONCERNS.md` (the `meta.dry_run` hardcoded-false concern), `.planning/codebase/TESTING.md`

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/twins-core/src/group/find.rs`: `Stage` enum (Partial, Full, Verify), `Progress { stage, done, total }`, `Options::on_progress` / `on_error` / `cancel: Option<Arc<AtomicBool>>`, `FindError::Cancelled`. Cancellation already exists in core; nothing wires it up.
- `crates/twins-core/src/scan/walk.rs`: `walk(opts, visit, on_error) -> Result<Stats, ScanError>`. `ScanError` already documents cancellation ("invalid roots, bad globs or cancellation").
- `crates/twins-core/src/scan/options.rs`: builder-style `Options` (cancel field to confirm).
- `crates/twins-core/src/report.rs`: `Meta { roots, files, candidates, strategy, dry_run }`, `build`, `write_json`, `write_text`.
- `crates/twins-core/src/group/keep.rs`: `Keeper`, `Strategy`, `group::plan`.

### Established Patterns
- Consuming builder methods with `#[must_use]`, `thiserror` errors named `{Domain}Error`, doc comments on all public items (`missing_docs` warns), clippy pedantic under `-D warnings`.
- Separate rayon pools for the walk (one thread per CPU) and hashing (capped at 8).
- Integration tests use `assert_cmd`, `predicates` and `tempfile` (`crates/twins-cli/tests/`).

### Integration Points
- `crates/twins-cli/src/run.rs::scan` is the code to hollow out. Today it builds `Index`, calls `scan::walk` and `group::find`, then `Keeper::new(Strategy::default(), None)`, `group::plan` and `report::build` with `dry_run: false`. Its `Progress` struct is replaced by a terminal `Observer`.
- `run::exit_code` needs a cancelled → 130 branch.
- `Cargo.toml` `[workspace.package] rust-version` and `.github/workflows/ci.yml` (stable only today).

</code_context>

<specifics>
## Specific Ideas

- Progress line examples the user approved: `[1/4] walk  48 210 files`, `[3/4] partial hash  1 234 / 5 000`, `[5/5] verify` when `--verify` is on.
- Cancel message: exactly one stderr line, `scan cancelled`.

</specifics>

<deferred>
## Deferred Ideas

- An explicit `meta.mode` field in the JSON report: consider it with `twins clean` in Phase 2.
- A CLI-level `--dry-run` end-to-end test: Phase 2, once `clean --dry-run` exists.

</deferred>

---

*Phase: 01-core-pipeline-refactor*
*Context gathered: 2026-10-02*

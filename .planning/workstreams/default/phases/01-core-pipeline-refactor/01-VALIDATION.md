---
phase: "1"
slug: "core-pipeline-refactor"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-10-02"
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `01-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]`; `assert_cmd` 2.2.2 + `predicates` 3 + `tempfile` 3.27.0 for the CLI |
| **Config file** | none (Cargo defaults) |
| **Quick run command** | `cargo test -p twins-core --test pipeline_test --test observe_test` |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` |
| **Estimated runtime** | ~30 seconds (full suite, warm build) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p twins-core` (or the touched crate's tests) plus `cargo clippy --workspace --all-targets -- -D warnings`
- **After every plan wave:** Run the full suite command above
- **Before `/gsd-verify-work`:** Full suite must be green, plus `cargo test -p twins-cli --test cli_test -- --ignored` and the MSRV CI job
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

Task IDs are assigned when the plans are written. Until then, each row maps a requirement to its test.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-xx | — | 0 | CORE-01 | — | N/A | characterization (CLI) | `cargo test -p twins-cli --test cli_test` | ✅ file / ❌ W0 new tests | ⬜ pending |
| 1-xx | — | — | CORE-01 | — | N/A | integration (core) | `cargo test -p twins-core --test pipeline_test` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-01 | — | N/A | unit | `cargo test -p twins-core --test observe_test` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-01 | — | stdout stays pure JSON | CLI | `cargo test -p twins-cli --test cli_test json_stderr` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-02 | — | no partial report after cancel | integration (core) | `cargo test -p twins-core --test pipeline_test cancel` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-02 | — | N/A | unit | `cargo test -p twins-core --test scan_test cancel` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-02 | — | N/A | unit | `cargo test -p twins-core --test hash_test cancel` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-02 | — | exit 130 distinct from 1 and 2 | CLI | `cargo test -p twins-cli --test cli_test exit_code` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-03 | — | report mode derived in core | integration (core) | `cargo test -p twins-core --test pipeline_test dry_run` | ❌ W0 | ⬜ pending |
| 1-xx | — | — | CORE-04 | — | N/A | CI | `cargo +1.90.0 test --workspace` (CI `msrv` job) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/twins-cli/tests/cli_test.rs` — full-structure JSON characterization (mask `scanned_at`; compare `roots` with a relative root, `keep`, `remove`, `files`, `summary`) and exact text-output characterization, committed and green before `run.rs` changes
- [ ] `crates/twins-cli/tests/cli_test.rs` — `.code(2)` for `/System`, bad `--min-size`, bad `--exclude` (locks exit codes before the error-type change)
- [ ] `crates/twins-core/tests/scan_test.rs` — walk cancellation with a pre-set flag
- [ ] `crates/twins-core/tests/pipeline_test.rs`, `crates/twins-core/tests/observe_test.rs` — new files (reuse `fixtures/mod.rs` `Tree`, `mib`)
- [ ] `.github/workflows/ci.yml` — `msrv` job on Rust 1.90

Framework install: none needed.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Second Ctrl+C exits immediately with 130 | CORE-02 | Needs a TTY and a human pressing keys twice | Run `twins scan ~` in a terminal, press Ctrl+C twice quickly |
| Progress shows on stderr with `--json` on a TTY | CORE-01 | Needs a real terminal | Run `twins scan ~ --json > /dev/null` in a terminal and watch stderr |
| Real SIGINT stops within ~1 s on a large tree | CORE-02 | Timing-sensitive; `#[ignore]` test run by hand | `cargo test -p twins-cli --test cli_test -- --ignored sigint` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

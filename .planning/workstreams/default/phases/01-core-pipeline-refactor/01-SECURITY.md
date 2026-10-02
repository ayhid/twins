---
phase: "1"
slug: "core-pipeline-refactor"
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
created: "2026-10-02"
---

# Phase 1 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Filesystem → twins-core | The walk and hashers read user files and metadata | File contents and paths (local, user-owned) |
| twins-core → CLI stdout | The report is the scripting contract (`--json`, schema v1) | Duplicate groups, paths, sizes |
| CLI stderr ↔ terminal | Progress and skip lines go to a TTY | Counts, fixed labels, file names with `--verbose` |
| OS signals → twins-cli | SIGINT raises the core `CancelToken` | Cancel flag only |
| crates.io → build | New dependency `signal-hook` 0.4.4 | Third-party code compiled into the CLI |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-01-01 | Tampering | pipeline::scan report contents | high | mitigate | CLI characterization tests (JSON, text, exit codes) plus the pipeline parity test in `pipeline_test.rs` | closed |
| T-01-02 | Repudiation | report dry-run flag | high | mitigate | Set only from `RunMode` in `pipeline.rs`; `dry_run_in_meta_follows_run_mode` | closed |
| T-01-03 | Information disclosure | Event::FileSkipped paths | low | accept | See Accepted Risks | closed |
| T-01-04 | Denial of service | cancellation at hashing stage boundaries | medium | mitigate | `cancel_at_partial_hash_*`, `cancel_at_full_hash_*`, `cancel_at_verify_returns_cancelled` | closed |
| T-01-05 | Tampering | stage marker ordering across rayon workers | low | mitigate | Marker sent from the calling thread; `progress_events_follow_their_stage_start`; progress bump and report under one mutex (`3c048b7`) | closed |
| T-01-06 | Tampering | stdout JSON report | high | mitigate | `progress.rs` has no print!/println!/stdout; `json_stderr_is_silent_when_not_a_terminal`; characterization tests | closed |
| T-01-07 | Denial of service | per-file events flooding a shell | medium | mitigate | Lock-free `Throttle` (AtomicU64 + compare_exchange, 80 ms); `throttle_limits_progress` | closed |
| T-01-08 | Tampering | terminal escapes in `--verbose` file names | low | accept | See Accepted Risks | closed |
| T-01-09 | Tampering | declared MSRV vs real buildability | medium | mitigate | CI `msrv` job on 1.90.0; `cargo +1.90.0 test --workspace` green (112/0) | closed |
| T-01-10 | Denial of service | collision with the release workstream's CI redesign | low | mitigate | 01-04 appended the `msrv` job only (no removed lines, checked at plan time) | closed |
| T-01-11 | Denial of service | walk stat phase ignoring cancel | medium | mitigate | Per-root, per-entry, per-file and post-phase checks in `walk.rs`; `walk_cancel_during_stat_phase_stops` | closed |
| T-01-12 | Tampering | digest of a partially read file | high | mitigate | `hash::full_cancellable` returns Err (`Interrupted`) on cancel, never a Digest; `full_cancellable_stops_on_raised_flag` | closed |
| T-01-13 | Repudiation | interrupted file reported as unreadable | low | mitigate | `compute_keys` drops post-cancel keys; `cancel_during_full_hash_is_not_reported_as_error` | closed |
| T-01-14 | Tampering | partial report on stdout after Ctrl+C | high | mitigate | Report written only after `Ok`; cancelled arm prints to stderr; `sigint_cancels_scan_with_exit_130` (ignored, run green by hand) | closed |
| T-01-15 | Elevation of privilege | async-signal-unsafe handler code | medium | mitigate | Only `signal_hook::flag` safe registration; no `unsafe` in `crates/twins-cli/src` | closed |
| T-01-16 | Denial of service | core installing a SIGINT handler | medium | mitigate | `cargo tree -p twins-core` has no signal-hook, ctrlc or tokio | closed |
| T-01-17 | Repudiation | second Ctrl+C `_exit` skips cleanup | low | accept | See Accepted Risks | closed |
| T-01-SC | Tampering | cargo installs (signal-hook 0.4.4) | high | mitigate | Package legitimacy audit OK in RESEARCH; exact version, `default-features = false`, Cargo.lock committed | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above workflow.security_block_on count toward threats_open*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-01-01 | T-01-03 | Skipped paths are the user's own local files, printed only with `--verbose` (unchanged behaviour) | Plan 01-01 threat model | 2026-10-02 |
| AR-01-02 | T-01-08 | Pre-existing `--verbose` behaviour; progress lines carry only counts and fixed labels. Revisit escape sanitising if file names reach new output surfaces | Plan 01-03 threat model | 2026-10-02 |
| AR-01-03 | T-01-17 | A scan writes nothing in this phase, so there is nothing to clean up; revisit when Phase 2 adds journaling | Plan 01-07 threat model | 2026-10-02 |

*Accepted risks do not resurface in future audit runs.*

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-10-02 | 18 | 18 | 0 | secure-phase (L1 grep-depth, register authored at plan time) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-10-02

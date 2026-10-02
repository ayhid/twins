# Phase 1: Core Pipeline Refactor - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md. This log preserves the alternatives considered.

**Date:** 2026-10-02
**Phase:** 01-core-pipeline-refactor
**Areas discussed:** Ctrl+C behavior, Progress display, dry_run meaning, Toolchain & CI pin

---

## Ctrl+C behavior

| Option | Description | Selected |
|--------|-------------|----------|
| 130 | Unix SIGINT convention; distinct from 1 and 2 | ✓ |
| 1 | Generic failure | |
| Dedicated code (e.g. 3) | twins-specific | |

| Option | Description | Selected |
|--------|-------------|----------|
| Second Ctrl+C exits immediately | Escape hatch when unwinding is stuck | ✓ |
| Ignore it | Always wait for a clean unwind | |

| Option | Description | Selected |
|--------|-------------|----------|
| One stderr line | `scan cancelled`, nothing on stdout | ✓ |
| Line + what was done | Includes the stage and counts | |

| Option | Description | Selected |
|--------|-------------|----------|
| You decide | The planner picks the signal crate | ✓ |
| ctrlc crate | | |
| signal-hook | | |

**User's choice:** all recommended options; signal plumbing left to Claude.

---

## Progress display

| Option | Description | Selected |
|--------|-------------|----------|
| Step counter + counts | `[3/4] partial hash  1 234 / 5 000` | ✓ |
| Keep current style | `partial hash: 1234/5000` | |
| Counts + percentage | `... (24%)` | |

| Option | Description | Selected |
|--------|-------------|----------|
| Emit event, flash label | Size grouping is a real stage | ✓ |
| Fold into walk | No separate stage | |

| Option | Description | Selected |
|--------|-------------|----------|
| Show on stderr with --json | stdout stays pure JSON | ✓ |
| Stay silent (current) | | |

| Option | Description | Selected |
|--------|-------------|----------|
| Verify as conditional 5th step | `[5/5] verify` | ✓ |
| Fold into full hash | | |

**User's choice:** all recommended options.

---

## dry_run meaning

| Option | Description | Selected |
|--------|-------------|----------|
| false: scan isn't a clean | Output unchanged; matches Go v0 | ✓ |
| true: nothing was changed | Changes scan output | |

| Option | Description | Selected |
|--------|-------------|----------|
| Keep schema v1 as is | RunMode internal, dry_run derived | ✓ |
| Add meta.mode now | Additive field | |

| Option | Description | Selected |
|--------|-------------|----------|
| Core-level test | Pipeline in DryRun mode → true; scan → false | ✓ |
| Hidden CLI flag | | |

**User's choice:** all recommended options.

---

## Toolchain & CI pin

| Option | Description | Selected |
|--------|-------------|----------|
| rust-version only | No rust-toolchain.toml | ✓ |
| rust-toolchain.toml = 1.90 | | |

| Option | Description | Selected |
|--------|-------------|----------|
| stable + 1.90 job | Full stable job plus an MSRV build/test job | ✓ |
| 1.90 only | | |
| stable only | | |

| Option | Description | Selected |
|--------|-------------|----------|
| Fix new lints | Targeted `#[allow]` with a comment only where justified | ✓ |
| Case by case | | |

**User's choice:** all recommended options.

---

## Claude's Discretion

- Ctrl+C signal crate (`ctrlc` vs `signal-hook`)
- Exact `Event` variant set, throttle interval, module layout
- How the walk exposes per-file progress

## Deferred Ideas

- `meta.mode` JSON field: Phase 2
- CLI-level `--dry-run` test: Phase 2

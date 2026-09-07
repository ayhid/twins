# twins

Find and remove duplicate files on macOS, safely.

> **Status:** twins is being rewritten in Rust with a Tauri desktop app, see
> [#1](https://github.com/ayhid/twins/issues/1). This branch of the work ships the engine and a
> read-only CLI. Deletion, caching, configuration and the desktop app follow in later steps.
> The previous Go implementation is preserved at commit `9024929`.

```
twins scan [path...]     # find duplicates under the paths (default: ~) and list them
twins report [path...]   # same, as a JSON report for scripts
twins version
```

Nothing is ever modified by these commands.

## Install

```sh
cargo install --path crates/twins-cli
```

Requires macOS. Grant **Full Disk Access** to your terminal in System Settings → Privacy & Security
if you want to scan folders like Mail or Messages.

## How it finds duplicates

A pipeline where each stage only touches what the previous one could not rule out:

1. **Walk** with exclusions: `.git`, `node_modules`, caches, Time Machine, `~/Library` (opt-in),
   bundles like `.app` and `.photoslibrary` (never opened), symlinks (never followed), iCloud
   placeholders (never downloaded) and network volumes.
2. **Group by size**, dropping unique sizes, typically more than 90 % of files.
3. **Hardlink dedup**: paths sharing a `(device, inode)` are the same data and waste no space.
4. **Partial hash**: the first and last 16 KiB (xxHash64), two reads per candidate.
5. **Full hash**: BLAKE3 over the survivors only, up to 8 files in parallel.

Add `--verify` for a final byte-by-byte comparison.

## Options

| Flag | Effect |
|---|---|
| `--json` | print the JSON report instead of text (`report` always does) |
| `--min-size SIZE` | ignore smaller files; `512`, `10K`, `1.5MiB`, `2 GB` (default `1MiB`) |
| `--exclude GLOB` | skip matches; without `/` matched on the name, with `/` on the full path; repeatable |
| `--jobs N` | parallelism for stat and hashing (default: CPUs, capped at 8) |
| `--include-empty` | consider empty files |
| `--include-library` | descend into `~/Library` |
| `--include-node-modules` | descend into `node_modules` |
| `--include-remote` | accept network volumes |
| `--verify` | byte-by-byte comparison after hashing |
| `--verbose` | list every file that could not be read |

Which copy is marked as kept follows the `oldest` strategy: the earliest modified copy, then the
shallowest path, then the lexically first path. Hardlinks of the kept file are never listed for
removal.

## Safety

- `/System`, `/Library`, `/usr`, `/Applications`, … are never scanned.
- Roots on network volumes are refused unless `--include-remote` is given.

## Scripting

```sh
twins report ~/Downloads | jq '.summary'
twins report ~/Downloads | jq -r '.groups[].remove[]'
```

The JSON schema is versioned (`"version": 1`).

## Development

```sh
cargo build --workspace                       # target/debug/twins
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Workspace layout: `crates/twins-core` (engine), `crates/twins-cli` (binary).

MIT

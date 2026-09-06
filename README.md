# twins

Find and remove duplicate files on macOS, safely. Inspired by [Mole](https://github.com/tw93/Mole):
a keyboard-driven terminal UI, `--dry-run` everywhere, the Trash by default and an operations journal.

```
twins            # interactive menu
twins scan       # find duplicates under ~ and browse them (nothing is modified)
twins clean      # find, choose what to keep, remove the rest
twins report     # JSON report for scripts
```

## Install

```sh
brew install ayhid/tap/twins        # once the tap is published
# or
go install github.com/ayhid/twins/cmd/twins@latest
```

Requires macOS. Grant **Full Disk Access** to your terminal in System Settings → Privacy & Security
if you want to scan folders like Mail or Messages.

## How it finds duplicates

A five-stage pipeline where each stage only touches what the previous one could not rule out:

1. **Parallel walk** with exclusions: `.git`, `node_modules`, caches, Time Machine, `~/Library`
   (opt-in), bundles like `.app` and `.photoslibrary` (never opened), symlinks (never followed),
   iCloud placeholders (never downloaded) and network volumes.
2. **Group by size** — unique sizes are dropped, typically more than 90 % of files.
3. **Hardlink dedup** — paths sharing a `(device, inode)` are the same data and waste no space.
4. **Partial hash** — the first and last 16 KiB (xxhash), two reads per candidate.
5. **Full hash** — BLAKE3 over the survivors only, up to 8 files in parallel.

Add `--verify` for a final byte-by-byte comparison. Fingerprints are cached in
`~/Library/Application Support/twins/cache.db`, keyed by inode, size and mtime, so a second
scan of the same folder takes a fraction of a second.

On a 16 000-file Downloads folder: 1.5 s cold, 0.15 s with a warm cache.

## Browsing

```
twins scan ~/Pictures ~/Downloads
```

| Key | Action |
|---|---|
| `↑/↓` `j/k` `pgup/pgdn` `g/G` | move |
| `enter` | expand / collapse a group |
| `space` | mark or unmark a file (on a group line: toggle the whole group) |
| `a` / `u` | auto-select everything according to `--keep` / unmark everything |
| `/` | filter by path |
| `o` / `p` | reveal in Finder / Quick Look |
| `x` | remove what is marked (`clean` only) |
| `?` `q` | help, quit |

`★` is the copy that survives, `✗` is scheduled for removal. You can never mark the last copy of a group.

## Cleaning

```sh
twins clean ~/Downloads                     # browse, mark, confirm → Trash
twins clean --yes --dry-run ~/Downloads     # non-interactive preview
twins clean --yes --keep newest ~/Downloads # scriptable, straight to the Trash
twins clean --link ~/Photos                 # replace duplicates with APFS clones (space freed, paths kept)
twins clean --permanent ~/tmp               # asks you to type "permanent"; add --force for scripts
```

Which copy is kept is decided by `--keep`:

| Strategy | Keeps |
|---|---|
| `oldest` (default) | the earliest modified copy, then the shallowest path |
| `newest` | the most recently modified copy |
| `shortest-path` | the copy with the fewest path components |
| `in-dir` + `--keep-in DIR` | a copy under `DIR`, falling back to `oldest` |

Hardlinks of the kept file are never removed.

## Safety

- Files go to the Trash (Finder's *Put Back* undoes it). `--permanent` requires typing the word.
- At least one copy per group is always kept; the plan is validated before anything runs.
- `/System`, `/Library`, `/usr`, `/Applications`, … are never scanned nor touched.
- `--dry-run` journals what would happen and changes nothing.
- Every operation is appended to `~/Library/Logs/twins/operations.log` (JSON lines).

## Configuration

```sh
twins config init     # writes ~/Library/Application Support/twins/config.toml
twins config show
twins config path
twins config clear-cache
```

```toml
min_size = "1MiB"          # smaller files are ignored
exclude = ["*.log", "build"]
keep = "oldest"            # oldest | newest | shortest-path | in-dir
keep_dir = ""
delete_mode = "trash"      # trash | permanent | link
include_library = false
include_node_modules = false
include_empty = false
jobs = 0                   # 0 = CPUs, max 8
```

Flags override the file.

## Scripting

```sh
twins report ~/Downloads | jq '.summary'
twins report ~/Downloads | jq -r '.groups[].remove[]'
```

## Development

```sh
make test     # go test -race ./...
make cover    # coverage summary
make bench
make build    # bin/twins
```

MIT

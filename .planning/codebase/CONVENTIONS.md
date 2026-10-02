---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
# Coding Conventions

**Analysis Date:** 2026-10-02

## Naming Patterns

**Files:**
- Snake case: `hash.rs`, `fsutil.rs`, `cli_test.rs`
- Module entry points: `mod.rs` in directories like `crates/twins-core/src/group/mod.rs`
- Test files: `{module}_test.rs` (e.g., `hash_test.rs`, `keep_test.rs`)

**Functions:**
- Snake case: `partial()`, `equal()`, `visited()`, `colliding_pair()`
- Descriptive names with verb-noun pattern: `parse_size()`, `write_json()`, `is_local_volume()`

**Variables:**
- Snake case: `mut filled`, `dir`, `opts`, `min_size`
- Abbreviations acceptable when clear from context: `f` for file, `n` for count

**Types & Structs:**
- Pascal case: `Digest`, `FileMeta`, `Group`, `ScanArgs`, `HashError`
- Error types: `{Action}Error` pattern (e.g., `HashError`, `FsError`, `GroupError`)
- Strategy enums: `Strategy::Oldest`, `Strategy::Newest`, `Strategy::ShortestPath`

**Constants:**
- Upper case with underscores: `SAMPLE_SIZE`, `BUFFER_SIZE`, `SAMPLE_LEN`, `SF_DATALESS`, `MIB`
- Module-level visibility: exported from module files (e.g., `pub const SAMPLE_SIZE: u64 = 16 * 1024;`)

## Code Style

**Formatting:**
- Tool: `rustfmt` (built-in Rust formatter, checked in CI with `cargo fmt --all --check`)
- Standard Rust formatting rules apply (4-space indents, trailing commas in multi-line structures)
- No custom `.rustfmt.toml` configuration—uses defaults

**Linting:**
- Tool: `clippy`
- CI enforcement: `cargo clippy --workspace --all-targets -- -D warnings` (warnings treated as errors)
- Workspace lints in `Cargo.toml`:
  - `rust` lints: `unsafe_code = "warn"`, `missing_docs = "warn"`
  - `clippy` lints: `all = { level = "warn", priority = -1 }`, `pedantic = { level = "warn", priority = -1 }`
  - Exceptions: `module_name_repetitions = "allow"` (to allow patterns like `group::Group`)
- Suppress with `#[allow(...)]` only when justified by comment (e.g., `#[allow(clippy::struct_excessive_bools)]`)

## Import Organization

**Order:**
1. `use std::...` (standard library)
2. `use crate::...` (internal modules)
3. External crates (e.g., `use clap::Parser`, `use serde::Serialize`)

Example from `crates/twins-cli/src/run.rs`:

```rust
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use anyhow::{Result, anyhow};
use twins_core::group::{self, Index, Keeper, Strategy};
use twins_core::human::parse_size;

use crate::cli::ScanArgs;
```

**Path Aliases:**
- Relative paths: `use crate::...` for internal modules
- No path remapping configured; imports use direct module paths

## Documentation

**Public Items:**
- All public functions, types, and modules must have doc comments
- Use `///` for items and `//!` for module-level documentation
- Include error documentation with `# Errors` section

Example from `crates/twins-core/src/hash.rs`:

```rust
/// Fingerprints the first and last [`SAMPLE_SIZE`] bytes of the file.
/// Files no larger than two samples are read entirely. Designed to discard
/// non-duplicates with at most two reads.
///
/// # Errors
/// [`HashError`] when the file cannot be opened or read.
pub fn partial(path: &Path, size: u64) -> Result<u64, HashError> {
```

**Doc Comments on Fields:**
- Public struct fields documented with `///`

Example from `crates/twins-core/src/hash.rs`:

```rust
pub struct HashError {
    /// Operation attempted, e.g. `open` or `read`.
    pub op: &'static str,
    /// Path the operation was attempted on.
    pub path: PathBuf,
    /// Underlying error.
    #[source]
    pub source: io::Error,
}
```

**Private Code:**
- No doc comments required for private items (though helpful comments are acceptable)

## Error Handling

**Patterns:**
- Custom error types derived with `thiserror::Error`:
  ```rust
  #[derive(Debug, thiserror::Error)]
  #[error("{op} {path}: {source}")]
  pub struct HashError {
      pub op: &'static str,
      pub path: PathBuf,
      #[source]
      pub source: io::Error,
  }
  ```
- Use `Result<T, E>` return types for fallible functions
- Propagate errors with `?` operator
- For CLI: use `anyhow::Result<()>` and `anyhow!()` macro for simple context

**Error Naming:**
- `{Domain}Error` pattern: `HashError`, `FsError`, `GroupError`
- Use enum variants for multiple error conditions

Example from `crates/twins-core/src/fsutil.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("{0}: not a regular file")]
    NotRegular(PathBuf),
    #[error("{op} {path}: {source}")]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
```

## Unsafe Code

**Guidelines:**
- Mark unsafe blocks with `#[allow(unsafe_code)]` at function level
- Add explanatory comment above unsafe block:
  ```rust
  // SAFETY: `c_path` is a valid NUL-terminated string and `fs` points at
  // writable memory of the right size; statfs fully initialises it on
  // success, which is the only case we read it.
  let rc = unsafe { libc::statfs(c_path.as_ptr(), fs.as_mut_ptr()) };
  ```

## Visibility & Encapsulation

**Module Exports:**
- Use `pub use` in `mod.rs` for public re-exports
- Avoid `pub mod` for leaf modules—prefer `mod` and selective `pub use`

Example from `crates/twins-core/src/group/mod.rs`:

```rust
mod find;
mod hasher;
mod index;
mod keep;

pub use find::{FindError, GroupError, Options, Progress, Stage, find};
pub use hasher::{DirectHasher, Hasher};
pub use index::Index;
pub use keep::{Action, Keeper, Strategy, UnknownStrategy, plan, total_reclaimable};
```

**Struct Visibility:**
- Private fields with public accessor methods
- Use `#[must_use]` on accessors to encourage consumption

Example from `crates/twins-core/src/group/mod.rs`:

```rust
pub struct Group {
    size: u64,
    digest: Digest,
    files: Vec<FileMeta>,
}

impl Group {
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }
}
```

## Attributes & Annotations

**#[must_use]:**
- Applied to all builder methods and functions whose return value should not be discarded
- Applied to functions that compute important values

Example: `#[must_use]` on `FileMeta::new()`, `Group::new()`, `Keeper::choose()`

**#[derive(...)]:**
- Use standard trait derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
- Add `thiserror::Error` for error types
- Add `serde::{Deserialize, Serialize}` for types that need JSON serialization

Example from `crates/twins-core/src/report.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub version: u32,
    // ... fields
}
```

**#[serde(...)]:**
- Use `#[serde(default, skip_serializing_if = "String::is_empty")]` for optional fields
- Use `#[serde(...)]` sparingly—only for schema control

## Builder Pattern

**Construction:**
- Use method chaining with `self` returns for optional configuration
- Builders consume `self` and return `Self`

Example from `crates/twins-core/src/scan.rs`:

```rust
let opts = scan::Options::new(roots.clone())
    .min_size(min_size)
    .include_empty(args.include_empty)
    .include_library(args.include_library)
    .exclude(args.exclude.clone())
    .workers(args.jobs);
```

---

*Convention analysis: 2026-10-02*

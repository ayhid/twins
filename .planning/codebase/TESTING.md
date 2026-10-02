---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
# Testing Patterns

**Analysis Date:** 2026-10-02

## Test Framework

**Runner:**
- Framework: Rust's built-in `#[test]` macro (no external test framework)
- Run all tests: `cargo test --workspace`
- Watch mode: Use `cargo watch -x "test --workspace"` (not integrated into project)
- Coverage: Not measured; no coverage configuration

**Assertion Library:**
- Standard Rust `assert_*!()` macros: `assert!()`, `assert_eq!()`, `assert_ne!()`
- Integration tests use `assert_cmd::Command` and `predicates::prelude::*`

## Test File Organization

**Location:**
- Unit tests: Co-located in `tests/` directory by crate
  - `crates/twins-core/tests/` contains test files for core library
  - `crates/twins-cli/tests/` contains integration tests for CLI
- Fixtures: `crates/twins-core/tests/fixtures/mod.rs` (shared test helpers)

**Naming:**
- Pattern: `{module}_test.rs` (e.g., `hash_test.rs`, `keep_test.rs`, `scan_test.rs`, `cli_test.rs`)
- Test functions: Descriptive snake_case names with clauses separated by underscores
  - `partial_ignores_the_middle_of_large_files()`
  - `parse_strategy()`
  - `oldest_keeps_the_earliest_mtime()`
  - `min_size_widens_the_scan_and_report_is_json_by_default()`

**File Structure:**

```
crates/twins-core/
├── tests/
│   ├── fixtures/
│   │   └── mod.rs          # Shared fixture builders
│   ├── hash_test.rs
│   ├── keep_test.rs
│   ├── scan_test.rs
│   ├── group_test.rs
│   ├── report_test.rs
│   ├── safety_test.rs
│   ├── fsutil_test.rs
│   └── human_test.rs
└── src/
    └── ...

crates/twins-cli/
├── tests/
│   └── cli_test.rs         # Integration tests
└── src/
    └── ...
```

## Test Structure

**Module-Level Documentation:**
Each test file starts with module documentation:

```rust
//! Tests for partial / full hashing and byte comparison.

use std::fs;
use std::path::PathBuf;

use twins_core::hash::{self, Digest};
```

**Helper Functions:**
Test files define reusable setup functions at the top:

From `crates/twins-core/tests/hash_test.rs`:

```rust
fn write(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> PathBuf {
    let p = dir.path().join(name);
    fs::write(&p, content).unwrap();
    p
}

fn colliding_pair() -> (Vec<u8>, Vec<u8>) {
    let mut a = vec![7u8; 3 * SAMPLE];
    let mut b = a.clone();
    a[SAMPLE + 100] = 1;
    b[SAMPLE + 100] = 2;
    (a, b)
}
```

**Test Function Pattern:**
Standard Arrange-Act-Assert structure:

```rust
#[test]
fn partial_ignores_the_middle_of_large_files() {
    // Arrange
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = colliding_pair();
    let pa = write(&dir, "a", &a);
    let pb = write(&dir, "b", &b);

    // Act
    let ha = hash::partial(&pa, a.len() as u64).unwrap();
    let hb = hash::partial(&pb, b.len() as u64).unwrap();

    // Assert
    assert_eq!(ha, hb, "same head and tail must give the same partial hash");
}
```

## Shared Fixtures

**Location:** `crates/twins-core/tests/fixtures/mod.rs`

**Purpose:** Provide reusable test utilities and builders for filesystem state

**Key Components:**

1. **Tree Builder:**
   ```rust
   pub struct Tree {
       dir: tempfile::TempDir,
   }

   impl Tree {
       /// Creates every (path, content) pair, making parent directories.
       pub fn build(files: &[(&str, &[u8])]) -> Self { ... }

       pub fn root(&self) -> &Path { ... }
       pub fn path(&self, rel: &str) -> PathBuf { ... }
       pub fn paths(&self, rels: &[&str]) -> BTreeSet<PathBuf> { ... }
       pub fn hard_link(&self, target: &str, link: &str) { ... }
       pub fn symlink(&self, target: &str, link: &str) { ... }
   }
   ```

2. **Constants:**
   ```rust
   pub const MIB: usize = 1024 * 1024;
   ```

3. **Utility Functions:**
   ```rust
   pub fn mib(byte: u8) -> Vec<u8> {
       vec![byte; MIB]
   }
   ```

**Usage Example** from `crates/twins-core/tests/scan_test.rs`:

```rust
mod fixtures;

use fixtures::{Tree, mib};

fn tree() -> Tree {
    let big = mib(1);
    Tree::build(&[
        ("a.bin", &big),
        ("sub/b.bin", &big),
        ("small.bin", b"tiny"),
        ("empty.bin", b""),
    ])
}

#[test]
fn default_rules_visit_only_plain_large_files() {
    let t = tree();
    t.symlink("a.bin", "link.bin");
    // ... test continues
}
```

## Test Types

**Unit Tests:**
- Scope: Test individual functions and methods in isolation
- Location: `crates/twins-core/tests/`
- Examples: `hash_test.rs`, `keep_test.rs` test specific module behavior
- Pattern: Setup test data → Call function → Assert result

**Integration Tests:**
- Scope: End-to-end testing of CLI with actual binary
- Location: `crates/twins-cli/tests/cli_test.rs`
- Tools: `assert_cmd::Command` for running binary, `predicates` for output matching
- Examples:
  ```rust
  #[test]
  fn version_prints_crate_version() {
      let expected = format!("twins {}\n", env!("CARGO_PKG_VERSION"));
      twins().arg("version").assert().success().stdout(expected);
  }

  #[test]
  fn scan_text_marks_kept_file_and_never_lists_hardlinks_to_remove() {
      let dir = fixture();
      twins()
          .args(["scan", "--exclude", "*.log", "--jobs", "2"])
          .arg(dir.path())
          .assert()
          .success()
          .stdout(predicate::str::contains("[1] 1.0 MiB × 3  (1.0 MiB reclaimable)"))
          .stdout(predicate::str::contains("★"));
  }
  ```

## Temporary File Management

**Framework:** `tempfile` crate

**Pattern:**

```rust
use tempfile::TempDir;

let dir = tempfile::tempdir().unwrap();  // Creates temporary directory
// ... use dir.path() for operations
// ... automatically cleaned up when dir is dropped
```

**File Creation:**

```rust
fn write(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> PathBuf {
    let p = dir.path().join(name);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, content).unwrap();
    p
}
```

**Advantages:**
- Automatic cleanup (no test pollution)
- Each test gets isolated filesystem state
- Platform-appropriate temp location

## Error Assertions

**Testing Errors:**

```rust
#[test]
fn missing_files_are_errors_carrying_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let nope = dir.path().join("nope");
    let err = hash::full(&nope).unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");
    assert!(hash::partial(&nope, 10).is_err());
    assert!(hash::equal(&nope, &nope).is_err());
}
```

**Pattern:**
- Call `.unwrap_err()` on `Result` to extract error
- Assert error message contains expected context
- Verify error type/variant as needed

## CLI Integration Testing

**Setup Helper:**

```rust
fn twins() -> Command {
    let mut c = Command::cargo_bin("twins").expect("twins binary is built");
    c.env_remove("TWINS_CONFIG");
    c
}
```

**Test Fixture Helper:**

```rust
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let w = |rel: &str, content: &[u8]| {
        let p = dir.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
    };
    w("a.bin", &vec![1u8; MIB]);
    w("sub/a-copy.bin", &vec![1u8; MIB]);
    // ... more files
    dir
}
```

**Output Parsing:**

```rust
fn groups(json: &serde_json::Value) -> Vec<Vec<PathBuf>> {
    json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            g["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| PathBuf::from(f["path"].as_str().unwrap()))
                .collect()
        })
        .collect()
}
```

## Assertions with Predicates

**Framework:** `predicates` crate

**Patterns:**

```rust
use predicates::prelude::*;

// String contains assertion
.stdout(predicate::str::contains("1 group, 1 duplicate"))

// Failure with specific error message
.failure()
.stderr(predicate::str::contains("protected"))
```

## Test Dependencies

**Workspace Dependencies** (from `Cargo.toml`):
- `assert_cmd = "2.2.2"` - Running CLI binaries in tests
- `predicates = "3"` - Output matching in CLI tests
- `tempfile = "3.27.0"` - Temporary file/directory creation
- `serde_json = "1.0.151"` - Parsing JSON output in tests
- `thiserror = "2.0.20"` - Error types (used in library code tested)

## CI Testing

**Configuration:** `.github/workflows/ci.yml`

**Test Execution:**

```bash
cargo test --workspace
```

**Related CI checks:**
- `cargo fmt --all --check` - Code formatting
- `cargo clippy --workspace --all-targets -- -D warnings` - Linting
- `RUSTFLAGS: -D warnings` - Warnings treated as errors

## Test Coverage Philosophy

**Current State:**
- No explicit coverage measurement
- Tests focus on behavior correctness, not line coverage
- Critical paths (hashing, grouping, error handling) well-tested
- Integration tests validate end-to-end flows

**What to Test:**
- Core algorithms (hash functions, grouping logic)
- Error conditions and edge cases
- CLI output and flags
- Filesystem operations with real temp files

**What's NOT Tested:**
- Internal helper functions (considered implementation details)
- Perfect coverage of all branches (focus on critical paths)

---

*Testing analysis: 2026-10-02*

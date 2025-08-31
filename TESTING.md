Here’s the pragmatic Rust take.

**TL;DR**

- **Unit tests → colocated.** Put small, white-box tests in the same file as the code under `#[cfg(test)] mod tests { use super::*; … }`.
- **Integration tests → `tests/` dir.** Black-box tests that hit your public API; each file is compiled as its own crate.
- **Doctests → in your docs.** Treat code blocks in `///` as executable examples; they compile under `cargo test`.
- **Examples → `examples/`.** Runnable “how to use it” programs; great as living docs and targets for smoke testing.
- **Shared test helpers → `tests/common/`.** Put `tests/common/mod.rs` with utilities and `mod common;` from each test file.
- **Deterministic repo setup** via nextest: tests/scripts/nextest/setup-fixture.sh creates a tiny repo once per run; tests consume its path from env.
- **Bin crates → extract a lib.** Move logic to `src/lib.rs` so integration tests can import it cleanly.

---

### Recommended layout

```
my_crate/
├─ Cargo.toml
├─ src/
│  ├─ lib.rs
│  └─ foo.rs         # foo’s unit tests live at bottom of this file
├─ tests/
│  ├─ api_happy_path.rs
│  ├─ error_cases.rs
│  └─ common/
│     └─ mod.rs      # shared helpers, nextest fixture repo, tempdir setup, etc.
├─ examples/
│  └─ quickstart.rs
└─ benches/           # (optional) Criterion benchmarks
```

### Unit tests (close to the code)

Colocated tests are ideal for tiny behaviors and edge cases because they can use `super::*` and keep context tight.

```rust
// src/foo.rs
pub fn parse_thing(s: &str) -> Result<T, E> { /* … */ }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_basic() {
        assert!(parse_thing("ok").is_ok());
    }
}
```

Rule of thumb: if the test only makes sense while reading the implementation, keep it here.

### Integration tests (public API, user’s perspective)

Files in `tests/` compile as separate crates and only see your public items. This is perfect for end-to-end flows, CLI behavior, and invariants that cross modules.

```rust
// tests/api_happy_path.rs
use my_crate::*; // public API only

mod common;
use common::{tmp_workspace};

#[test]
fn end_to_end_succeeds() {
    let dir = tmp_workspace();
    let out = do_the_thing(&dir).unwrap();
    assert_eq!(out.count, 42);
}
```

**Shared helpers** (not auto-discovered as tests):

```rust
// tests/common/mod.rs
use tempfile::TempDir;
pub fn tmp_workspace() -> TempDir { tempfile::tempdir().unwrap() }
```

### Doctests (docs that never rot)

In `src/lib.rs`:

````rust
/// Parse a thing from text.
///
/// ```
/// use my_crate::parse_thing;
/// let t = parse_thing("ok").unwrap();
/// assert!(t.is_valid());
/// ```
pub fn parse_thing(...) { /* … */ }
````

Use `no_run` for network/IO examples and hide setup lines with leading `#`.

### Examples (runnable guides)

`examples/quickstart.rs` demonstrates real usage patterns. They’re great for documentation and can be compiled in CI via `cargo build --examples` or executed with `cargo run --example quickstart`.

### Visibility & “testing privates”

Prefer testing through public API. If you must touch internals, use:

- **Colocated unit tests** (have access via `use super::*`), or
- Temporarily loosen visibility (`pub(crate)`) for seams that matter, or
- A test-only cfg gate:

  ```rust
  #[cfg(test)]
  pub(crate) fn helper_for_tests(...) { ... }
  ```

Avoid test-only behavior changes in production code; keep it to helper surfaces.

### CLI and async patterns

- **CLI crates:** Put assertions in integration tests with `assert_cmd` and `predicates`.
- **Async:** Use the appropriate runtime attribute (e.g., `#[tokio::test]`).
- **Temp files:** `tempfile` is your friend for isolation.
- **Property tests:** `proptest` or `quickcheck` belong in unit or integration tests depending on scope.

### Workspaces

Each crate owns its tests. For cross-crate scenarios, create a top-level `tests/` crate that `dev-depends` on multiple members and orchestrates higher-level flows.

---

### Practical heuristics

- **Small & specific → colocated unit test.**
- **User-visible behavior or multi-module flow → integration test.**
- **Usage examples you’d put in the README → doctest or `examples/`.**
- **Anything reusable across tests → `tests/common/`.**

---

**Truthfulness Audit**

- _Fact:_ Rust supports colocated unit tests behind `#[cfg(test)]` and `use super::*`.
- _Fact:_ Files in `tests/` compile as separate crates and access only the public API.
- _Fact:_ Doctests execute code blocks from `///` comments under `cargo test` with flags like `no_run`/`ignore`.
- _Fact:_ `examples/` is the conventional directory for runnable example binaries.
- _Fact:_ `tests/common/mod.rs` is a standard pattern for shared helpers across integration tests.
- _Fact:_ `env!("CARGO_MANIFEST_DIR")`, `include_str!`, and `include_bytes!` are common ways to locate/ship fixtures.
- _Inference:_ Preferring public-API testing and minimizing “testing privates” improves design and maintenance.
- _Inference:_ Extracting bin logic into a lib eases testing and reuse.

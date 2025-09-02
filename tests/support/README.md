# test-support

Shared helpers for integration and unit tests, designed to be friendly with `cargo nextest`.

## What you get

- `init_tracing()` — one-line logging init honoring `RUST_LOG`, with test writer.
- `fixtures_dir()`, `read_fixture_text/bytes/json()` — robust fixture loading.
- `tempdir()`, `temp_cwd()` — temp workspaces with auto clean-up and guard utilities.
- `with_env()` — scoped env var setting.
- `cmd_bin()` — ergonomic CLI testing via `assert_cmd`.
- Optional Tokio time controls behind `feature = "tokio"`.

## Example

```rust
use test_support::{init_tracing, temp_cwd, cmd_bin, read_fixture_text};

#[test]
fn smoke() {
    init_tracing();
    let (_td, _cwd) = temp_cwd();
    let mut cmd = cmd_bin("my-cli");
    cmd.arg("--version").assert().success();
    let _data = read_fixture_text("sample.txt");
}
```

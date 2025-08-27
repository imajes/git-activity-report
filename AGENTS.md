# Repository Guidelines

This repo captures a Prototype Python CLI (`git-activity-report`) that exports Git activity to JSON for LLM-driven reporting, plus tests, schemas, and fixtures. 
Use this as a quick contributor guide.

## Project Structure & Module Organization

- `git-activity-report.py` — Python CLI (reference implementation).
- **Rust (port)**

  - `Cargo.toml`, optional `rust-toolchain.toml` (pin stable toolchain).
  - `src/` modules (planned): `main.rs`, `cfg.rs`, `window.rs`, `gitio.rs`, `model.rs`, `enrich.rs`, `unmerged.rs`, `render.rs`, `util.rs`.
  - Shard filenames: `YYYY.MM.DD-HH.MM-<shortsha>.json`; manifests: `manifest-YYYY-MM.json`, `manifest.json`.

- `tests/schemas/` — JSON Schemas (Draft 2020‑12).
- `tests/fixtures/` — validation fixtures.
- `Justfile` — validation, formatting, fixture generation.

## Build, Test, and Development Commands

**Validation (schemas/fixtures):**

```bash
just validate-all
```

**Python tool quick run:**

```bash
git activity-report --simple --for "last week" --repo . > out.json
```

**Rust (when present):**

```bash
rustup show                    # ensure toolchain
cargo build                    # build the binary
cargo test                     # run unit/integration tests
cargo fmt --all -- --check     # formatting gate
cargo clippy -- -D warnings    # lint; treat warnings as errors
cargo run -- --help            # smoke test CLI
```

**Utilities:**

```bash
just build-fixtures            # synthesize tiny repo + fixtures
just fmt-fixtures              # pretty-print fixtures with jq
just doctor                    # tooling sanity
```

## Coding Style & Naming Conventions

**Python**: PEP 8, 4‑space indents, stdlib only. Keep flags explicit and documented in `--help`.

**Rust**:

- Format with `rustfmt` (CI enforces `cargo fmt --all -- --check`).
- Lint with `clippy`; aim for clean `cargo clippy -- -D warnings`.
- Naming: `snake_case` for modules/functions, `UpperCamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Error handling: prefer `anyhow`/`thiserror` patterns if introduced; otherwise `Result<T, E>` with context.
- Keep JSON field shapes **identical** to Python Schema v2.

**Naming patterns**:

- Shards: `YYYY.MM.DD-HH.MM-<shortsha>.json`
- Range manifests: `manifest-YYYY-MM.json`; top-level: `manifest.json`

## Testing Guidelines

- Schemas: JSON Schema Draft 2020‑12, validated with `ajv-cli` (we invoke with `--spec=draft2020`).
- Fixtures: live in `tests/fixtures/`; keep them small and deterministic.
- Add edge cases (renames `R###`, merges if `--include-merges`, clipped patches when `--max-patch-bytes > 0`).

## Commit & Pull Request Guidelines

- Use clear, scoped commit messages (e.g., `feat:`, `fix:`, `refactor:`). Reference CLI flags or schema names when changing them.
- PRs should include: a short description, sample command(s) to reproduce, and which fixtures/schemas were touched. Link to any related issues.

## Agent‑Specific Notes (Context)

- The JSON is consumed by an external report‑writer agent. Do not invent links or change timestamps’ shape: `timestamps.{author,commit,author_local,commit_local,timezone}` is contract‑critical.
- Keep schema changes additive and versioned; update fixtures and `just validate-all` accordingly.

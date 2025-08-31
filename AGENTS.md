# Repository Guidelines

This repo captures a Prototype Python CLI (`git-activity-report`) and a Rust port that exports Git activity to JSON for LLM-driven reporting, plus tests and schemas.
Use this as a quick contributor guide.

Readability Preamble — Build in Phases, Then Act

Code here is written for humans first. Build values in a clear “create” phase (extract, compute, assemble), then perform side‑effects or mutations in a separate “act” phase (push/insert/write/return), with a single blank line separating phases. Follow these references when authoring code — apply them as you write, not post‑hoc:

- Layout (structure and ordering): see LAYOUT_SPEC.md
- Spacing (vertical separation): see SPACING_SPEC.md

## Project Structure & Module Organization

- `git-activity-report.py` — Python CLI (reference implementation).
- **Rust (port)**

  - `Cargo.toml`, optional `rust-toolchain.toml` (pin stable toolchain).
  - `src/` modules (planned): `main.rs`, `cfg.rs`, `window.rs`, `gitio.rs`, `model.rs`, `enrich.rs`, `unmerged.rs`, `render.rs`, `util.rs`.
  - Shard filenames: `YYYY.MM.DD-HH.MM-<shortsha>.json`; manifests: `manifest-YYYY-MM.json`, `manifest.json`.

- `tests/schemas/` — JSON Schemas (Draft 2020‑12).
- `Justfile` — build, test, formatting, spacing audit.

## Build, Test, and Development Commands

**Validation:** schemas are validated in Rust tests using the `jsonschema` crate; run `just test`.

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
just doctor                    # tooling status (Rust toolchain)
```

## Coding Style & Naming Conventions

**Python**: PEP 8, 4‑space indents, stdlib only. Keep flags explicit and documented in `--help`.

**Rust**:

- Format with `rustfmt` (CI enforces `cargo fmt --all -- --check`).
- Lint with `clippy`; aim for clean `cargo clippy -- -D warnings`.
- Naming: `snake_case` for modules/functions, `UpperCamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Error handling: prefer `anyhow`/`thiserror` patterns if introduced; otherwise `Result<T, E>` with context.
- Keep JSON field shapes **identical** to Python Schema v2.

### Spacing & Readability

- Follow `SPACING_SPEC.md` for human‑friendly blank‑line spacing, and `LAYOUT_SPEC.md` for structural layout rules. Apply them during creation — not as a post‑hoc tool, to prevent endless rewrites.
- Key rule: add a blank line between declaration/setup and the next control‑flow statement. Do not split `} else {}` or `else if` chains.
- Keep doc comments and attribute stacks directly attached to their items.

**Naming patterns**:

- Shards: `YYYY.MM.DD-HH.MM-<shortsha>.json`
- Range manifests: `manifest-YYYY-MM.json`; top-level: `manifest.json`

## Testing Guidelines

- Schemas: JSON Schema Draft 2020‑12 validated in‑process by tests (no external `ajv` required).
- Add edge cases (renames `R###`, merges if `--include-merges`, clipped patches when `--max-patch-bytes > 0`).

## Workflow Checklist (Preflight)

- Run `just doctor` and tests (`just test`).
- Build, clippy (`-D warnings`), and tests.
- Manual spacing pass per `SPACING.md` on any touched files (Rust, Python, Justfiles).

## Commit & Pull Request Guidelines

- Use clear, scoped commit messages (e.g., `feat:`, `fix:`, `refactor:`). Reference CLI flags or schema names when changing them.
- PRs should include: a short description, sample command(s) to reproduce, and any schema changes. Link to related issues.
- Readability checklist (applied during creation, not rewrites):
  - Code adheres to `LAYOUT_SPEC.md` (build in phases, then act; extract‑before‑build; field grouping) and `SPACING_SPEC.md` (phase separation, tight mini‑phases, I/O boundaries).
  - No post‑hoc spacing/layout rewrites required; code was authored in compliance.
  - Optional assistive checks: `just audit-spacing` (or `just audit-spacing-strict` when requested) are clean.

## Agent‑Specific Notes (Context)

- The JSON is consumed by an external report‑writer agent. Do not invent links or change timestamps’ shape: `timestamps.{author,commit,author_local,commit_local,timezone}` is contract‑critical.
- Keep schema changes additive and versioned; cover with schema tests and snapshots.

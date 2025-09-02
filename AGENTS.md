# Repository Guidelines

This repo captures a Prototype Python CLI (`git-activity-report`) and a Rust port that exports Git activity to JSON for LLM-driven reporting, plus tests and schemas.
Use this as a quick contributor guide.

Readability Preamble — Core Guidance for Agents

Build in phases, decide state once, and keep IO at the edges. Resolve windows into labeled ranges up front, set `multi_windows` explicitly, then process ranges in a single loop with a fixed lifecycle (generate → save → index). Make outputs contract‑strong (per‑range `report-<label>.json`, top‑level `manifest.json`), and write code in readable steps: extract before build, one concern per statement, one blank between phases. The rationale lives in prompt-engineering/CODING_PHILOSOPHY.md; apply the structural rules from prompt-engineering/LAYOUT_SPEC.md and the vertical separation rules from prompt-engineering/SPACING_SPEC.md while authoring, not as a post‑hoc pass.

## Project Structure & Module Organization

- `prototype/git-activity-report.py` — Python CLI (reference implementation).
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

Decision Precedence (Tie‑Breakers)

- When goals conflict, resolve in this order (highest first):
  1. Contract stability (filenames, schema, pointer shapes)
  2. Orchestration simplicity (single loop, explicit state)
  3. Correctness/testability (pure helpers, invariants)
  4. Performance (avoid duplicate heavy work)
  5. Minimal diff size/readability

See AGENT_RUBRIC.md for the operational checklist and acceptance criteria.

### Spacing & Readability

- Follow `prompt-engineering/SPACING_SPEC.md` for human‑friendly blank‑line spacing, and `prompt-engineering/LAYOUT_SPEC.md` for structural layout rules. Apply them during creation — not as a post‑hoc tool, to prevent endless rewrites.
- Key rule: add a blank line between declaration/setup and the next control‑flow statement. Do not split `} else {}` or `else if` chains.
- Keep doc comments and attribute stacks directly attached to their items.

**Naming patterns**:

- Shards: `YYYY.MM.DD-HH.MM-<shortsha>.json`
- Range manifests: `manifest-YYYY-MM.json`; top-level: `manifest.json`

## Testing Guidelines

- Schemas: JSON Schema Draft 2020‑12 validated in‑process by tests.
- Add edge cases (renames `R###`, merges if `--include-merges`, clipped patches when `--max-patch-bytes > 0`).

## Workflow Checklist (Preflight)

- Run `just doctor` and tests (`just test`).
- Build, clippy (`-D warnings`), and tests.
- Manual spacing pass per `prompt-engineering/SPACING.md` on any touched files (Rust, Python, Justfiles).

## Commit & Pull Request Guidelines

- Use clear, scoped commit messages (e.g., `feat:`, `fix:`, `refactor:`). Reference CLI flags or schema names when changing them.
- PRs should include: a short description, sample command(s) to reproduce, and any schema changes. Link to related issues.
- Readability checklist (applied during creation, not rewrites):
  - Code adheres to `prompt-engineering/LAYOUT_SPEC.md` (build in phases, then act; extract‑before‑build; field grouping) and `prompt-engineering/SPACING_SPEC.md` (phase separation, tight mini‑phases, I/O boundaries).
  - No post‑hoc spacing/layout rewrites required; code was authored in compliance.
- Optional assistive checks: `just audit-spacing` (or `just audit-spacing-strict` when requested) are clean.
- Module headers lint: `just check-headers` ensures `purpose:` and `role:` headers exist in `src/` files; keep files self-documenting.

## Agent Protocol (Enforced)

- Step 0 — Start a cycle: Run `just new-cycle "short description"`. Fill the template and keep it updated during the work.
- Step 1 — Generate overlay: The command writes `.agents/repo_overlay.json` used by agents/tooling to ground states and roles.
- Step 2 — Build in phases per the rubric and specs (LAYOUT/SPACING).
- Step 3 — Checks: `just ci-check` before commit/PR.

Enforcement options:

- Git hooks (local): enable with `git config core.hooksPath .githooks`. The pre-commit hook runs cycle and header checks.
- CI (server): `.github/workflows/agent-cycle-check.yml` blocks merges if cycle/header checks fail.

## Agent Cycles Log

- Maintain per-cycle records under `.agents/cycles/` using `prompt-engineering/TASK_TEMPLATE.md`. See `prompt-engineering/AGENT_CYCLES.md` for naming and workflow.
- Generate a short project overlay from `AGENT_RUBRIC.md` at the start of each cycle to ground states, artifacts, and acceptance criteria.

## Agent‑Specific Notes (Context)

- The JSON is consumed by an external report‑writer agent. Do not invent links or change timestamps’ shape: `timestamps.{author,commit,author_local,commit_local,timezone}` is contract‑critical.
- Keep schema changes additive and versioned; cover with schema tests and snapshots.

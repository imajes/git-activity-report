# Repository Guidelines

This repository contains a Rust CLI (with a small Python prototype) that exports Git activity to JSON for downstream reporting. Use this guide to get productive quickly and keep outputs contract‑stable.

## Project Structure & Module Organization

- `src/`: Rust sources. Key files: `main.rs`, `cli.rs`, `params.rs`, `model.rs`, `commit.rs`, `gitio.rs`, `range_windows.rs`, `range_processor.rs`, `render.rs`, `enrich.rs`, `enrichment/github_api.rs`, `enrichment/github_pull_requests.rs`, `util.rs`.
- `tests/`: Integration tests and schema validation; schemas in `tests/schemas/*.json`.
- `prototype/git-activity-report.py`: Reference Python CLI.
- `prompt-engineering/`: Authoring guidelines (spacing/layout/philosophy).
- `docs/man/`: Man page output path; generated via `just man`.
- Shards: `YYYY.MM.DD-HH.MM-<shortsha>.json`; manifests: `manifest-YYYY-MM.json`, `manifest.json`.

## Immutable Coding Rules (Non‑Negotiable)

- Phase before action: build values first, then act in a separate statement. Do not inline large literals in pushes/returns; bind to a local then use.
- Extract before build: any nested access chain (≥3 hops or multi‑line) MUST be extracted into a well‑named local (e.g., `pull_request_user`, `pull_request_head`).
- One concern per statement: keep compute, I/O, and mutation as distinct steps.
- Early guards (mandatory): avoid deep nesting by checking failure/skip conditions first and returning/continuing immediately. Deeply nested `if` blocks are forbidden; extract helpers or add guards to flatten. Aim for ≤ 2 nesting levels in functions.
- Spacing rules (enforced by authoring):
  - Insert exactly one blank line between phases (compute → I/O → push/return).
  - Do not split `} else {}` or `else if` chains; keep doc comments/attributes glued to items.
  - Max single blank between non‑blank lines; no vertical stacks of empties.
- Field grouping: group related fields (identity, temporal, links, payload), keep stable order across files, and separate groups with a blank where the language allows.
- Naming: semantic locals (no single/double‑letter names beyond tiny loop indices). Suffix raw JSON with `_json` when helpful.
- Imports: keep as one compact group; add a single blank after the block.
- Orchestrators: separate pipeline steps with one blank (normalize → process ranges → optional enrich → build manifest → write → return).
- Module headers: every `src/*.rs` file MUST start with single‑line `purpose:` and `role:` headers (checked by `just check-headers`).
- Contracts: JSON shapes and filenames are stable; schema changes must be additive and covered by tests.
- Guard blocks: after a multi‑line `if`/`match`, insert a blank before starting a new phase.

Quick examples

- Builder then use:
  - GOOD: `let pr = GithubPullRequest { /* … */ }; out.push(pr);`
  - BAD: `out.push(GithubPullRequest { /* many fields */ });`
- Compute vs I/O:
  - GOOD: compute → blank → write → blank → push/return.
  - BAD: `let txt = build(...)?; std::fs::write(path, txt)?; out.push(path);`
-
- Early guards vs deep nesting:
  - BAD: `if cond { if sub { do_work(); } }`
  - GOOD: `if !cond { return Ok(()); } if !sub { return Ok(()); } do_work();`
  - In loops, prefer `continue` guards to avoid inner nesting: `if invalid { continue; } // then process valid cases flat`

## Cycle Protocol (Non‑Negotiable)

Start every work cycle with a log and end it with checks. Follow these steps exactly.

1. Initialize cycle

- Create entry: `just new-cycle "short description"` (or `bash scripts/new_cycle.sh "..."`).
- Fill the new file under `.agents/cycles/` using `prompt-engineering/TASK_TEMPLATE.md` (summary, contracts, phases, invariants).
- The script also writes `.agents/repo_overlay.json` for tooling; keep it.

2. Plan and implement

- Sketch phases in the cycle file, then code strictly per the rules above (layout/spacing during creation, not post‑hoc).
- Prefer small, testable helpers; keep I/O at the edges.

3. Local checks (must pass before any PR)

- Build/tests: `cargo build && cargo test` (or `just test`).
- Format/lint: `cargo fmt --all -- --check` and `cargo clippy -- -D warnings`.
- Spacing/layout: run `just audit-spacing` (or `just audit-spacing-strict` when requested).
- Module headers: `just check-headers`.
- CI gate locally: `just ci-check` (runs cycle/header checks).

4. Open PR

- Include: concise description, sample command(s) to reproduce, any schema changes, and a link to the cycle file.
- If enriching GitHub PRs, ensure `GITHUB_TOKEN` is set or `gh auth login` was run; enrichment is best‑effort.

5. Close the cycle

- Update the cycle file with outcomes and links (PR, commits, artifacts).
- Ensure outputs respect contracts (filenames, schema) and are validated by tests.

## Build, Test, and Development Commands

- Build: `cargo build`
- Tests: `cargo test` (or `just test` for nextest+coverage)
- Lint/format: `cargo fmt --all -- --check` and `cargo clippy -- -D warnings`
- Run help: `cargo run -- --help`
- Example run: `cargo run -- --simple --for "last week" --repo . > out.json`
- Utilities: `just doctor` (tooling status), `just man` (write `docs/man/git-activity-report.1`).

## Coding Style & Naming Conventions

- Rust: format with `rustfmt`; lint with `clippy` (treat warnings as errors).
- Naming: `snake_case` (functions/modules), `UpperCamelCase` (types), `SCREAMING_SNAKE_CASE` (consts).
- Contracts: JSON shapes match the schemas; changes must be additive and tested.
- Follow `prompt-engineering/LAYOUT_SPEC.md` and `prompt-engineering/SPACING_SPEC.md` during authoring.

## Testing Guidelines

- Schemas: JSON Schema (Draft 2020‑12) validated in tests (`tests/integration/*`).
- Run all tests: `just test` or `cargo test`.
- Add coverage for edge cases (renames `R###`, merges with `--include-merges`, clipped patches via `--max-patch-bytes`).

## Commit & Pull Request Guidelines

- Commits: scoped and conventional (e.g., `feat:`, `fix:`, `refactor:`); mention changed flags/schemas.
- PRs: include a concise description, sample command(s) to reproduce, and call out any schema changes.

## Pull Request Checklist (Must Pass)

- Build/tests: `cargo build` and `cargo test` (or `just test`).
- Format: `cargo fmt --all -- --check`.
- Lint: `cargo clippy -- -D warnings` (no warnings permitted).
- Spacing/layout: `just audit-spacing` (or `just audit-spacing-strict` when requested).
- Module headers: `just check-headers` (requires `purpose:` and `role:` in each `src/*.rs`).
- CI preflight: `just ci-check` (cycle/header checks) must succeed locally.
- PR description includes:
  - One‑line summary and scope of changes.
  - Sample command(s) to reproduce/validate.
  - Link to the cycle file under `.agents/cycles/`.
  - Any schema changes noted explicitly (additive only).

## Security & Configuration Tips

- Keep tokens out of logs; prefer environment variables.
- Local hooks: `git config core.hooksPath .githooks` (optional) to run basic checks.

## Agent‑Specific Notes (Context)

- The JSON is consumed by an external report‑writer agent.
- Keep schema changes additive and versioned; cover with schema tests and snapshots.
- GitHub enrichment: to populate PR details, set `GITHUB_TOKEN` or run `gh auth login`. Falling back to no token is acceptable; reports remain valid.

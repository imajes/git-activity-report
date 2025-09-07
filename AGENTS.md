# Repository Guidelines for Agents

This document provides explicit and actionable guidelines for Codex or other AI agents tasked with working on this repository. Its focus is on maintaining contract-stable outputs, strict layout, and coding invariants, all enforceable by code and test.

## Project Structure

- `src/`: Main Rust sources (e.g., `main.rs`, `cli.rs`, `params.rs`, `model.rs`, `commit.rs`, `gitio.rs`, `range_windows.rs`, `range_processor.rs`, `render.rs`, `enrich.rs`, `enrichment/github_api.rs`, `enrichment/github_pull_requests.rs`, `util.rs`.)
- `tests/`: Integration tests and JSON schema validation (`tests/schemas/*.json`).
- `prototype/git-activity-report.py`: Python CLI prototype for reference.
- `prompt-engineering/`: Authoring specifications for layout, spacing, and philosophy.
- `docs/man/`: Output path for man pages, generated via `just man`.
- Data outputs: activity shards (`YYYY.MM.DD-HH.MM-<shortsha>.json`), manifests (`manifest-YYYY-MM.json`, `manifest.json`).

## Immutable Coding Rules

- **Never inline complex values**: Always assign large or multi-property structs to named locals before usage (e.g., pushes or returns).
- **Extract nested access**: Any three-level (or multi-line) property/field/method chain must be bound to a well-named local variable.
- **One concern per statement**: Separate compute, I/O, and mutation operations.
- **Prefer early returns/guards**: Eliminate deep nesting by early `return` or `continue` statements. Flatten control flow to ≤ 2 nesting levels. For complex logic, use helper functions.
- **Authoring spacing/layout rules (enforced):**
  - Insert exactly one blank line between compute, I/O, and push/return stages.
  - Do not separate `} else {}` or `else if` chains; keep attributes and doc comments glued to items.
  - Maximum one single blank between non-blank lines. No consecutive empty lines.
- **Field grouping**: Group related struct fields (identity, temporal, links, payload); order consistently; separate groups with single blank where supported.
- **Naming**: Use semantic local names. Avoid single/double-letter names (except in tiny loops). Use `_json` suffix for variables holding raw JSON when clarifying.
- **Imports**: Group imports in a single block; add a single blank after.
- **Orchestrators**: Separate pipeline phases with a blank: normalize → process ranges → optionally enrich → build manifest → write → return.
- **Module headers**: Each `src/*.rs` file must start with single-line `purpose:` and `role:` headers. Enforced by `just check-headers`.
- **Contract stability**: All JSON shapes and filenames are stable. Any schema change must be purposeful and approved by your human coding partner. The entire schema must be fully covered by tests.
- **Guard blocks**: After multi-line `if` or `match`, insert a blank before next phase.

Example patterns:

- Assign, then push:
  - **Correct:** `let pr = GithubPullRequest { ... }; out.push(pr);`
  - **Incorrect:** `out.push(GithubPullRequest { ... });`
- Split compute and I/O by blank lines.
  - **Correct:** compute → blank → write → blank → push
- Use early guards to flatten code; avoid deep nesting in `if` blocks.

## Cycle Protocol (Strict)

Each agent work cycle must follow these steps:
1. **Initialize**: `just new-cycle "short description"` or `bash scripts/new_cycle.sh "..."` to create a cycle file under `.agents/cycles/` using `prompt-engineering/TASK_TEMPLATE.md`.
2. **Plan and Implement**: Sketch phases in the cycle file and code directly to these plans; enforce layout and spacing during authoring.
3. **Local Checks** (pre-PR):
    - Build/tests: `just build && `just test`
    - Formatting/lint: `cargo fmt --all -- --check`, `cargo clippy -- -D warnings`
    - Spacing/layout: `just audit-spacing`
    - Module headers: `just check-headers`
    - Local CI: `just ci-check`
4. **Open PR**: Include concise description, sample commands, schema changes, and a link to the cycle file.
5. **Close cycle**: Update outcomes/links in the cycle file. Ensure contract outputs are schema-validated.

## Build, Test, and Development Commands
- Build: `cargo build`
- Test: `cargo test` or `just test`
- Lint/format: `cargo fmt --all -- --check`; `cargo clippy -- -D warnings`
- Help: `cargo run -- --help`
- Sample run: `cargo run -- --simple --for "last week" --repo . > out.json`
- Utilities: `just doctor`, `just man`

## Coding Style and Naming Conventions
- Format: Enforced by `rustfmt`.
- Lint: Enforced by `clippy` with warnings as errors.
- Naming: `snake_case` (functions/modules), `UpperCamelCase` (types), `SCREAMING_SNAKE_CASE` (consts).
- JSON contracts: All shapes map to schema, changes must be additive and tested.
- Follow rules in `prompt-engineering/LAYOUT_SPEC.md` and `prompt-engineering/SPACING_SPEC.md`.

## Testing Guidelines
- Schemas: Validate output against JSON Schema (Draft 2020-12); covered in `tests/integration/*`.
- All test paths: `just test`. IF FAILED, use `cargo test`
- Add edge case coverage (file renames, merges, patch clipping, etc.)

## Commit and PR Guidelines
- Commits: Use conventional messages (e.g., `feat:`, `fix:`, `refactor:`) and mention changed flags/schemas.
- PRs: Require description, sample commands, explicit note of schema changes.

### Code Hygiene Addendum

- Do not use `#[allow(dead_code)]` by default. This attribute is only permitted with explicit approval from the human collaborator and must include a brief in-line justification explaining why it is necessary and when it will be removed or cfg-gated.
- Prefer `#[cfg(any(test, feature = "testutil"))]` for test-only seams, helpers, and constructors. Production builds should remain warning-free without suppressing lints.

## PR Checklist for Agents (Must Pass)
- Build, test, format/lint, spacing/layout, and module headers as outlined above. CI (`just ci-check`) must pass.
- PR description must list: summary, sample command(s), cycle file link, and note all schema changes (additive only).

## Security and Agent Configuration
- Redact private tokens from logs. Use environment variables for credentials.
- Optionally configure Git hooks: `git config core.hooksPath .githooks`.

## Agent-Specific Context
- Output JSON feeds external report-writer agents.
- Schema changes must be backward-compatible and validated by appropriate schema tests.
- PR enrichment: Ensure `GITHUB_TOKEN` is available or login with `gh auth login`.

> **IMPORTANT**: All invariants and contracts are enforced by tests or scriptable checks. Deviations will result in PR rejection or test/CI failure. Codex and similar agents must adhere strictly to all layout, naming, output shape, and workflow protocols.

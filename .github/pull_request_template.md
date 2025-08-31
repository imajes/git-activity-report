Title: <type>: <short scope> — <concise summary>

Summary
- What changed and why. Link related issues.

Changes
- Flags/CLI: list any new or changed flags.
- Schema changes (if any) and rationale.
- Rust/Python parity: note if behavior differs intentionally.

Validation
- Commands to reproduce locally:
  - just test
- Optional sample runs:
  - cargo run -- ...

Risk/Impact
- Backwards compatibility concerns, edge cases, performance notes.

Checklist
- [ ] Docs updated (README/NEXT_STEPS as needed)
- [ ] Schema changes covered by tests/snapshots (if applicable)
- [ ] Tests updated/passing (>=90% where applicable)
- [ ] Clippy/rustfmt clean

Title: <type>: <short scope> — <concise summary>

Summary
- What changed and why. Link related issues.

Changes
- Flags/CLI: list any new or changed flags.
- Schemas/fixtures: note any schema or fixture updates (and why).
- Rust/Python parity: note if behavior differs intentionally.

Validation
- Commands to reproduce locally:
  - just build-fixtures
  - just validate-all
  - just test
- Sample runs:
  - python3 ./git-activity-report.py ...
  - cargo run -- ...

Risk/Impact
- Backwards compatibility concerns, edge cases, performance notes.

Checklist
- [ ] Docs updated (README/NEXT_STEPS as needed)
- [ ] Schemas/fixtures updated and validated (just validate-all)
- [ ] Tests updated/passing (>=90% where applicable)
- [ ] Clippy/rustfmt clean

Checkpoint — Status, Next Steps, and Quick Commands

Overview
- Python is the reference implementation; Rust port is largely feature complete and aligned with schema v2.
- Validations use local ajv; tests include unit + integration; CI and release workflows are added.
- Goldens/fixtures are neutralized to “Fixture Bot” and stable.

What’s Done (Rust)
- CLI + config normalization (simple/full modes; month/since–until windows; tz; merges; patch flags; save-patches; include-unmerged; github-prs).
- Full mode:
  - Shards under <base>/<label>/YYYY.MM.DD-HH.MM-<shortsha>.json (respects --tz).
  - Range manifest at <base>/manifest-<label>.json with items[], authors, summary, optional unmerged_activity.
- Unmerged scanning:
  - Excludes current branch; computes ahead/behind/merged; writes branch shards under unmerged/<branch> (slashes → __).
- Patches:
  - --include-patch embedding and --max-patch-bytes UTF‑8 clipping (sets patch_clipped).
  - --save-patches writes .patch files and sets patch_ref.local_patch_file.
- PR enrichment (best-effort):
  - Uses GITHUB_TOKEN and GitHub origin to fetch PRs; attaches github_prs[] and patch_ref.github_* URLs; optional/fallible.
- Commit meta mapping fixed:
  - Corrected %at/%ct/%s/%b indices; timestamps are accurate; subject/body populated.
  - Added body_lines (optional Vec<String>) alongside body string.
- patch_ref normalized:
  - git_show_cmd remains an array; added git_show_cmd_str with copy/paste command string.
  - Optional fields (local_patch_file, github_diff_url, github_patch_url) now present as null when unset.
- Code hygiene:
  - Type aliases for numstat return; clippy clean (-D warnings).
  - Whitespace/readability pass across util, gitio, main, enrich, and parts of render.

Tests & Tooling
- Unit tests (colocated): shard filename formatter.
- Integration tests:
  - cli_windows.rs: error paths + simple smoke.
  - simple_end_to_end.rs: simple mode on tiny repo; shape + timestamps.
  - full_unmerged.rs: full mode with unmerged; manifest/items.
  - patch_behaviors.rs: patch clipping + save-patches path wiring.
- tests/common/mod.rs helpers:
  - init_fixture_repo() deterministic tiny repo; disables GPG; main + feature/alpha.
  - bin_path() robustly discovers binary.
- Golden + compare:
  - tests/Justfile golden (Python), golden-rs (Rust + schema), validate-rs-full (schemas for full), compare-folders (PY vs RS outputs by folder; simple or full).
- Fixtures:
  - Committed simple/full fixtures are canonical and “Fixture Bot”.
  - Fixture generator writes to tests/.tmp (does not overwrite committed fixtures).

CI & Packaging
- CI (ci.yml): validate-all (ajv), fmt, clippy, build, just test (snapshots + goldens).
- Release (release.yml): builds on tagged pushes (macOS x86/ARM + Ubuntu), tars artifacts, creates GitHub release.

What’s Pending
- Implement --for phrase windows in Rust (parity with Python’s natural-language windows).
- Broaden tests:
  - Timezone filename parity (local vs utc).
  - Rename/copy statuses (R###/C###) and merge commit cases.
  - Unmerged branch edge cases (no commits, multiple branches).
  - PR enrichment network behavior (timeouts/rate-limits) — consider lightweight mocks or tolerant assertions.
- Examples + doctests; optional refactor to lib crate for cleaner API imports.
- Performance: consider rayon parallelization for per-commit work if needed.
- Packaging polish: Homebrew tap automation, cross builds for Linux x86_64/ARM.

Notes / Gotchas
- Python remains the contract; Rust output adds body_lines and git_show_cmd_str (schemas allow extra fields).
- PR enrichment is optional and intentionally non-fatal; absence is expected without token.
- Goldens should remain neutral; keep “Fixture Bot”.
- Compare tool:
  - Simple: validates schema and normalized diff.
  - Full: validates top/range manifests and diffs items by {sha, subject}. Add shard-by-shard compare later if desired.

Quick Commands (after restart)
- Tooling sanity:
  - just doctor
- Schema validation:
  - just validate-all
- Build + tests:
  - cargo build
  - cargo clippy -- -D warnings
  - cargo test
- Goldens on tiny repo:
  - just build-fixtures
  - just test
- Compare Python vs Rust on real outputs:
  - just -f tests/Justfile compare-folders py=@real-output/python rs=@real-output/rust mode=full
  - (or mode=simple for single-file outputs)

File Touchpoints
- src/gitio.rs: commit_meta index mapping; type aliases; spacing.
- src/render.rs: simple/full/unmerged flows; patch_ref, body_lines; whitespace.
- src/model.rs: added git_show_cmd_str; body_lines; patch_ref optional fields always present.
- src/enrich.rs: GitHub PRs (ureq); spacing.
- tests/common/mod.rs; tests/*.rs integration suite; tests/scripts/compare-outputs.sh; tests/Justfile.
- .github/workflows/{ci.yml,release.yml}; .gitignore updated.


# NEXT STEPS — Roadmap & Rust Port Plan

This document captures concrete next steps, with a focus on a faithful Rust port that preserves today’s behavior and schemas while improving performance and distribution.

---

## 0) Stabilize the Python reference (short sprint)

**Goal:** lock the Python tool and the v2 schema/fixtures so the Rust port has an exact target.

### 0.1 Freeze the schema

- Keep **Schema v2** as-is (timestamps live under `commit.timestamps` with `author`, `commit`, `author_local`, `commit_local`, `timezone`).
- Do **not** add new fields before the Rust MVP. If you want a field like `schema_version`, add it **after** M2 and bump schemas to `v2.1`.
- Ensure all schemas declare Draft 2020-12 and are validated with `ajv --spec=draft2020`.

### 0.2 Lock fixtures (goldens)

- Minimal set (already present):

  - `git-activity-report.simple.fixture.json`
  - `manifest.json` (top manifest)
  - `manifest-2025-08.json` (range manifest)
  - Two commit shards: `2025.08.12-14.03-aaaaaaaaaaaa.json`, `2025.08.13-09.12-bbbbbbbbbbbb.json`

- Add **edge-case** goldens (commit a small synthetic repo to produce them):

  - **rename**: an `R###` entry with `old_path`
  - **merge commit**: include with `--include-merges`
  - **binary/huge patch**: run with `--include-patch --max-patch-bytes 4096` to capture a clipped patch (`patch_clipped: true`)

### 0.3 Fixture generator (script)

Create `tests/scripts/make-fixture-repo.sh` to build a tiny repository deterministically and emit fresh fixtures when needed:

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT=$(pwd)
TMP=$(mktemp -d)
cd "$TMP"

git init -q
git config user.name "Fixture Bot"
git config user.email fixture@example.com

# Commit A
mkdir -p app/models
printf "class User; end
" > app/models/user.rb
git add .
git commit -q -m "feat: add user model" --date="2025-08-12T14:03:00"
A_SHA=$(git rev-parse HEAD)

# Commit B on feature branch (unmerged)
git checkout -q -b feature/alpha
mkdir -p app/services spec/services
printf "class PaymentService; end
" > app/services/payment_service.rb
printf "describe 'PaymentService' do; end
" > spec/services/payment_service_spec.rb
git add .
git commit -q -m "refactor: extract payment service" --date="2025-08-13T09:12:00"
B_SHA=$(git rev-parse HEAD)

echo "A=$A_SHA B=$B_SHA"

# Back to main for HEAD view
git checkout -q -b main

# Run the tool in both modes to produce fixtures under tests/fixtures
cd "$ROOT"
mkdir -p tests/fixtures

# Simple
git activity-report --simple --since "2025-08-01" --until "2025-09-01" --repo "$TMP" > tests/fixtures/git-activity-report.simple.fixture.json

# Full
mkdir -p tests/fixtures/full_out
git activity-report --full --since "2025-08-01" --until "2025-09-01" \
  --repo "$TMP" --split-out tests/fixtures/full_out --include-unmerged
# Copy out the interesting files into fixtures
cp tests/fixtures/full_out/manifest.json tests/fixtures/
cp tests/fixtures/full_out/manifest-2025-08.json tests/fixtures/
cp tests/fixtures/full_out/2025-08/2025.08.12-14.03-*.json tests/fixtures/2025.08.12-14.03-aaaaaaaaaaaa.json || true
cp tests/fixtures/full_out/2025-08/unmerged/feature__alpha/2025.08.13-09.12-*.json tests/fixtures/2025.08.13-09.12-bbbbbbbbbbbb.json || true

echo "Fixture repo at: $TMP"
```

Add a Just recipe:

```just
build-fixtures:
  bash tests/scripts/make-fixture-repo.sh
```

### 0.4 Validation & goldens

Wire the following Just recipes (or keep your current ones):

- `validate-all` — Ajv validation for all schemas/fixtures.
- `golden` — quick diff against stored fixtures with stable key order:

```just
golden:
  set -euo pipefail
  mkdir -p .tmp
  # Re-run simple on the fixture repo path if known; otherwise skip.
  # Example assumes tests/scripts saved TMP path to .tmp/tmpdir
  if [ -f .tmp/tmpdir ]; then REPO=$$(cat .tmp/tmpdir); else echo "(hint) run build-fixtures first"; exit 0; fi
  git activity-report --simple --since "2025-08-01" --until "2025-09-01" --repo $$REPO > .tmp/simple.json
  diff -u <(jq -S . tests/fixtures/git-activity-report.simple.fixture.json) <(jq -S . .tmp/simple.json)
  echo "golden OK"
```

### 0.5 CI

Add a minimal GitHub Action:

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: "20" }
      - run: npm i -g ajv-cli@5 jq
      - run: just validate-all
```

(If you want golden diffs in CI, add a deterministic fixture repo script and run `just build-fixtures && just validate-all`.)

### 0.6 Tag the reference

When green:

```bash
git tag -a v0.3.0-py -m "Python reference for Rust parity (schema v2)"
git push origin v0.3.0-py
```

---

## 1) Rust port — goals

- Single static‑ish binary, fast startup, parallel commit processing.
- 1:1 CLI behavior and flags with Python version.
- Output identical to Schema v2 (byte‑stable ordering where practical).
- Packaging via Homebrew Tap + GitHub Releases.

### Proposed crates

- **CLI**: `clap` (derive) + `clap_complete` for shell completions.
- **Time**: `time` crate (format with offset), or `chrono` if preferred; detect local offset via `time`.
- **JSON**: `serde`, `serde_json`.
- **FS/Path**: `tokio` optional; `std::fs` likely sufficient.
- **Concurrency**: `rayon` for parallel per‑commit processing.
- **Git**: keep shelling out to `git` for parity (`Command`); optional future: `git2`/`gix` for lower overhead.
- **GitHub**: `octocrab` (auth via env token) with graceful fallback.

### High‑level architecture

```
src/
  main.rs                 # CLI entry
  cfg.rs                  # ReportConfiguration equivalent
  window.rs               # Time window parsing & approxidate passthrough
  gitio.rs                # Thin wrappers over `git` commands, parse outputs
  model.rs                # Serde structs for schemas (Commit, Manifest, etc.)
  enrich.rs               # GitHub PR enrichment (best‑effort)
  unmerged.rs             # Branch scanning & ahead/behind
  render.rs               # Shard writing & manifest assembly
  util.rs                 # small helpers (short_sha, filename formatting)
```

### CLI parity checklist (Python ↔ Rust)

**Goal:** Rust accepts the same flags, enforces the same constraints, and produces byte‑compatible JSON (field‑for‑field) with the Python reference.

| Flag / Arg                  | Python default & semantics                                                                                                                                             | Rust requirement                                                                                                                   | Tests (acceptance)                                                                   |                                                                                                          |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| `--repo <PATH>`             | `.` (cwd). Any Git repo path.                                                                                                                                          | Same. Resolve to abs path in JSON.                                                                                                 | Run on a tiny fixture repo; compare `repo` to abs path.                              |                                                                                                          |
| `--month YYYY-MM`           | Calendar window (start..start+1mo).                                                                                                                                    | Same.                                                                                                                              | Snapshot simple/full outputs for a fixed month.                                      |                                                                                                          |
| `--for <phrase>`            | Natural windows: `last week`, `last month`, `every month for the last N months`, `every week for the last N weeks`; else pass literal to Git approxidate (since..now). | Same set recognized; unknown phrases passed as `since` to Git with `until=now`.                                                    | Buckets count, labels, and ranges match Python.                                      |                                                                                                          |
| `--since <x>` `--until <y>` | Both required together; Git approxidate allowed.                                                                                                                       | Same. Error on only one present.                                                                                                   | Return code (2) and usage message contain both switches.                             |                                                                                                          |
| `--simple` / `--full`       | Mutually exclusive; default **simple** if neither set.                                                                                                                 | Same.                                                                                                                              | Two runs with neither set → simple; with both set → error.                           |                                                                                                          |
| `--split-out <DIR>`         | Full mode base dir; **optional** (auto‑named if missing).                                                                                                              | Same.                                                                                                                              | When omitted, dir name pattern `activity-YYYYMMDD-HHMMSS` exists; manifests present. |                                                                                                          |
| `--out <FILE>`              | Simple mode only; default `-` (stdout).                                                                                                                                | Same.                                                                                                                              | Write to tmp file; compare to stdout run.                                            |                                                                                                          |
| `--include-merges`          | Off by default; omit merge commits when off.                                                                                                                           | Same.                                                                                                                              | Compare counts with/without.                                                         |                                                                                                          |
| `--include-patch`           | Off by default; embed unified diff string under `patch`.                                                                                                               | Same; identical patch text to `git show --patch --format= --no-color`.                                                             | SHA‑stable diff equality; newline exactness.                                         |                                                                                                          |
| `--max-patch-bytes <n>`     | `0` = no limit (default). When positive, UTF‑8 truncate with `patch_clipped: true`.                                                                                    | Same semantics.                                                                                                                    | Boundary tests at 1, 4096, 0.                                                        |                                                                                                          |
| `--save-patches <DIR>`      | Write `.patch` files alongside shards (full) or into dir (simple); set `patch_ref.local_patch_file`.                                                                   | Same placement & path strings.                                                                                                     | File exists; manifest path matches.                                                  |                                                                                                          |
| `--github-prs`              | Best‑effort enrichment via `GITHUB_TOKEN` or `gh`; silent fallback.                                                                                                    | Same; never fail the run due to PR issues.                                                                                         | With token → fields present; without → absent.                                       |                                                                                                          |
| `--include-unmerged`        | Scan local branches (except current). Include commits in window reachable from branch but not from `HEAD`.                                                             | Same; identical structure in `unmerged_activity`.                                                                                  | Counts match; items/shards written under `unmerged/<branch>` (slashes → `__`).       |                                                                                                          |
| \`--tz local                | utc\`                                                                                                                                                                  | Default `local`. Controls `timestamps.author_local/commit_local` strings and `timestamps.timezone`. Epochs are UTC‑based integers. | Same.                                                                                | `local` shows offset (e.g., `-05:00`); `utc` ends with `Z` (`+00:00`), timezone label flips accordingly. |
| `--help`, `--version`       | Print usage / semver.                                                                                                                                                  | Same, with examples mirrored from README.                                                                                          | Snapshot tests.                                                                      |                                                                                                          |

**Ordering invariants**

- Commits are listed **chronologically** earliest→latest (Python uses `rev-list --date-order --reverse`).
- Shard filenames: `YYYY.MM.DD-HH.MM-<shortsha>.json` using **local time** when `--tz local`, else UTC.
- Range manifest `items[]` order mirrors commit order; unmerged branch `items[]` also chronological.

**Invalid combos & errors**

- Setting both `--simple` and `--full` → usage error.
- Providing none of (`--month` | `--for` | `--since/--until`) → usage error.
- `--out` in full mode has no effect (warn or ignore quietly to match Python).
- Non‑repo `--repo` path → error with stderr from Git.

**Exit codes**

- `0` success; `1` runtime errors (git invocation, IO), `2` CLI usage errors.

**Snapshot tests**
Add `tests/snapshots/` and two Just recipes:

```just
help-py:
  mkdir -p tests/snapshots
  # Use the in-repo Python reference for help output and normalize name
  python3 ./git-activity-report.py --help \
    | sed -e 's#\(.\)/.*git-activity-report#git-activity-report#g' \
          -e 's#git activity-report#git-activity-report#g' \
    > tests/snapshots/help.python.txt

help-rs:
  mkdir -p tests/snapshots
  target/debug/git-activity-report --help \
    | sed -e 's#\(.\)/.*git-activity-report#git-activity-report#g' \
    > tests/snapshots/help.rust.txt
  diff -u tests/snapshots/help.python.txt tests/snapshots/help.rust.txt || (echo "help output diverged"; exit 1)
```

You can also snapshot `--version`:

```just
version-snap:
  mkdir -p tests/snapshots
  python3 ./git-activity-report.py --version > tests/snapshots/version.python.txt
  target/debug/git-activity-report --version > tests/snapshots/version.rust.txt
  diff -u tests/snapshots/version.python.txt tests/snapshots/version.rust.txt || true # versions may differ
```

### Data model (Serde) mirrors (Serde) mirrors

- `Commit` with `timestamps { author, commit, author_local, commit_local, timezone }`.
- `FileEntry { file, status, old_path?, additions?, deletions? }`.
- `PatchRef { embed, git_show_cmd[], local_patch_file?, github_diff_url?, github_patch_url? }`.
- Optional `github_prs[]` (only present if enrichment succeeds).
- **RangeManifest** and **TopManifest** structs identical to Python output.

### Parallelism plan

- Collect SHAs for the window synchronously.
- Process commits in parallel (rayon `par_iter`):

  - `git show --numstat` and `--name-status` once per SHA, parse to `FileEntry[]`.
  - Optional: patch capture & clipping.
  - Optional: PR enrichment (consider rate‑limiting; may need a small async pool or sequential to avoid abuse).

- Preserve **stable output ordering** by commit time after join.

### Filename formatting & stability

- Use local time (respect `--tz`) to format `YYYY.MM.DD-HH.MM-<shortsha>.json`.
- Ensure zero‑pad; avoid locale variance.

### Error handling

- Any `git` command non‑zero → include stderr in a contextual error; continue where safe.
- PR enrichment failure → log debug, skip.
- Missing shards when writing manifests → warn and continue.

### Tests

- **Golden tests**: run the Python tool on tiny test repos (via a fixture script), capture JSON, run Rust, compare.
- **Schema validation**: validate Rust outputs with the same `tests/schemas/*.json` using `ajv` in CI.
- **Pathological cases**: binary files in diffs, huge patches with clipping, renames (`R###`), copies (`C###`).

### Packaging

- Build for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`.
- Release workflow: GitHub Actions → artifacts + Homebrew Tap formula bump.
- Binary name: `git-activity-report` (matches Python). Provide `--version` and `--help`.

---

## 2) Quality of life & feature backlog

- **Effort estimation**: optional `--estimate-effort` that appends `estimated_hours` to commits, plus total in manifest.
- **Branch filters**: `--branches "feature/*,hotfix/*"` to narrow unmerged scan.
- **Unmerged only**: `--unmerged-only` for quick in‑flight summaries.
- **Smarter workstreams**: configurable prefix → label mapping (e.g., `app/` → "Application", `infra/` → "Infrastructure").
- **Large repo speedups**: cache `git show` outputs to a temp DB when re‑running over the same window.
- **Schema versioning**: include `schema_version` in outputs to ease future evolution.

---

## 3) Milestones

1. **M0 — Freeze Python v0.3** (current): schema v2 locked, fixtures finalized, CI green.
2. **M1 — Rust MVP**: CLI parity, simple mode only, no PRs; passes schema validation for fixtures.
3. **M2 — Full mode & unmerged**: shards, manifests, branch scan complete.
4. **M3 — PR enrichment**: GitHub API wired, best‑effort, throttled.
5. **M4 — Patches & clipping**: `.patch` output and inline patch embedding.
6. **M5 — Packaging**: Homebrew tap, release automation.
7. **M6 — Optional estimation**: heuristic hours and totals; doc updates.

---

## 4) Acceptance criteria for the Rust port

- `just validate-all` passes on Rust‑generated outputs across sample repos.
- Byte‑for‑byte field parity on all required props; ordering of arrays matches Python output where applicable.
- Performance: ≥2× faster than Python reference on a 5k‑commit window (same machine), measured with patches off and on.
- UX: `--help` includes examples mirroring README.

---

## 5) Work split for agents / contributors

- **Agent A (Rust skeleton)**: project scaffolding, CLI, config, window parsing.
- **Agent B (Git IO)**: rev‑list, show parsing, name‑status/numstat merger, patch capture.
- **Agent C (Enrichment)**: GitHub PRs via `octocrab`, retries, rate limits.
- **Agent D (Render)**: shard writing, manifest assembly, filename policy.
- **Agent E (CI/Release)**: GitHub Actions, schema validation, brew tap automation.

---

## 6) Risks & mitigations

- **Locale/time surprises** → always format ISO with explicit offsets; test both `--tz local` and `--tz utc`.
- **GitHub API volatility** → keep enrichment optional; guard with timeouts and clear fallbacks.
- **Very large patches** → default clipping stays off but ensure memory‑safe streaming.
- **Cross‑platform paths** → avoid backslashes in shard filenames; normalize with `/` in manifests.

---

## 7) Tracking

Create GitHub issues labeled `rust-port` for each checklist above. Each issue links to a sample output and the corresponding schema test in CI.

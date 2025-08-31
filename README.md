# git-activity-report

Export Git activity into structured JSON — either a single report or a split‑apart dataset with per‑commit shards and an overall manifest. Optimized for feeding LLM agents and humans.

## Features

- **Natural language time windows**: `--for "last week"`, `--for "every month for the last 6 months"`, or `--month YYYY-MM`, or explicit `--since/--until` (Git approxidate supported).
- **Local‑time timestamps** by default: each commit carries epoch seconds and local ISO strings with offsets.
- **Two output styles**:

  - Single report (default): one JSON file with `commits[]`.
  - Split‑apart (`--split-apart`): per‑commit shard files plus a per‑range report (`report-<label>.json` with `items[]`), and an overall `manifest.json` for multi‑range runs.

- **Optional GitHub PR enrichment**: attaches PR metadata and `.diff`/`.patch` links when available (quietly skipped if unauthenticated).
- **Optional unmerged branch scan**: include commits in the window that are **not** reachable from `HEAD` (in‑flight work), grouped by local branch.
- **Patches**: embed in JSON (`--include-patch`, optional `--max-patch-bytes`), and/or write `.patch` files to disk (`--save-patches`).
Prototype: a Python script still lives under `prototype/` for reference, but the Rust binary is the primary implementation.

## Install / Run

- Local run (from this repo):

  ```bash
  python3 ./prototype/git-activity-report.py --help
  ```

- Optional install (to invoke as `git activity-report ...`): place the script on your PATH as `git-activity-report` and make it executable.

  ```bash
  install -m 0755 ./prototype/git-activity-report.py ~/bin/git-activity-report
  # now you can use: git activity-report --help
  ```

- Rust binary (local dev install):

  ```bash
  just install
  # now you can use: git-activity-report --help
  ```

- Man page:

  ```bash
  just man           # writes docs/man/git-activity-report.1
  just man-install   # installs into ~/.local/share/man/man1
  man git-activity-report
  ```

  The binary is named `git-activity-report`, so it also works as a Git subcommand: `git activity-report ...`.

> Optional: set `GITHUB_TOKEN` or authenticate `gh` for PR enrichment.

## Quick start

Single report for last week, include unmerged branches:

```bash
git activity-report --for "last week" --include-unmerged --repo . > last_week.json
```

Split‑apart last month with PR enrichment and patches saved:

```bash
git activity-report --split-apart --for "last month" \
  --out out/last-month \
  --github-prs \
  --save-patches out/last-month/patches
```

## CLI reference (high‑use flags)

- Time range (pick one):

  - `--month YYYY-MM`
  - `--for "last week" | "last month" | "every month for the last N months" | "every week for the last N weeks"`
  - `--since <approxidate>` and `--until <approxidate>` (aliases: `--start` / `--end`)

- Output:
  - `--split-apart` to write shards + per‑range report(s) and, for multi‑range, an overall manifest.
  - Without `--split-apart`, a single report is produced (one per run). For multi‑range runs, reports are written under `--out` and an overall manifest is still generated.
- Content:

  - `--include-merges` (off by default)
  - `--include-patch` (embed patches), `--max-patch-bytes 0` (no cap; default), `--save-patches DIR`

- Output paths:

  - `--out`: for single report, a file path (default stdout "-"); for split‑apart or multi‑range, a base directory (default: auto‑named temp dir)

- Integrations: `--github-prs`
- Unmerged work: `--include-unmerged`
- Timezone label: `--tz local|utc` (default `local`)

## Output structure

- **Timestamps** live inside each commit’s `timestamps` block:

  ```json
  {
    "author": 1693512345,
    "commit": 1693516789,
    "author_local": "2025-08-31T10:05:45-05:00",
    "commit_local": "2025-08-31T11:19:49-05:00",
    "timezone": "local"
  }
  ```

- **Single report**: one JSON object with `commits[]` and optional `unmerged_activity`.
- **Split‑apart**: `report-<label>.json` (per‑range) with `items[]` pointing to `YYYY.MM.DD-HH.MM-<shortsha>.json` shard files; for multi‑range runs, an overall `manifest.json` indexes the per‑range reports.
- **Schemas** (JSON Schema draft 2020‑12) live under `tests/schemas/`.

## Unmerged branch detection

- Scans local branches (except the current one), collects commits in the window that are **reachable from the branch but not from `HEAD`**.
- Manifests include `ahead_of_head` / `behind_head` counts and `merged_into_head` boolean when determinable.

## GitHub PR enrichment

- Enable with `--github-prs`.
- If `GITHUB_TOKEN` or `gh` auth is available, commit objects will include `github_prs[]` with `number`, `title`, `state`, `created_at`, `merged_at`, `html_url`, and convenience `diff_url`/`patch_url`.
- If unavailable or rate‑limited, enrichment is skipped silently.

## Testing & validation

- Run the test suite (unit, integration, snapshots, and JSON Schema validation):

```bash
just test
```

- Schemas live under `tests/schemas/*.json` and are validated in-process via Rust tests. ajv could also be used for manual testing.

## Examples

- Six months of monthly shards, with PR metadata and in‑flight work:

```bash
git activity-report --split-apart --for "every month for the last 6 months" \
  --repo . --out out/last6 --github-prs --include-unmerged

## Testing: freezing time

- For deterministic testing of natural-language windows, use the hidden flag `--now-override` with either RFC3339 (e.g., `2025-08-15T12:00:00Z`) or a local naive time (e.g., `2025-08-15T12:00:00`).
- Example:

```bash
git activity-report --for "last week" --repo . --tz utc \
  --now-override 2025-08-15T12:00:00
```
```

- Simple JSON for a custom window using approxidate:

```bash
git activity-report --since "2 weeks ago" --until "yesterday" --repo . > span.json
```

## Troubleshooting

- **Schema validation fails**: review failing test output from `tests/schema_validation.rs`; ensure schemas under `tests/schemas/*.json` match the produced output.
- **No PRs attached**: ensure `GITHUB_TOKEN` is set or `gh auth status` is valid.
- **No unmerged commits**: verify you actually have local branches with unique commits in the window.
- **Timestamps look wrong**: remember the ISO strings include the local offset; set `--tz utc` if you prefer UTC rendering.

## Roadmap (short)

- Rust port (binary distribution, parallel processing)
- Optional effort estimation fields in output
- Branch glob filters and `--unmerged-only`
- Homebrew tap + GitHub Releases artifacts

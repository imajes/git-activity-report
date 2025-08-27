# git-activity-report

Export Git activity into structured JSON—either a single file (simple mode) or a sharded dataset with a manifest (full mode). Optimized for feeding LLM agents to produce client‑friendly work reports.

## Features

* **Natural language time windows**: `--for "last week"`, `--for "every month for the last 6 months"`, or `--month YYYY-MM`, or explicit `--since/--until` (Git approxidate supported).
* **Local‑time timestamps** by default: each commit carries epoch seconds and local ISO strings with offsets.
* **Two modes**:

  * **Simple**: one JSON payload with `commits[]`.
  * **Full**: per‑commit shards under a labeled directory and a range manifest; a top manifest indexes multiple ranges.
* **Optional GitHub PR enrichment**: attaches PR metadata and `.diff`/`.patch` links when available (quietly skipped if unauthenticated).
* **Optional unmerged branch scan**: include commits in the window that are **not** reachable from `HEAD` (in‑flight work), grouped by local branch.
* **Patches**: embed in JSON (`--include-patch`, optional `--max-patch-bytes`), and/or write `.patch` files to disk (`--save-patches`).
* **Zero non‑stdlib deps**: requires Python 3 and Git on PATH.

## Install / Run

- Local run (from this repo):

  ```bash
  python3 ./git-activity-report.py --help
  ```

- Optional install (to invoke as `git activity-report ...`): place the script on your PATH as `git-activity-report` and make it executable.

  ```bash
  install -m 0755 ./git-activity-report.py ~/bin/git-activity-report
  # now you can use: git activity-report --help
  ```

> Optional: set `GITHUB_TOKEN` or authenticate `gh` for PR enrichment.

## Quick start

Simple, last week, include unmerged branches:

```bash
git activity-report --simple --for "last week" --include-unmerged --repo . > last_week.json
```

Full, last month, sharded, with PR enrichment and patches saved:

```bash
git activity-report --full --for "last month" \
  --split-out out/last-month \
  --github-prs \
  --save-patches out/last-month/patches
```

## CLI reference (high‑use flags)

* Time range (pick one):

  * `--month YYYY-MM`
  * `--for "last week" | "last month" | "every month for the last N months" | "every week for the last N weeks"`
  * `--since <approxidate>` and `--until <approxidate>`
* Mode: `--simple` **or** `--full`
* Content:

  * `--include-merges` (off by default)
  * `--include-patch` (embed patches), `--max-patch-bytes 0` (no cap; default), `--save-patches DIR`
* Output paths:

  * `--out FILE` (simple mode, default stdout)
  * `--split-out DIR` (full mode base directory)
* Integrations: `--github-prs`
* Unmerged work: `--include-unmerged`
* Timezone label: `--tz local|utc` (default `local`)

## Output structure

* **Timestamps** live inside each commit’s `timestamps` block:

  ```json
  {
    "author": 1693512345,
    "commit": 1693516789,
    "author_local": "2025-08-31T10:05:45-05:00",
    "commit_local": "2025-08-31T11:19:49-05:00",
    "timezone": "local"
  }
  ```

* **Simple mode**: one JSON object with `commits[]` and optional `unmerged_activity`.
* **Full mode**: `manifest-<label>.json` indexes shard files named `YYYY.MM.DD-HH.MM-<shortsha>.json`; a top `manifest.json` indexes multiple ranges.
* **Schemas** (JSON Schema draft 2020‑12) live under `tests/schemas/`.

## Unmerged branch detection

* Scans local branches (except the current one), collects commits in the window that are **reachable from the branch but not from `HEAD`**.
* Manifests include `ahead_of_head` / `behind_head` counts and `merged_into_head` boolean when determinable.

## GitHub PR enrichment

* Enable with `--github-prs`.
* If `GITHUB_TOKEN` or `gh` auth is available, commit objects will include `github_prs[]` with `number`, `title`, `state`, `created_at`, `merged_at`, `html_url`, and convenience `diff_url`/`patch_url`.
* If unavailable or rate‑limited, enrichment is skipped silently.

## Testing & validation

A `Justfile` provides validation using `ajv-cli` (Draft 2020‑12):

```bash
just validate-all
```

Fixtures and schemas:

```
./tests/schemas/*.json
./tests/fixtures/*.json
```

## Examples

* Six months of monthly shards, with PR metadata and in‑flight work:

```bash
git activity-report --full --for "every month for the last 6 months" \
  --repo . --split-out out/last6 --github-prs --include-unmerged
```

* Simple JSON for a custom window using approxidate:

```bash
git activity-report --simple --since "2 weeks ago" --until "yesterday" --repo . > span.json
```

## Troubleshooting

* **Schema validation fails**: confirm you used the current schema (`tests/schemas/.....`) and `ajv --spec=draft2020`.
* **No PRs attached**: ensure `GITHUB_TOKEN` is set or `gh auth status` is valid.
* **No unmerged commits**: verify you actually have local branches with unique commits in the window.
* **Timestamps look wrong**: remember the ISO strings include the local offset; set `--tz utc` if you prefer UTC rendering.

## Roadmap (short)

* Rust port (binary distribution, parallel processing)
* Optional effort estimation fields in output
* Branch glob filters and `--unmerged-only`
* Homebrew tap + GitHub Releases artifacts

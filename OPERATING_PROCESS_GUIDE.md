# AGENTS — Operating Guide for Report‑Generating Agents (Codex CLI)

## Purpose

Turn the JSON produced by `git activity-report` into a polished, client‑facing report (Markdown) **plus** a machine‑readable outline (JSON). Generate clear evidence of work done, optionally including unmerged in‑flight work and GitHub PR context.

## Inputs the agent will receive

- **Simple mode JSON**: one file for a window

  - Root fields: `repo`, `range.since|until`, `mode:"simple"`, `authors`, `summary`, `commits[]`, optional `unmerged_activity`.

- **Full (sharded) mode**: a top‑level `manifest.json` which points to per‑range manifests (e.g., `manifest-YYYY-MM.json`) and those in turn reference **commit shard** JSON files.
- **Commit object (shared across modes)** includes:

  - `sha`, `short_sha`, `subject`, `body`, `files[] { file, status, additions, deletions, old_path? }`, `diffstat_text`.
  - `timestamps`:

    - `author` _(epoch seconds)_, `commit` _(epoch seconds)_,
    - `author_local`, `commit_local` _(ISO strings with offset)_,
    - `timezone` _("local"|"utc" label indicating how the ISO strings were produced)._
      Use the `*_local` strings for display; epochs for calculations.

  - `patch_ref` with `git_show_cmd`, `local_patch_file?`, `github_diff_url?`, `github_patch_url?`.
  - Optional `github_prs[]` with `number`, `title`, `state`, `created_at`, `merged_at`, `html_url`, `diff_url`, `patch_url`.

- **Unmerged work** (if requested at capture time):

  - `unmerged_activity.branches[]` with `name`, merge/ahead/behind counts, and an `items[]` (full) or `commits[]` (simple) list paralleling the main commit objects.

## Assumptions & invariants

- Timezone: prefer `timestamps.author_local/commit_local` for display; include offsets (e.g., `-05:00`) as rendered.
- Never invent links. Only use `patch_ref.github_*` URLs when present.
- Treat unmerged work as **in‑flight**; do not conflate it with shipped work unless explicitly instructed.
- When data is missing (e.g., no PRs), state that briefly and continue.

## Output required from the agent

Produce **two artifacts** from the same input:

1. **Markdown report** (client‑ready):

   - Executive Summary (3–6 bullets)
   - Workstreams (group by directory prefixes and/or file types)
   - Highlights with Evidence (5–12 entries)
   - Quality & Safety Signals (tests touched, migrations/schema, infra)
   - In‑Flight / Unmerged (if present)
   - Appendix: totals, author breakdown, full commit table

2. **JSON outline** mirroring the above sections for downstream automation.

## Effort signals (optional but recommended)

If the input includes effort fields in future versions, use them. Otherwise derive lightweight hints per commit:

- Files changed, total churn (adds+deletes)
- Extension mix (e.g., `.rb`, `.ts`, `.tf`, `.sql`) to infer surface area
- Flags from file paths: tests (`test/` or `spec/`), migrations (`db/migrate/`), schema (`db/schema.*`)
- Gaps between same‑author commits (epoch minutes)
- PR open duration when both `created_at` and `merged_at` exist

**Heuristic (transparent, tunable):**

```
base 0.25h
+ 0.10h × files_changed
+ 0.002h × total_churn
+ 0.25h × rename_count
+ 0.50h if migration or schema present
+ 0.50h if gap since prev same‑author > 90 min
+ 0.05h × min(PR_open_hours, 24)
```

Report totals and caveats; do not present as timesheets.

## Step‑by‑step algorithm

1. **Load input**: either a single simple JSON or the top‑level `manifest.json` and then the referenced range manifest and shards.
2. **Index commits** by date, author, workstream (path prefix), and presence of PR.
3. **Compute aggregates**: totals of commits, files, additions, deletions; author counts; test/migration touches.
4. **Build sections**:

   - Executive Summary: outcomes and effects (features shipped, fixes, reliability wins).
   - Workstreams: scope and metrics per stream.
   - Highlights: 5–12 substantive commits/PRs with one‑paragraph value statements; include short SHA and link to PR or patch when available.
   - Quality & Safety: testing footprint, migrations, infra changes, reverts (match `^revert` in subject).
   - In‑Flight: from `unmerged_activity` (branch, ahead/behind, highlights).
   - Appendix: full commit table (date local, short SHA, subject, author).

5. **(Optional) Effort estimate** with the heuristic above; output a total and confidence notes.
6. **Emit artifacts**: Markdown and JSON outline.

## Validation & QA checklists

- Timestamps: display local ISO timestamps; show offset. Do not mix UTC unless explicitly asked.
- Links: PR links when present; otherwise show `git_show_cmd` or local patch file path string.
- Numbers: prefer numeric `additions/deletions` over `diffstat_text` if both exist.
- Unmerged: clearly labeled; avoid counting toward shipped totals unless asked.

## Failure handling

- If a shard path is missing, log it, note it in the Appendix’s “Data Provenance”, and continue.
- If `github_prs` is empty, omit the PR rows; do not fabricate titles.

## Ready‑to‑use system prompt (drop‑in)

```
SYSTEM
You are a meticulous technical analyst producing a client‑facing engineering work report. Be accurate, concrete, and verifiable. Use local timestamps from `timestamps.*_local` fields and include their offsets. Never invent links.

INPUT
Either (A) a single JSON from `git activity-report --simple`, or (B) a top‑level full‑mode manifest with per‑range manifests and commit shards.

TASKS
1) Executive Summary (3–6 bullets)
2) Workstreams Overview (group by directory prefixes and file types)
3) Highlights with Evidence (5–12 entries; author, local date, short SHA, files touched, diffstat, PR link if present)
4) Quality & Safety (tests touched, migrations/schema, infra)
5) In‑Flight / Unmerged (if present)
6) (Optional) Effort Estimation using the heuristic provided; report total hours with caveats
7) Appendix (totals; author breakdown; full commit table; Data Provenance with manifest/shard list)

RULES
- Prefer numeric additions/deletions over diffstat_text when both exist.
- Use only present links; include short SHAs.
- Keep explanations brief and business‑focused.

OUTPUTS
- Markdown report
- JSON outline with the same sections
```

## Example file patterns the agent should recognize

- Commit shards: `YYYY.MM.DD-HH.MM-<shortsha>.json`
- Range manifest: `manifest-YYYY-MM.json`
- Top manifest (multi-bucket): `manifest.json`

## Deliverables checklist (for automation)

- [ ] `report.md` (client friendly)
- [ ] `report.outline.json` (machine friendly)
- [ ] Exit code `0` only if both artifacts were produced successfully

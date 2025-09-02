# Effort Estimation Plan

Purpose

- Add a first, pragmatic effort estimation feature that predicts time (minutes) spent on coding tasks using the data already produced by this system. Keep it additive and optional, contract‑safe, and aligned with the repo’s orchestration and spacing rules.

Guiding Principles

- Contract stability first: new fields are optional; no breaking schema changes.
- Orchestration simplicity: compute estimates in one pass alongside existing enrichment.
- Correctness/testability: pure helpers; clear feature extraction; predictable math.
- Performance: reuse computed data; avoid extra heavy calls; best‑effort under flags.
- Readability: follow prompt-engineering/SPACING_SPEC.md and LAYOUT_SPEC.md during authoring.

What We Can Use (Available Signals)

- Commit‑level: file list (paths, statuses, additions/deletions), patch size (bytes), diffstat text, subject/body, timestamps, merge flag (implied by parents > 1), optional embedded patch.
- PR‑level (when enabled): number/title/state, timestamps (created/merged/closed), links, submitter/user, approver (reviews or merged_by), list of PR commits (sha + subject), and commit‑to‑PR mapping via sha.
- Unmerged activity: local branches with in‑flight commits.

Concept: Estimating “Time Spent” From Git Signals

- Goal: a rough, explainable estimate of developer time in minutes per unit (commit, PR, range) with a confidence score.
- Shape: a small struct with minutes (f64), confidence (0–1), and an explanation/basis string summarizing the dominant factors.
- Strategy: rule‑based + light math; no model training at first. Add calibration hooks later.

Feature Extraction (Per Commit)

- Size: total changed lines = sum(additions + deletions) across files (ignore missing stats).
- Scope: number of files changed; number of rename/moves; number of binary files.
- Churn type: ratio of added:deleted; presence of large deletions/refactors.
- Tests ratio: % of touched files under test directories or with test‑like names.
- Languages: weight by extension (rust/ts/go higher cognitive; md/json lower).
- Patch structure: number of hunks (optional if patch embedded); otherwise estimate by size.
- Merge commits: treat as 0 minutes (effort hidden in branch; reported at PR level).

Heuristic Mapping (Minutes)

- Base minutes: 5.0 per commit (context + staging + message).
- Files coefficient: +0.75 per file up to 20, then +0.25 per additional file.
- Lines coefficient (diminishing): +sqrt(total_lines) × 0.9.
- Language weights (multiply subtotal):
  - Rust/Go/TypeScript: ×1.25; Python/JS: ×1.1; Markdown/JSON/YAML: ×0.8.
- Tests ratio: ×[0.85–1.0] if mostly test‑only; ×[1.0–1.1] if mixed with prod.
- Rename‑heavy (>50% R###): ×0.7 (less cognitive load; mechanical).
- Large deletions (>70% deletions): ×0.8 (cleanup)
- Cap/smoothing: clamp minutes to [1, 240] for a single commit estimate.

PR‑Level Estimation

- Sum commit estimates for commits that belong to the PR (match sha from PR commits against range commits).
- PR overheads:
  - Review cycles: +6–12 minutes per APPROVED review; +3–8 per COMMENTED/CHANGES_REQUESTED.
  - Rebase/churn overhead: +0.2 min per file per additional review beyond first.
  - Coordination/context: +10 minutes flat for PR assembly + description.
- Cycle‑time bounds (if created_at/merged_at present):
  - Treat as an upper bound for active time; do not exceed 35–50% of cycle duration on average.
  - Confidence nudged upward when cycle‑time present.

Aggregation Across Ranges

- Per author totals with daily/weekly rollups.
- Split‑apart mode: include per‑shard estimates; range report aggregates to a summary.

Contracts & Output Fields (Proposed; Additive)

- Commit: estimated_minutes (number), estimate_confidence (number 0–1), estimate_basis (string short note).
- PR: estimated_minutes, estimate_confidence, estimate_basis, plus breakdown (reviews, commits subtotal).
- Report: summary.estimated_minutes_total, authors_minutes[author] (optional map).

Rollout Plan (Phased)

1. Sketch (this change)

- Implement pure helpers under src/enrichment/effort.rs.
- No wiring yet; no schema updates; safe to compile without output changes.

2. Opt‑in Wiring (behind flag)

- Add `--estimate-effort` (implied by `--detailed` optionally).
- Populate commit‑level estimates when enabled; aggregate to PR and range.
- Update JSON Schemas additively; add tests and snapshots.

3. Calibration + Refinement

- Add a config/weights table; expose env overrides.
- Per‑repo calibration: compute “minutes per 100 lines” baseline from history (exclude merges/renames), bounded by percentiles.
- Validate against PR cycle times; tune multipliers.

4. Documentation + Examples

- Document strategy, tradeoffs, and known biases (meetings, pairing, tooling automation).
- Provide example outputs and a knob to disable per‑commit to keep noise low.

Quality & Spacing/Orchestration Notes

- Keep IO at edges: estimation works on in‑memory model objects; no network calls.
- Single loop: run estimation during the same pass as other enrichments; store results then persist as usual.
- Readability: extract‑before‑build; one concern per statement; blank lines between phases.

Open Questions

- Should estimate reflect task management (e.g., commits from different days => multiple sessions)?

RESPONSE: Yes, very much so. Multiple days should absolutely be a dragging factor - for each day, add in a small bump for time taken to recalibrate on the task, and time taken for reporting back to business.

- Should review time be attributed to submitter, approver, or both? Initial approach: attribute submitter and optionally attribute a small share (e.g., 30%) to reviewers.

RESPONSE: Yes, to both. I agree with the approach idea.

- Should we surface a band (min..max) instead of a point estimate? For now: point + confidence.

RESPONSE: Yes, this is smart. Can we do a min..max band AND the point estimate?

Next Steps

- Wire commit‑level estimator behind a flag; extend schemas and tests.
- Implement PR review overhead with real review events in aggregator.
- Add author rollups in the report summary.

Implementation Notes (current state)

- CLI knobs (calibration):
  - `--estimate-review-approved-minutes` (default 9.0)
  - `--estimate-review-changes-minutes` (default 6.0)
  - `--estimate-review-commented-minutes` (default 4.0)
  - `--estimate-files-overhead-per-review-minutes` (default 0.2 × files)
  - `--estimate-day-drag-minutes` (default 7.0)
  - `--estimate-pr-assembly-minutes` (default 10.0)
  - `--estimate-approver-only-minutes` (default 10.0)
  - `--estimate-cycle-time-cap` (default 0.5 of cycle time)
- Attribution:
  - `simple.report.authors_minutes` aggregates commit‑level minutes by author.
  - `github_pr.reviewers_minutes_by_github_login` breaks down review minutes by reviewer login per PR.
  - `simple.report.reviewers_minutes` aggregates reviewer minutes across all PRs.
- Debug (`--verbose`):
  - Per‑commit and per‑PR summary lines; per‑reviewer lines `reviewer <login>: +Xm`.

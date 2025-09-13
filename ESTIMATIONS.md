# Effort Estimation (Additive, Optional)

Purpose

- Provide an explainable, best‑effort estimate of time spent (in minutes) for commits and PRs using only data already captured by the tool. Estimates are optional and gated by `--estimate-effort` (also implied by `--detailed`).

What it computes

- Commit: estimated_minutes (+ min/max band, confidence, and a short basis string summarizing dominant factors).
- PR: sums commit estimates that belong to the PR, then adds review/coordination overhead and a small multi‑day drag; bounds by a fraction of the PR cycle time when available.

Heuristic (v0)

- Base: 5.0 minutes per commit.
- Files: +0.75 min per file up to 20, then +0.25 per additional file.
- Lines: +sqrt(additions + deletions) × 0.9.
- Language weighting: Rust/Go/TS ×1.25; Python/JS ×1.1; Markdown/JSON/YAML ×0.8.
- Discounts: rename‑heavy (>50% R###) ×0.7; heavy deletions (>70%) ×0.8.
- Tests: mostly tests (≥80%) ×0.9; mixed tests ×1.05.
- PR overheads: APPROVED +9m; CHANGES_REQUESTED +6m; COMMENTED +4m; extra reviews add +0.2m × files; PR assembly +10m; approver only +10m.
- PR cycle‑time cap: ≤ 50% of wall‑clock from created_at → merged_at when both known.

Contracts

- JSON fields are additive and optional:
  - Commit: `estimated_minutes`, `estimated_minutes_min`, `estimated_minutes_max`, `estimate_confidence`, `estimate_basis`.
  - PR: same fields attached to each PR object under `commit.github.pull_requests[]`.
- Units are minutes (floating point). Downstream can present hours if desired.

Usage

```bash
git activity-report --for "last week" --repo . --estimate-effort > out.json
# or:
git activity-report --detailed --for "last week" --repo . > out.json
```

Caveats

- Estimates do not include meetings, pairing, deployment toil, or context outside commits/reviews.
- Merge commits show 0 minutes; effort is attributed to the PR and constituent commits.
- Confidence is indicative only; treat results as planning/communication aids, not timesheets.


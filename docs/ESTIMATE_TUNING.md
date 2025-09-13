# Exploring and Tuning Effort Estimates

This guide shows how to explore, critique, and tune the effort estimation heuristics when numbers feel too optimistic (too low) or too pessimistic. The estimators are explainable and designed to be easy to calibrate.

## What’s tunable today

Two sets of weights drive estimates (see `src/enrichment/effort.rs`).

- Commit weights (per‑commit):
  - `base_commit_min` (default 5.0)
  - `per_file_min` (0.75)
  - `per_file_tail_min` (0.25; applies after 20 files)
  - `sqrt_lines_coeff` (0.9; minutes added as `sqrt(add+del) * coeff`)
  - Discounts/uplifts: `rename_discount` (0.7), `heavy_delete_discount` (0.8),
    `test_only_discount` (0.9), `mixed_tests_uplift` (1.05)

- PR weights (per‑PR; add overhead to commit subtotal):
  - `pr_assembly_min` (10.0)
  - `review_approved_min` (9.0), `review_changes_min` (6.0), `review_commented_min` (4.0)
  - `files_overhead_per_review_min` (0.2 × files × extra_reviews)
  - `day_drag_min` (7.0 per extra day the PR spans across its commits)
  - `cycle_time_cap_ratio` (0.5; cap = 50% of wall‑clock `created_at → merged_at`)

Currently, the defaults live in one place for easy tuning (no magic numbers):

- src/enrichment/effort.rs
  - EffortWeights::default and PrEstimateParams::default (semantic multipliers/overheads)
  - tuning::* constants (thresholds, banding, and PR defaults)

CLI flags are intentionally not exposed yet to keep the surface small; use the single edit points above.

## Workflow: measure → tweak → compare

1) Produce a baseline

```bash
# Last week, UTC, stable "now" for reproducibility
cargo run -- --for "last week" --repo . --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-baseline.json

# With PRs (token or gh auth required)
cargo run -- --for "last week" --repo . --github-prs --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-pr-baseline.json
```

2) Inspect totals and distributions

```bash
# Commit totals
jq '[.commits[].estimated_minutes // 0] | {total: add, mean: (add/length)}' out-baseline.json

# p50/p90 minutes per commit
jq '[.commits[].estimated_minutes // 0] | sort | {p50: .[length*0.5|floor], p90: .[length*0.9|floor]}' out-baseline.json

# PR totals (when github_prs is on)
jq '[.commits[] | select(.github!=null) | .github.pull_requests[] | .estimated_minutes // 0]    | {total: add, mean: (add/length)}' out-pr-baseline.json

# PR cycle‑time vs. estimate (sanity): ratio <= cap (default 0.5)
jq -r '.commits[] | select(.github!=null) | .github.pull_requests[]   | select(.time_to_merge_seconds!=null)   | {n: .number, minutes: .estimated_minutes, wall_mins: (.time_to_merge_seconds/60)}' out-pr-baseline.json
```

3) Tweak weights & constants (local patch) and rebuild

Edit `src/enrichment/effort.rs` to change the defaults under `EffortWeights` and `PrEstimateParams` (both have `impl Default` blocks). Rebuild and re-run the same windows.

Common adjustments when estimates feel low (optimistic):

- Commit‑level
  - Increase `base_commit_min` from 5 → 8–12.
  - Increase `per_file_min` from 0.75 → 1.0–1.5.
  - Increase `sqrt_lines_coeff` from 0.9 → 1.1–1.3.
  - Soften discounts (closer to 1.0): `rename_discount` 0.7 → 0.9, `heavy_delete_discount` 0.8 → 0.95.
  - Reduce test discount: `test_only_discount` 0.9 → 1.0; raise `mixed_tests_uplift` 1.05 → 1.15.

- PR‑level
  - Increase `pr_assembly_min` (tuning::PR_ASSEMBLY_MIN) from 10 → 15–20.
  - Increase review minutes: APPROVED (tuning::PR_REVIEW_APPROVED_MIN) 9 → 12, CHANGES 6 → 10, COMMENTED 4 → 6.
  - Increase `files_overhead_per_review_min` (tuning::PR_FILES_OVERHEAD_PER_REVIEW_MIN) 0.2 → 0.4–0.6.
  - Increase `day_drag_min` (tuning::PR_DAY_DRAG_MIN) 7 → 10–12.
  - If estimates get clipped too low by wall‑time bounds, increase `cycle_time_cap_ratio` (tuning::PR_CYCLE_TIME_CAP_RATIO) 0.5 → 0.7.

Rebuild and compare with your baseline outputs.

```bash
cargo build
cargo run -- --for "last week" --repo . --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-tuned.json

jq '{base: ([.commits[].estimated_minutes // 0] | add), 
     tuned: (input | [.commits[].estimated_minutes // 0] | add)}'   out-baseline.json out-tuned.json
```

4) Validate PRs match expectations

If PR estimates still look low despite matched commits:

- Confirm the window includes the PR’s commits (basis shows `commits_matched=N`). Choose a wider `--for` or `--since/--until` so matched commits reflect the PR contents.
- Raise `pr_assembly_min`, review minutes, and `day_drag_min` as needed.
- Consider increasing `cycle_time_cap_ratio` if your organization’s “active time” tends to be a larger fraction of wall time.

## Presets to try (edit EffortWeights::default / PrEstimateParams::default)

- “Moderate” (more realistic for many teams)
  - Commits: base 10.0, per_file 1.25, tail 0.4, sqrt_coeff 1.2, rename 0.9, del 0.95, test_only 1.0, mixed 1.15
  - PRs: assembly 18, approved 12, changes 10, commented 6, files_overhead 0.5, day_drag 12, cap 0.7

- “Conservative” (upper‑bound planning)
  - Commits: base 12.0, per_file 1.5, tail 0.5, sqrt_coeff 1.3, rename 0.95, del 1.0, test_only 1.05, mixed 1.2
  - PRs: assembly 25, approved 15, changes 12, commented 8, files_overhead 0.7, day_drag 15, cap 0.8

## Optional: add runtime overrides (future work)
Runtime overrides are available via environment variables (no rebuild):

```
# Commit weights
export GAR_EST_BASE_COMMIT_MIN=10
export GAR_EST_PER_FILE_MIN=1.25
export GAR_EST_PER_FILE_TAIL_MIN=0.4
export GAR_EST_SQRT_LINES_COEFF=1.2
export GAR_EST_RENAME_DISCOUNT=0.9
export GAR_EST_HEAVY_DELETE_DISCOUNT=0.95
export GAR_EST_TEST_ONLY_DISCOUNT=1.0
export GAR_EST_MIXED_TESTS_UPLIFT=1.15
export GAR_EST_COG_BASE_MIN=12
export GAR_EST_COG_EXT_MIX_COEFF=0.4
export GAR_EST_COG_DIR_MIX_COEFF=0.4
export GAR_EST_COG_BALANCED_EDIT_COEFF=0.1
export GAR_EST_COG_LANG_COMPLEXITY_COEFF=0.1

# PR overheads
export GAR_EST_PR_REVIEW_APPROVED_MIN=12
export GAR_EST_PR_REVIEW_CHANGES_MIN=10
export GAR_EST_PR_REVIEW_COMMENTED_MIN=6
export GAR_EST_PR_FILES_OVERHEAD_PER_REVIEW_MIN=0.5
export GAR_EST_PR_DAY_DRAG_MIN=12
export GAR_EST_PR_ASSEMBLY_MIN=18
export GAR_EST_PR_APPROVER_ONLY_MIN=12
export GAR_EST_PR_CYCLE_TIME_CAP_RATIO=0.7
```

Unset to return to defaults:

```
env -u GAR_EST_BASE_COMMIT_MIN -u GAR_EST_PER_FILE_MIN \
  -u GAR_EST_PR_ASSEMBLY_MIN -- cargo run -- …
```

## Tips & gotchas

- Estimates are minutes; downstream can format hours.
- Merge commits are treated as 0 (effort is attributed to the PR/branch work).
- “Optimistic” outputs often mean: small `base_commit_min`, too‑low `sqrt_lines_coeff`, minimal PR overheads, or `cycle_time_cap_ratio` capping too hard.
- Use `--now-override` and `--tz` to make runs reproducible across machines.

## Appendix: quick jq snippets

- Top 10 heaviest commits by estimated minutes

```bash
jq '.commits | sort_by(.estimated_minutes // 0) | reverse | .[:10]    | map({sha: .short_sha, subject, minutes: .estimated_minutes, basis: .estimate_basis})' out-tuned.json
```

- PRs with estimates and review counts

```bash
jq '.commits[] | select(.github!=null) | .github.pull_requests[]   | {n: .number, title: .title, minutes: .estimated_minutes,      reviews: .review_count, approvals: .approval_count, basis: .estimate_basis}' out-pr-baseline.json
```

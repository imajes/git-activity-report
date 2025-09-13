# Estimation Tuning Discovery — A/B Snapshot

Context

- Window: last week (relative to a fixed now)
- Now (override): 2025-09-15T12:00:00 (UTC)
- Repo: this repository
- Goal: compare baseline estimator vs. a more conservative profile to better account for cognitive overhead and PR process time.

Reproduction

- Baseline (no env overrides):

```
cargo run -- --for "last week" --repo . --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-baseline.json

# With PRs (requires token or gh auth)
cargo run -- --for "last week" --repo . --github-prs --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-pr-baseline.json
```

- Conservative profile (set in shell, no rebuild required):

```
# Commit weights
export GAR_EST_BASE_COMMIT_MIN=12
export GAR_EST_PER_FILE_MIN=1.5
export GAR_EST_PER_FILE_TAIL_MIN=0.5
export GAR_EST_SQRT_LINES_COEFF=1.3
export GAR_EST_RENAME_DISCOUNT=0.95
export GAR_EST_HEAVY_DELETE_DISCOUNT=1.0
export GAR_EST_TEST_ONLY_DISCOUNT=1.05
export GAR_EST_MIXED_TESTS_UPLIFT=1.2
export GAR_EST_COG_BASE_MIN=12
export GAR_EST_COG_EXT_MIX_COEFF=0.4
export GAR_EST_COG_DIR_MIX_COEFF=0.4
export GAR_EST_COG_BALANCED_EDIT_COEFF=0.1
export GAR_EST_COG_LANG_COMPLEXITY_COEFF=0.1

# PR overheads
export GAR_EST_PR_REVIEW_APPROVED_MIN=15
export GAR_EST_PR_REVIEW_CHANGES_MIN=12
export GAR_EST_PR_REVIEW_COMMENTED_MIN=8
export GAR_EST_PR_FILES_OVERHEAD_PER_REVIEW_MIN=0.7
export GAR_EST_PR_DAY_DRAG_MIN=15
export GAR_EST_PR_ASSEMBLY_MIN=25
export GAR_EST_PR_APPROVER_ONLY_MIN=12
export GAR_EST_PR_CYCLE_TIME_CAP_RATIO=0.8

cargo run -- --for "last week" --repo . --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-tuned.json

# With PRs
cargo run -- --for "last week" --repo . --github-prs --estimate-effort --tz utc   --now-override 2025-09-15T12:00:00 > out-pr-tuned.json
```

Results

- Commit estimates (from this repo, UTC, last week):
  - Baseline: count=18, total≈419.64, mean≈23.31, min≈9.24, max≈56.07
  - Conservative: count=18, total≈726.96, mean≈40.39, min≈17.70, max≈95.06
  - Delta: mean +≈73% (23.31 → 40.39)

- PR estimates (deduped by PR number):
  - Baseline: count=2, total≈192.39, mean≈96.20, min=3.0, max≈189.39
  - Conservative: count=2, total≈347.52, mean≈173.76, min≈4.80, max≈342.72
  - Delta: mean +≈80%

Interpretation

- Adding explicit cognitive overhead (breadth, directory/extension diversity, balanced edits, language complexity) and raising PR process overheads produce materially higher, more realistic numbers for this repository sample.
- The run uses banded outputs (min/max) and cycle‑time caps for PRs; increasing `PR_CYCLE_TIME_CAP_RATIO` to 0.8 reduces clipping when wall time is not a limiting factor.

Notes & Next Steps (optional)

- Keep defaults moderate but provide documented presets and env overrides for teams to calibrate per repo.
- If the conservative profile aligns better with intuition across more repos, consider moving some values into the compiled defaults in a future cycle.

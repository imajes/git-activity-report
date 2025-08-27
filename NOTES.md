## Upgrade Ideas

- Local timezone by default: output local ISO timestamps unless you pass --tz utc.
- Effort signals + optional naive estimate:
  - Always include effort_hints (files changed, churn, gaps between commits, extension mix, migration/test flags, PR open time when known).
  - Optional --estimate-effort computes a lightweight hours estimate per commit (tunable weights).
- PR duration: when PR data exists, include created_at, merged_at, and open duration—perfect fodder for the heuristic.
- No drama GitHub: --github-prs stays best-effort; if unauthenticated/rate-limited, we keep going.
- Natural windows stay as-is: “last week”, “every month for the last 6 months”… and we still fall back to Git’s approxidate for odd phrases.
- Add a small fixture suite (tiny repos) and a “golden JSON” test to lock behavior.

## Rust Port Notes

- Keep shelling to git (simple, matches Python), or use git2 if you need finer-grained diff APIs.
- Use clap, serde, serde_json, time, and optionally octocrab.
- Parallelize per-commit processing with rayon.

## incantation note

git activity-report --full \
 --for "every month for the last 6 months" \
 --repo . \
 --github-prs \
 --split-out out/last6 \
 --save-patches out/last6/patches

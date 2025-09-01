// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Coordinator to apply configured enrichments; keeps orchestration separate from enrichment implementations
// role: enrichment/coordinator
// outputs: Mutates Commit (per-commit enrichments) and returns optional aggregated enrichments for reports
// invariants: Best-effort; avoids hard failures; integration details live under `crate::enrichment` modules
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===
use crate::model::{Commit, GithubPullRequest};

/// Apply per-commit enrichments based on flags.
pub fn apply_commit_enrichments(commit: &mut Commit, repo: &str, github_prs: bool) {
  if github_prs {
    crate::enrichment::github_pull_requests::enrich_with_github_prs(commit, repo);
  }
}

/// Aggregate report-level enrichments based on flags.
pub fn aggregate_report_enrichments(commits: &[Commit], repo: &str, github_prs: bool) -> Option<Vec<GithubPullRequest>> {
  if !github_prs {
    return None;
  }
  crate::enrichment::github_pull_requests::collect_pull_requests_for_commits(commits, repo)
}

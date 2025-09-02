// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Coordinator to apply configured enrichments; keeps orchestration separate from enrichment implementations
// role: enrichment/coordinator
// outputs: Mutates Commit (per-commit enrichments) and returns optional aggregated enrichments for reports
// invariants: Best-effort; avoids hard failures; integration details live under `crate::enrichment` modules
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===
#[cfg(any(test, feature = "testutil"))]
use crate::model::{Commit, GithubPullRequest};

/// Apply per-commit enrichments based on flags.
#[cfg(any(test, feature = "testutil"))]
pub fn apply_commit_enrichments(commit: &mut Commit, repo: &str, github_prs: bool) {
  if github_prs {
    crate::enrichment::github_pull_requests::enrich_with_github_prs(commit, repo);
  }
}

/// Aggregate report-level enrichments based on flags.
#[cfg(any(test, feature = "testutil"))]
pub fn aggregate_report_enrichments(commits: &[Commit], repo: &str, github_prs: bool) -> Option<Vec<GithubPullRequest>> {
  if !github_prs {
    return None;
  }
  crate::enrichment::github_pull_requests::collect_pull_requests_for_commits(commits, repo)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn minimal_commit() -> Commit {
    Commit {
      sha: "deadbeef".into(),
      short_sha: "deadbee".into(),
      parents: vec![],
      author: crate::model::Person { name: "A".into(), email: "a@ex".into(), date: "".into() },
      committer: crate::model::Person { name: "A".into(), email: "a@ex".into(), date: "".into() },
      timestamps: crate::model::Timestamps {
        author: 0,
        commit: 0,
        author_local: "".into(),
        commit_local: "".into(),
        timezone: "utc".into(),
      },
      subject: "s".into(),
      body: "".into(),
      files: vec![],
      diffstat_text: "".into(),
      patch_references: crate::model::PatchReferences { embed: false, git_show_cmd: "".into(), local_patch_file: None, github: None },
      patch_clipped: None,
      patch_lines: None,
      body_lines: None,
      github: None,
    }
  }

  #[test]
  fn coordinator_noop_when_flag_disabled() {
    let mut c = minimal_commit();
    apply_commit_enrichments(&mut c, ".", false);
    assert!(c.github.is_none());
  }

  #[test]
  fn aggregator_none_when_flag_disabled() {
    let out = aggregate_report_enrichments(&[], ".", false);
    assert!(out.is_none());
  }
}

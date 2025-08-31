use crate::enrich;
use crate::model::Commit;

/// Enriches a commit with its associated GitHub Pull Request info.
pub fn enrich_with_github_prs(commit: &mut Commit, repo: &str) {
  if let Ok(prs) = enrich::try_fetch_prs(repo, &commit.sha) {
    if let Some(first_pr) = prs.first() {
      commit.patch_ref.github_diff_url = first_pr.diff_url.clone();
      commit.patch_ref.github_patch_url = first_pr.patch_url.clone();
    }
    commit.github_prs = Some(prs);
  }
}


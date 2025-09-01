// purpose: Best-effort enrichment adding GitHub PR links and PR list to a commit
// role: enrichment/integration
// inputs: &mut Commit, repo path
// outputs: Mutated commit.patch_ref (diff/patch URLs) and commit.github_prs
// side_effects: Network or local API calls inside enrich::try_fetch_prs (best-effort)
// invariants:
// - On success, preserves existing commit fields; sets URLs if present in first PR; attaches PR list
// - On failure, commit remains valid; fields untouched
// errors: None propagated (best-effort); enrichment failures are ignored
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
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

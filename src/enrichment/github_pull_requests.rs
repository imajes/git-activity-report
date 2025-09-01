// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Best-effort enrichment adding GitHub PR links and PR list to a commit
// role: enrichment/integration
// inputs: &mut Commit, repo path
// outputs: Mutated commit.patch_ref (diff/patch URLs) and commit.github_prs
// side_effects: Network or local API calls inside github_api::try_fetch_prs_for_commit (best-effort)
// invariants:
// - On success, preserves existing commit fields; sets URLs if present in first PR; attaches PR list
// - On failure, commit remains valid; fields untouched
// errors: None propagated (best-effort); enrichment failures are ignored
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use crate::enrichment::github_api as ghapi;
use crate::ext::serde_json::JsonFetch;
use crate::model::{Commit, GithubPullRequest};

/// Enriches a commit with its associated GitHub Pull Request info.
pub fn enrich_with_github_prs(commit: &mut Commit, repo: &str) {
  if let Ok(prs) = ghapi::try_fetch_prs_for_commit(repo, &commit.sha) {
    if let Some(first_pr) = prs.first() {
      commit.patch_ref.github_diff_url = first_pr.diff_url.clone();
      commit.patch_ref.github_patch_url = first_pr.patch_url.clone();
    }
    commit.github_prs = Some(prs);
  }
}

/// Aggregate and enrich PRs across a commit set into a top-level array.
/// Best-effort: returns None when origin or token are missing.
pub fn collect_pull_requests_for_commits(commits: &[Commit], repo: &str) -> Option<Vec<GithubPullRequest>> {
  let (owner, name) = match ghapi::parse_origin_github(repo) {
    Some(p) => p,
    None => {
      eprintln!("[github] Skipping PR aggregation: repo origin is not GitHub (origin.remote.url)");
      return None;
    }
  };
  let token = match ghapi::get_github_token() {
    Some(t) => t,
    None => {
      eprintln!("[github] Missing token. Set GITHUB_TOKEN or run: gh auth login");
      return None;
    }
  };

  if commits.is_empty() {
    return Some(Vec::new());
  }

  use std::collections::BTreeSet;
  let mut pr_numbers: BTreeSet<i64> = BTreeSet::new();
  for commit in commits {
    if let Some(prs) = &commit.github_prs {
      for pr in prs {
        if pr.number > 0 {
          pr_numbers.insert(pr.number);
        }
      }
    }
  }

  if pr_numbers.is_empty() {
    return Some(Vec::new());
  }

  let mut out: Vec<GithubPullRequest> = Vec::with_capacity(pr_numbers.len());
  for number in pr_numbers {
    let details_json = ghapi::get_pull_details(&owner, &name, number, &token);
    if let Some(pr_json) = details_json {
      let html_url = pr_json.fetch("html_url").to_or_default::<String>();
      let pr_commits = ghapi::list_commits_in_pull(&owner, &name, number, &token);

      let title = pr_json.fetch("title").to_or_default::<String>();
      let state = pr_json.fetch("state").to_or_default::<String>();
      let created_at = pr_json.fetch("created_at").to::<String>();
      let merged_at = pr_json.fetch("merged_at").to::<String>();
      let closed_at = pr_json.fetch("closed_at").to::<String>();
      let body = pr_json.fetch("body").to::<String>();
      let user_login = pr_json.fetch("user.login").to::<String>();
      let user = user_login.map(|login| crate::model::GithubUser { login: Some(login) });
      let head = pr_json.fetch("head.ref").to::<String>();
      let base = pr_json.fetch("base.ref").to::<String>();

      let diff_url = if html_url.is_empty() { None } else { Some(format!("{}.diff", html_url)) };
      let patch_url = if html_url.is_empty() { None } else { Some(format!("{}.patch", html_url)) };

      let pr = GithubPullRequest {
        number,
        title,
        state,
        body,
        created_at,
        merged_at,
        closed_at,
        html_url,
        diff_url,
        patch_url,
        user,
        head,
        base,
        commits: Some(pr_commits),
      };

      out.push(pr);
    }
  }

  Some(out)
}

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
use crate::enrichment::github_api::GithubApi;
use crate::ext::serde_json::JsonFetch;
use crate::model::{Commit, GithubPullRequest};

/// Enriches a commit with its associated GitHub Pull Request info (best-effort).
/// Default path uses repository origin and token discovery.
pub fn enrich_with_github_prs(commit: &mut Commit, repo: &str) {
  if let Ok(prs) = ghapi::try_fetch_prs_for_commit(repo, &commit.sha) {
    if let Some(first_pr) = prs.first() {
      commit.patch_ref.github_diff_url = first_pr.diff_url.clone();
      commit.patch_ref.github_patch_url = first_pr.patch_url.clone();
    }

    commit.github_prs = Some(prs);
  }
}

/// Enrich a commit using an injected GithubApi backend (no token/env logic here).
#[cfg(any(test, feature = "testutil"))]
pub fn enrich_with_github_prs_with_api(commit: &mut Commit, repo: &str, api: &dyn GithubApi) {
  // Phase 1: resolve origin; early guard when not a GitHub remote
  let (owner, name) = match ghapi::parse_origin_github(repo) {
    Some(p) => p,
    None => return,
  };

  // Phase 2: fetch JSON array via api; early guard on missing/shape
  let parsed = match api.list_pulls_for_commit_json(&owner, &name, &commit.sha) {
    Some(v) => v,
    None => return,
  };

  let arr = match parsed.as_array() {
    Some(a) => a,
    None => return,
  };

  // Phase 3: build typed PRs (extract-before-build) and attach
  let mut out: Vec<GithubPullRequest> = Vec::with_capacity(arr.len());
  for pr_json in arr {
    let html_url = pr_json.fetch("html_url").to_or_default::<String>();
    let user_login = pr_json.fetch("user.login").to::<String>();
    let user = user_login.map(|login| crate::model::GithubUser { login: Some(login) });
    let head = pr_json.fetch("head.ref").to::<String>();
    let base = pr_json.fetch("base.ref").to::<String>();

    let diff_url = if html_url.is_empty() { None } else { Some(format!("{}.diff", html_url)) };
    let patch_url = if html_url.is_empty() { None } else { Some(format!("{}.patch", html_url)) };

    let item = GithubPullRequest {
      number: pr_json.fetch("number").to::<i64>().unwrap_or(0),
      title: pr_json.fetch("title").to_or_default::<String>(),
      state: pr_json.fetch("state").to_or_default::<String>(),
      body: pr_json.fetch("body").to::<String>(),
      created_at: pr_json.fetch("created_at").to::<String>(),
      merged_at: pr_json.fetch("merged_at").to::<String>(),
      closed_at: pr_json.fetch("closed_at").to::<String>(),
      html_url,
      diff_url,
      patch_url,
      user,
      head,
      base,
      commits: None,
    };

    out.push(item);
  }

  if let Some(first) = out.first() {
    commit.patch_ref.github_diff_url = first.diff_url.clone();
    commit.patch_ref.github_patch_url = first.patch_url.clone();
  }

  commit.github_prs = Some(out);
}

/// Aggregate and enrich PRs across a commit set into a top-level array.
/// Best-effort: returns None when origin or token are missing.
pub fn collect_pull_requests_for_commits(commits: &[Commit], repo: &str) -> Option<Vec<GithubPullRequest>> {
  // Phase 1: origin + token; early guards with operator messages
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

  // Phase 2: delegate to injected seam with HTTP backend
  let api = ghapi::make_default_api(Some(token));

  collect_pull_requests_for_commits_with_api(commits, (&owner, &name), api.as_ref())
}

/// Aggregate and enrich PRs using an injected GithubApi (no token/env logic here).
pub fn collect_pull_requests_for_commits_with_api(
  commits: &[Commit],
  owner_name: (&str, &str),
  api: &dyn GithubApi,
) -> Option<Vec<GithubPullRequest>> {
  // Phase 1: early guard
  if commits.is_empty() {
    return Some(Vec::new());
  }

  // Phase 2: collect unique PR numbers from commits
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

  // Phase 3: fetch details and commits; build typed PRs
  let (owner, name) = owner_name;
  let mut out: Vec<GithubPullRequest> = Vec::with_capacity(pr_numbers.len());

  for number in pr_numbers {
    let details_json = api.get_pull_details_json(owner, name, number);
    if let Some(pr_json) = details_json {
      let html_url = pr_json.fetch("html_url").to_or_default::<String>();
      let pr_commits = api.list_commits_in_pull(owner, name, number);

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

  // Finalize
  Some(out)
}

#[cfg(test)]
mod tests {
  use super::*;
  use serial_test::serial;

  fn minimal_commit_with_pr(num: i64) -> Commit {
    let mut c = Commit {
      sha: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".into(),
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
      patch_ref: crate::model::PatchRef { embed: false, git_show_cmd: "".into(), local_patch_file: None, github_diff_url: None, github_patch_url: None },
      patch: None,
      patch_clipped: None,
      github_prs: None,
      body_lines: None,
    };
    c.github_prs = Some(vec![GithubPullRequest {
      number: num,
      title: String::new(),
      state: String::new(),
      body: None,
      created_at: None,
      merged_at: None,
      closed_at: None,
      html_url: String::new(),
      diff_url: None,
      patch_url: None,
      user: None,
      head: None,
      base: None,
      commits: None,
    }]);
    c
  }

  fn init_git_repo_with_origin() -> tempfile::TempDir {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(repo).status();
    let _ = std::process::Command::new("git").args(["remote", "add", "origin", "https://github.com/openai/example.git"]).current_dir(repo).status();
    td
  }

  #[test]
  #[serial]
  fn enriches_commit_with_pr_links() {
    std::env::set_var("GITHUB_TOKEN", "x");
    std::env::set_var("GAR_TEST_PR_JSON", serde_json::json!([{ "html_url": "https://github.com/openai/example/pull/10", "number": 10, "title": "T", "state": "open", "user": {"login": "octo"}, "head": {"ref": "h"}, "base": {"ref": "b"} }]).to_string());
    let td = init_git_repo_with_origin();
    let repo = td.path().to_str().unwrap();
    let mut c = minimal_commit_with_pr(0);
    enrich_with_github_prs(&mut c, repo);
    assert!(c.github_prs.as_ref().unwrap().len() >= 1);
    assert_eq!(c.patch_ref.github_diff_url.as_deref(), Some("https://github.com/openai/example/pull/10.diff"));
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
  fn aggregates_pull_requests_with_details_and_commits() {
    std::env::set_var("GITHUB_TOKEN", "x");
    std::env::set_var("GAR_TEST_PULL_DETAILS_JSON", serde_json::json!({
      "html_url": "https://github.com/openai/example/pull/1",
      "number": 1,
      "title": "Add feature",
      "state": "closed",
      "user": {"login": "octo"},
      "head": {"ref": "feature/x"},
      "base": {"ref": "main"},
      "created_at": "2024-01-01T00:00:00Z",
      "closed_at": "2024-01-02T00:00:00Z",
      "merged_at": null
    }).to_string());
    std::env::set_var("GAR_TEST_PR_COMMITS_JSON", serde_json::json!([{ "sha": "abc1234", "commit": {"message": "Subject\nBody"}}]).to_string());
    let td = init_git_repo_with_origin();
    let repo = td.path().to_str().unwrap();
    let commits = vec![minimal_commit_with_pr(1)];
    let out = collect_pull_requests_for_commits(&commits, repo).unwrap();
    assert_eq!(out.len(), 1);
    let pr = &out[0];
    assert_eq!(pr.number, 1);
    assert_eq!(pr.title, "Add feature");
    assert_eq!(pr.user.as_ref().and_then(|u| u.login.clone()).as_deref(), Some("octo"));
    assert_eq!(pr.commits.as_ref().unwrap()[0].short_sha.len(), 7);
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
  }

  #[test]
  fn seam_collect_with_env_api_without_token() {
    // Prepare env-backed API (no token) and a commit that references PR #1
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({
        "html_url": "https://github.com/openai/example/pull/1",
        "number": 1,
        "title": "Add feature",
        "state": "closed",
        "user": {"login": "octo"},
        "head": {"ref": "feature/x"},
        "base": {"ref": "main"},
        "created_at": "2024-01-01T00:00:00Z",
        "closed_at": "2024-01-02T00:00:00Z",
        "merged_at": null
      }).to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_COMMITS_JSON",
      serde_json::json!([{ "sha": "abc1234", "commit": {"message": "Subject\nBody"}}]).to_string(),
    );

    let commits = vec![minimal_commit_with_pr(1)];
    let api = ghapi::make_env_api();
    let out = collect_pull_requests_for_commits_with_api(&commits, ("openai", "example"), api.as_ref()).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].html_url, "https://github.com/openai/example/pull/1");

    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
  }

  #[test]
  fn seam_enrich_commit_with_env_api() {
    // Origin (for owner/name parsing), env-backed API, and commit enrichment
    let td = init_git_repo_with_origin();
    let repo = td.path().to_str().unwrap();
    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([{ "html_url": "https://github.com/openai/example/pull/10", "number": 10, "title": "T", "state": "open" }]).to_string(),
    );

    let api = ghapi::make_env_api();
    let mut c = minimal_commit_with_pr(0);
    enrich_with_github_prs_with_api(&mut c, repo, api.as_ref());
    assert_eq!(
      c.patch_ref.github_diff_url.as_deref(),
      Some("https://github.com/openai/example/pull/10.diff")
    );

    std::env::remove_var("GAR_TEST_PR_JSON");
  }
}

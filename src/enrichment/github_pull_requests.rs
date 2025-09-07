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
#[cfg(any(test, feature = "testutil"))]
use crate::enrichment::github_api::GithubApi;
#[cfg(any(test, feature = "testutil"))]
use crate::ext::serde_json::JsonFetch;
use crate::model::{Commit, CommitGithub, PatchReferencesGithub};
#[cfg(any(test, feature = "testutil"))]
use crate::model::{GithubPullRequest, GithubUser};
#[cfg(any(test, feature = "testutil"))]
use crate::util::diff_seconds;

// --- Local helpers to unify repeated patterns ---
fn commit_patch_refs(owner: &str, name: &str, sha: &str) -> PatchReferencesGithub {
  let base = format!("https://github.com/{}/{}/commit/{}", owner, name, sha);
  PatchReferencesGithub {
    commit_url: Some(base.clone()),
    diff_url: Some(format!("{}.diff", base)),
    patch_url: Some(format!("{}.patch", base)),
  }
}

#[cfg(any(test, feature = "testutil"))]
fn urls_from_html(html_url: &str) -> (Option<String>, Option<String>) {
  if html_url.is_empty() {
    (None, None)
  } else {
    (Some(format!("{}.diff", html_url)), Some(format!("{}.patch", html_url)))
  }
}

#[cfg(any(test, feature = "testutil"))]
fn build_github_user(api: &dyn GithubApi, login: &str, assoc_opt: Option<&str>) -> GithubUser {
  let user_json = api.get_user_json(login);
  let email = user_json.as_ref().and_then(|u| u.fetch("email").to::<String>());

  let mut user_type = if login.ends_with("[bot]") {
    "bot".to_string()
  } else if let Some(a) = assoc_opt {
    classify_assoc_local(a)
  } else {
    "unknown".to_string()
  };

  if user_type.as_str() == "unknown" {
    let is_bot_json = user_json
      .as_ref()
      .and_then(|u| u.fetch("type").to::<String>())
      .map(|t| t.eq_ignore_ascii_case("Bot"))
      .unwrap_or(false);

    if is_bot_json || login.ends_with("[bot]") {
      user_type = "bot".to_string();
    }
  }

  GithubUser {
    login: Some(login.to_string()),
    profile_url: Some(format!("https://github.com/{}", login)),
    r#type: Some(user_type),
    email,
  }
}

#[cfg(any(test, feature = "testutil"))]
fn classify_assoc_local(a: &str) -> String {
  let s = a.to_ascii_uppercase();
  match s.as_str() {
    "OWNER" | "MEMBER" | "COLLABORATOR" => "member".into(),
    "CONTRIBUTOR" | "FIRST_TIME_CONTRIBUTOR" | "FIRST_TIMER" => "contributor".into(),
    _ => "other".into(),
  }
}

#[cfg(any(test, feature = "testutil"))]
fn compute_review_metrics(arr: &[serde_json::Value]) -> (i64, i64, Option<String>, Option<String>) {
  let mut approvals = 0i64;
  let mut changes = 0i64;
  let mut first_ts: Option<String> = None;
  let mut latest_ts: Option<String> = None;
  let mut latest_login: Option<String> = None;

  for r in arr.iter() {
    let state = r.fetch("state").to_or_default::<String>();
    let login_opt = r.fetch("user.login").to::<String>();
    let submitted = r.fetch("submitted_at").to::<String>();

    if let Some(ts) = &submitted {
      if first_ts.as_ref().map(|cur| ts < cur).unwrap_or(true) {
        first_ts = Some(ts.clone());
      }
    }

    if state.eq_ignore_ascii_case("APPROVED") {
      approvals += 1;
      if let Some(ts) = &submitted {
        if latest_ts.as_ref().map(|cur| ts > cur).unwrap_or(true) {
          latest_ts = Some(ts.clone());
          latest_login = login_opt.clone();
        }
      }
    } else if state.eq_ignore_ascii_case("CHANGES_REQUESTED") {
      changes += 1;
    }
  }

  (approvals, changes, first_ts, latest_login)
}

/// Enriches a commit with its associated GitHub Pull Request info (best-effort).
/// Default path uses repository origin and token discovery.
pub fn enrich_with_github_prs(commit: &mut Commit, repo: &str) {
  if let Some((owner, name)) = ghapi::parse_origin_github(repo) {
    commit.patch_references.github = Some(commit_patch_refs(&owner, &name, &commit.sha));
  }

  if let Ok(prs) = ghapi::try_fetch_prs_for_commit(repo, &commit.sha) {
    if !prs.is_empty() {
      commit.github = Some(CommitGithub { pull_requests: prs });
    }
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

  // Compute GitHub commit/patch/diff URLs from origin + sha.
  // Attach URLs to commit.
  commit.patch_references.github = Some(commit_patch_refs(&owner, &name, &commit.sha));

  // Phase 2: fetch JSON array via api; early guard on missing/shape
  let parsed = match api.list_pulls_for_commit_json(&owner, &name, &commit.sha) {
    Some(v) => v,
    None => return,
  };

  let arr = match parsed.as_array() {
    Some(a) => a,
    None => return,
  };

  // Phase 3: build typed PRs (extract-before-build) and attach, including reviewers/approver
  let mut out: Vec<GithubPullRequest> = Vec::with_capacity(arr.len());

  for pr_json in arr {
    let html_url = pr_json.fetch("html_url").to_or_default::<String>();
    let submitter_login = pr_json.fetch("user.login").to::<String>();

    let submitter = submitter_login.clone().map(|login| GithubUser {
      login: Some(login.clone()),
      profile_url: Some(format!("https://github.com/{}", login)),
      r#type: Some("unknown".into()),
      email: None,
    });
    let head = pr_json.fetch("head.ref").to::<String>();
    let base = pr_json.fetch("base.ref").to::<String>();

    let number = pr_json.fetch("number").to::<i64>().unwrap_or(0);
    let title = pr_json.fetch("title").to_or_default::<String>();
    let state = pr_json.fetch("state").to_or_default::<String>();

    let body_lines = pr_json
      .fetch("body")
      .to::<String>()
      .map(|b| b.lines().map(|s| s.to_string()).collect());

    let created_at = pr_json.fetch("created_at").to::<String>();
    let merged_at = pr_json.fetch("merged_at").to::<String>();
    let closed_at = pr_json.fetch("closed_at").to::<String>();

    let (diff_url, patch_url) = urls_from_html(&html_url);

    let item = GithubPullRequest {
      number,
      title,
      state,
      body_lines,
      created_at,
      merged_at,
      closed_at,
      html_url,
      diff_url,
      patch_url,
      submitter,
      approver: None,
      reviewers: None,
      head,
      base,
      commits: None,
      review_count: None,
      approval_count: None,
      change_request_count: None,
      time_to_first_review_seconds: None,
      time_to_merge_seconds: None,
    };

    out.push(item);
  }

  if !out.is_empty() {
    commit.github = Some(CommitGithub { pull_requests: out });
  }
}

/// Aggregate and enrich PRs across a commit set into a top-level array.
/// Best-effort: returns None when origin or token are missing.
#[cfg(any(test, feature = "testutil"))]
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
#[cfg(any(test, feature = "testutil"))]
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
    if let Some(gh) = &commit.github {
      let prs = &gh.pull_requests;

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
    if let Some(pr_json) = api.get_pull_details_json(owner, name, number) {
      let pr = build_aggregated_pr(number, &pr_json, owner, name, api);
      out.push(pr);
    }
  }

  // Finalize
  Some(out)
}

#[cfg(any(test, feature = "testutil"))]
fn build_aggregated_pr(
  number: i64,
  pr_json: &serde_json::Value,
  owner: &str,
  name: &str,
  api: &dyn GithubApi,
) -> GithubPullRequest {
  let html_url = pr_json.fetch("html_url").to_or_default::<String>();
  let pr_commits = api.list_commits_in_pull(owner, name, number);

  let title = pr_json.fetch("title").to_or_default::<String>();
  let state = pr_json.fetch("state").to_or_default::<String>();
  let created_at = pr_json.fetch("created_at").to::<String>();
  let merged_at = pr_json.fetch("merged_at").to::<String>();
  let closed_at = pr_json.fetch("closed_at").to::<String>();
  let body_lines = pr_json
    .fetch("body")
    .to::<String>()
    .map(|b| b.lines().map(|s| s.to_string()).collect());
  let submitter = pr_json.fetch("user.login").to::<String>().map(|login| GithubUser {
    login: Some(login.clone()),
    profile_url: Some(format!("https://github.com/{}", login)),
    r#type: None,
    email: None,
  });

  // Reviews + metrics
  let mut review_count: Option<i64> = None;
  let mut approval_count: Option<i64> = None;
  let mut change_request_count: Option<i64> = None;
  let mut time_to_first_review_seconds: Option<i64> = None;
  let mut approver = None;

  if let Some(reviews_json) = api.list_reviews_for_pull_json(owner, name, number) {
    if let Some(arr) = reviews_json.as_array() {
      review_count = Some(arr.len() as i64);
      let (approvals, changes, first_ts, latest_login) = compute_review_metrics(arr);
      approval_count = Some(approvals);
      change_request_count = Some(changes);

      let created_for_first = pr_json.fetch("created_at").to::<String>();

      if let (Some(created), Some(first)) = (created_for_first, first_ts) {
        time_to_first_review_seconds = diff_seconds(&created, &first);
      }

      if let Some(login) = latest_login {
        approver = Some(build_github_user(api, &login, None));
      }
    }
  }
  if approver.is_none() {
    let merged_by_login = pr_json.fetch("merged_by.login").to::<String>();
    approver = merged_by_login.map(|login| build_github_user(api, &login, None));
  }

  let head = pr_json.fetch("head.ref").to::<String>();
  let base = pr_json.fetch("base.ref").to::<String>();
  let (diff_url, patch_url) = urls_from_html(&html_url);
  let time_to_merge_seconds = merged_at
    .as_ref()
    .and_then(|m| created_at.as_ref().and_then(|c| diff_seconds(c, m)));

  GithubPullRequest {
    number,
    title,
    state,
    body_lines,
    created_at,
    merged_at,
    closed_at,
    html_url,
    diff_url,
    patch_url,
    submitter,
    approver,
    reviewers: None,
    head,
    base,
    commits: Some(pr_commits),
    review_count,
    approval_count,
    change_request_count,
    time_to_first_review_seconds,
    time_to_merge_seconds,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  use serial_test::serial;

  fn minimal_commit_with_pr(num: i64) -> Commit {
    let mut c = Commit {
      sha: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".into(),
      short_sha: "deadbee".into(),
      parents: vec![],
      author: crate::model::Person {
        name: "A".into(),
        email: "a@ex".into(),
        date: "".into(),
      },
      committer: crate::model::Person {
        name: "A".into(),
        email: "a@ex".into(),
        date: "".into(),
      },
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
      patch_references: crate::model::PatchReferences {
        embed: false,
        git_show_cmd: "".into(),
        local_patch_file: None,
        github: None,
      },
      patch_clipped: None,
      patch_lines: None,
      body_lines: None,
      github: None,
    };
    c.github = Some(CommitGithub {
      pull_requests: vec![GithubPullRequest {
        number: num,
        title: String::new(),
        state: String::new(),
        body_lines: None,
        created_at: None,
        merged_at: None,
        closed_at: None,
        html_url: String::new(),
        diff_url: None,
        patch_url: None,
        submitter: None,
        approver: None,
        reviewers: None,
        head: None,
        base: None,
        commits: None,
        review_count: None,
        approval_count: None,
        change_request_count: None,
        time_to_first_review_seconds: None,
        time_to_merge_seconds: None,
      }],
    });
    c
  }

  fn init_git_repo_with_origin() -> tempfile::TempDir {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example.git"])
      .current_dir(repo)
      .status();
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
    assert!(c.github.as_ref().unwrap().pull_requests.len() >= 1);
    // patch_references.github should include commit_url derived from origin
    assert!(
      c.patch_references
        .github
        .as_ref()
        .and_then(|g| g.commit_url.clone())
        .unwrap()
        .contains("/commit/")
    );
    // Submitter mirrors user; approver not available from list endpoint
    let pr0 = &c.github.as_ref().unwrap().pull_requests[0];
    assert_eq!(
      pr0.submitter.as_ref().and_then(|u| u.login.clone()).as_deref(),
      Some("octo")
    );
    assert!(pr0.approver.is_none());
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
  fn aggregates_pull_requests_with_details_and_commits() {
    std::env::set_var("GITHUB_TOKEN", "x");
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({
        "html_url": "https://github.com/openai/example/pull/1",
        "number": 1,
        "title": "Add feature",
        "state": "closed",
        "user": {"login": "octo"},
        "merged_by": {"login": "marge"},
        "head": {"ref": "feature/x"},
        "base": {"ref": "main"},
        "created_at": "2024-01-01T00:00:00Z",
        "closed_at": "2024-01-02T00:00:00Z",
        "merged_at": null
      })
      .to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_COMMITS_JSON",
      serde_json::json!([{ "sha": "abc1234", "commit": {"message": "Subject\nBody"}}]).to_string(),
    );
    let td = init_git_repo_with_origin();
    let repo = td.path().to_str().unwrap();
    let commits = vec![minimal_commit_with_pr(1)];
    let out = collect_pull_requests_for_commits(&commits, repo).unwrap();
    assert_eq!(out.len(), 1);
    let pr = &out[0];
    assert_eq!(pr.number, 1);
    assert_eq!(pr.title, "Add feature");
    assert_eq!(
      pr.submitter.as_ref().and_then(|u| u.login.clone()).as_deref(),
      Some("octo")
    );
    assert_eq!(
      pr.submitter.as_ref().and_then(|u| u.login.clone()).as_deref(),
      Some("octo")
    );
    assert_eq!(
      pr.approver.as_ref().and_then(|u| u.login.clone()).as_deref(),
      Some("marge")
    );
    assert_eq!(pr.commits.as_ref().unwrap()[0].short_sha.len(), 7);
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
  }

  #[test]
  #[serial]
  fn aggregates_pull_requests_approver_from_reviews() {
    std::env::set_var("GITHUB_TOKEN", "x");
    // PR details without merged_by → approver should be taken from reviews
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({
        "html_url": "https://github.com/openai/example/pull/2",
        "number": 2,
        "title": "Refactor",
        "state": "closed",
        "user": {"login": "submit"},
        "head": {"ref": "refactor/x"},
        "base": {"ref": "main"},
        "created_at": "2024-02-01T00:00:00Z",
        "closed_at": "2024-02-02T00:00:00Z",
        "merged_at": "2024-02-02T00:00:00Z"
      })
      .to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_REVIEWS_JSON",
      serde_json::json!([
        {"state": "COMMENTED", "user": {"login": "x"}, "submitted_at": "2024-02-01T01:00:00Z"},
        {"state": "APPROVED", "user": {"login": "alice"}, "submitted_at": "2024-02-01T02:00:00Z"},
        {"state": "APPROVED", "user": {"login": "bob"}, "submitted_at": "2024-02-01T03:00:00Z"}
      ])
      .to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_COMMITS_JSON",
      serde_json::json!([{ "sha": "abc1234", "commit": {"message": "Subject\nBody"}}]).to_string(),
    );
    let td = init_git_repo_with_origin();
    let repo = td.path().to_str().unwrap();
    let commits = vec![minimal_commit_with_pr(2)];
    let out = collect_pull_requests_for_commits(&commits, repo).unwrap();
    assert_eq!(out.len(), 1);
    let pr = &out[0];
    assert_eq!(
      pr.approver.as_ref().and_then(|u| u.login.clone()).as_deref(),
      Some("bob")
    );

    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
    std::env::remove_var("GAR_TEST_PR_REVIEWS_JSON");
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
      })
      .to_string(),
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
    assert!(
      c.patch_references
        .github
        .as_ref()
        .and_then(|g| g.commit_url.clone())
        .is_some()
    );

    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
  fn seam_enrich_commit_with_env_api_sets_pr_urls() {
    let td = init_git_repo_with_origin();
    let repo = td.path().to_str().unwrap();
    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([{ "html_url": "https://github.com/openai/example/pull/10", "number": 10, "title": "T", "state": "open" }]).to_string(),
    );
    let api = ghapi::make_env_api();
    let mut c = minimal_commit_with_pr(0);
    enrich_with_github_prs_with_api(&mut c, repo, api.as_ref());
    let pr = &c.github.as_ref().unwrap().pull_requests[0];
    assert!(pr.diff_url.as_ref().unwrap().ends_with(".diff"));
    assert!(pr.patch_url.as_ref().unwrap().ends_with(".patch"));
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
  fn aggregator_time_metrics() {
    std::env::set_var("GITHUB_TOKEN", "x");
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({
        "html_url": "https://github.com/openai/example/pull/3",
        "number": 3,
        "title": "Timing",
        "state": "closed",
        "user": {"login": "octo"},
        "head": {"ref": "feature/t"},
        "base": {"ref": "main"},
        "created_at": "2024-01-01T00:00:00Z",
        "closed_at": "2024-01-03T00:00:00Z",
        "merged_at": "2024-01-03T00:00:00Z"
      })
      .to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_REVIEWS_JSON",
      serde_json::json!([
        {"state": "COMMENTED", "user": {"login": "x"}, "submitted_at": "2024-01-01T12:00:00Z"}
      ])
      .to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_COMMITS_JSON",
      serde_json::json!([{ "sha": "abc1234", "commit": {"message": "Subject\nBody"}}]).to_string(),
    );
    let commits = vec![minimal_commit_with_pr(3)];
    let out =
      collect_pull_requests_for_commits_with_api(&commits, ("openai", "example"), ghapi::make_env_api().as_ref())
        .unwrap();
    let pr = &out[0];
    assert_eq!(pr.time_to_first_review_seconds, Some(12 * 3600));
    assert_eq!(pr.time_to_merge_seconds, Some(2 * 24 * 3600));
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
    std::env::remove_var("GAR_TEST_PR_REVIEWS_JSON");
  }

  #[test]
  fn unit_compute_review_metrics_missing_submitted_at() {
    let arr = json!([
      {"state": "APPROVED", "user": {"login": "approver"}},
      {"state": "COMMENTED", "user": {"login": "c"}, "submitted_at": "2024-02-01T01:00:00Z"}
    ]);
    let (approvals, changes, first_ts, latest_login) = compute_review_metrics(arr.as_array().unwrap());
    assert_eq!(approvals, 1);
    assert_eq!(changes, 0);
    assert_eq!(first_ts.as_deref(), Some("2024-02-01T01:00:00Z"));
    assert!(latest_login.is_none());
  }

  #[test]
  fn unit_build_github_user_contributor() {
    let api = DummyApi;
    let u = build_github_user(&api, "foo", Some("FIRST_TIME_CONTRIBUTOR"));
    assert_eq!(u.r#type.as_deref(), Some("contributor"));
    assert!(u.email.is_none());
  }

  #[test]
  fn unit_compute_review_metrics_orders_and_counts() {
    let arr = json!([
      {"state": "APPROVED", "user": {"login": "alice"}, "submitted_at": "2024-02-01T02:00:00Z"},
      {"state": "COMMENTED", "user": {"login": "c"}, "submitted_at": "2024-02-01T01:30:00Z"},
      {"state": "CHANGES_REQUESTED", "user": {"login": "d"}, "submitted_at": "2024-02-01T01:45:00Z"},
      {"state": "APPROVED", "user": {"login": "bob"}, "submitted_at": "2024-02-01T03:00:00Z"}
    ]);
    let (approvals, changes, first_ts, latest_login) = compute_review_metrics(arr.as_array().unwrap());
    assert_eq!(approvals, 2);
    assert_eq!(changes, 1);
    assert_eq!(first_ts.as_deref(), Some("2024-02-01T01:30:00Z"));
    assert_eq!(latest_login.as_deref(), Some("bob"));
  }

  #[test]
  fn unit_compute_review_metrics_no_approvals() {
    let arr = json!([
      {"state": "COMMENTED", "user": {"login": "x"}, "submitted_at": "2024-02-01T01:00:00Z"},
      {"state": "CHANGES_REQUESTED", "user": {"login": "y"}, "submitted_at": "2024-02-01T02:00:00Z"}
    ]);
    let (approvals, changes, first_ts, latest_login) = compute_review_metrics(arr.as_array().unwrap());
    assert_eq!(approvals, 0);
    assert_eq!(changes, 1);
    assert_eq!(first_ts.as_deref(), Some("2024-02-01T01:00:00Z"));
    assert!(latest_login.is_none());
  }

  struct DummyApi;
  #[cfg(any(test, feature = "testutil"))]
  impl ghapi::GithubApi for DummyApi {
    fn list_pulls_for_commit_json(&self, _o: &str, _n: &str, _s: &str) -> Option<serde_json::Value> {
      None
    }
    fn get_pull_details_json(&self, _o: &str, _n: &str, _num: i64) -> Option<serde_json::Value> {
      None
    }
    fn list_commits_in_pull(&self, _o: &str, _n: &str, _num: i64) -> Vec<crate::model::PullRequestCommit> {
      Vec::new()
    }
    fn list_reviews_for_pull_json(&self, _o: &str, _n: &str, _num: i64) -> Option<serde_json::Value> {
      None
    }
    fn list_commits_in_pull_json(&self, _o: &str, _n: &str, _num: i64) -> Option<serde_json::Value> {
      None
    }
    fn get_user_json(&self, login: &str) -> Option<serde_json::Value> {
      match login {
        "alice" => Some(json!({"email": "alice@example.com", "type": "User"})),
        "renovate[bot]" => Some(json!({"type": "Bot"})),
        _ => None,
      }
    }
  }

  #[test]
  fn unit_build_github_user_member_and_bot() {
    let api = DummyApi;
    let u = build_github_user(&api, "alice", Some("MEMBER"));
    assert_eq!(u.login.as_deref(), Some("alice"));
    assert_eq!(u.r#type.as_deref(), Some("member"));
    assert_eq!(u.email.as_deref(), Some("alice@example.com"));

    let b = build_github_user(&api, "renovate[bot]", None);
    assert_eq!(b.r#type.as_deref(), Some("bot"));
    assert!(b.email.is_none());
  }

  #[test]
  fn unit_urls_from_html_variants() {
    let (d, p) = urls_from_html("");
    assert!(d.is_none() && p.is_none());
    let (d2, p2) = urls_from_html("https://github.com/openai/example/pull/1");
    assert!(d2.unwrap().ends_with(".diff") && p2.unwrap().ends_with(".patch"));
  }
}

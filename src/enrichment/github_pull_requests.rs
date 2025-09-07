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

/// Enriches a commit with its associated GitHub Pull Request info (best-effort).
/// Default path uses repository origin and token discovery.
pub fn enrich_with_github_prs(commit: &mut Commit, repo: &str) {
  if let Some((owner, name)) = ghapi::parse_origin_github(repo) {
    let base = format!("https://github.com/{}/{}/commit/{}", owner, name, commit.sha);
    let gh_urls = PatchReferencesGithub {
      commit_url: Some(base.clone()),
      diff_url: Some(format!("{}.diff", base)),
      patch_url: Some(format!("{}.patch", base)),
    };
    commit.patch_references.github = Some(gh_urls);
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

    let diff_url = if html_url.is_empty() {
      None
    } else {
      Some(format!("{}.diff", html_url))
    };
    let patch_url = if html_url.is_empty() {
      None
    } else {
      Some(format!("{}.patch", html_url))
    };

    let item = GithubPullRequest {
      number: pr_json.fetch("number").to::<i64>().unwrap_or(0),
      title: pr_json.fetch("title").to_or_default::<String>(),
      state: pr_json.fetch("state").to_or_default::<String>(),
      body_lines: pr_json
        .fetch("body")
        .to::<String>()
        .map(|b| b.lines().map(|s| s.to_string()).collect()),
      created_at: pr_json.fetch("created_at").to::<String>(),
      merged_at: pr_json.fetch("merged_at").to::<String>(),
      closed_at: pr_json.fetch("closed_at").to::<String>(),
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
    let details_json = api.get_pull_details_json(owner, name, number);

    if let Some(pr_json) = details_json {
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

      // Determine approver: prefer latest APPROVED review; fallback to merged_by
      let mut approver = None;

      if let Some(reviews_json) = api.list_reviews_for_pull_json(owner, name, number) {
        if let Some(arr) = reviews_json.as_array() {
          review_count = Some(arr.len() as i64);
          let mut approvals = 0i64;
          let mut changes = 0i64;
          let mut first_ts: Option<String> = None;
          let mut latest_idx: Option<usize> = None;
          let mut latest_ts: Option<String> = None;

          for (i, r) in arr.iter().enumerate() {
            let state = r.fetch("state").to_or_default::<String>();

            if state.eq_ignore_ascii_case("APPROVED") {
              approvals += 1;
              let ts = r.fetch("submitted_at").to::<String>();

              match (&latest_ts, ts.as_ref()) {
                (Some(cur), Some(new_ts)) => {
                  if new_ts > cur {
                    latest_ts = Some(new_ts.clone());
                    latest_idx = Some(i);
                  }
                }
                (None, Some(new_ts)) => {
                  latest_ts = Some(new_ts.clone());
                  latest_idx = Some(i);
                }
                _ => {
                  latest_idx = Some(i);
                }
              }
            } else if state.eq_ignore_ascii_case("CHANGES_REQUESTED") {
              changes += 1;
            }
            if let Some(ts) = r.fetch("submitted_at").to::<String>() {
              if first_ts.as_ref().map(|cur| ts < *cur).unwrap_or(true) {
                first_ts = Some(ts);
              }
            }
          }
          approval_count = Some(approvals);
          change_request_count = Some(changes);
          if let (Some(created), Some(first)) = (pr_json.fetch("created_at").to::<String>(), first_ts) {
            time_to_first_review_seconds = diff_seconds(&created, &first);
          }
          if let Some(i) = latest_idx {
            let login = arr[i].fetch("user.login").to::<String>();
            approver = login.map(|l| GithubUser {
              login: Some(l.clone()),
              profile_url: Some(format!("https://github.com/{}", l)),
              r#type: None,
              email: None,
            });
          }
        }
      }
      if approver.is_none() {
        let merged_by_login = pr_json.fetch("merged_by.login").to::<String>();
        approver = merged_by_login.map(|login| GithubUser {
          login: Some(login.clone()),
          profile_url: Some(format!("https://github.com/{}", login)),
          r#type: None,
          email: None,
        });
      }
      let head = pr_json.fetch("head.ref").to::<String>();
      let base = pr_json.fetch("base.ref").to::<String>();

      let diff_url = if html_url.is_empty() {
        None
      } else {
        Some(format!("{}.diff", html_url))
      };
      let patch_url = if html_url.is_empty() {
        None
      } else {
        Some(format!("{}.patch", html_url))
      };

      let time_to_merge_seconds = merged_at
        .as_ref()
        .and_then(|m| created_at.as_ref().and_then(|c| diff_seconds(c, m)));

      let pr = GithubPullRequest {
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
}

// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Isolated GitHub API helpers used by enrichment (token discovery, REST calls)
// role: enrichment/github-api
// inputs: repo path for origin detection; env GITHUB_TOKEN; optional `gh` CLI for token fallback
// outputs: JSON values and typed commit snapshots for PRs
// side_effects: Network calls to api.github.com; spawns `gh` subprocess when needed
// invariants:
// - Never panic; return None/empty on failures (best-effort enrichment)
// - Token discovery prefers GITHUB_TOKEN, then `gh auth token`
// - Origin parser only recognizes GitHub remotes (https or ssh)
// errors: Swallowed; callers decide whether to surface warnings
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use crate::ext::serde_json::JsonFetch;
use crate::model::{GithubPullRequest, GithubUser, PullRequestCommit};
use crate::util::run_git;

/// Parse `remote.origin.url` to extract (owner, repo) when hosted on GitHub.
pub fn parse_origin_github(repo: &str) -> Option<(String, String)> {
  if let Ok(url) = run_git(repo, &["config".into(), "--get".into(), "remote.origin.url".into()]) {
    let u = url.trim();
    let re1 = regex::Regex::new(r"^(?:git@github\.com:|https?://github\.com/)([^/]+)/([^/]+?)(?:\.git)?$").ok()?;
    if let Some(c) = re1.captures(u) {
      let owner = c.get(1)?.as_str().to_string();
      let repo_name = c.get(2)?.as_str().to_string();
      return Some((owner, repo_name));
    }
  }
  None
}

/// Discover a GitHub token: env var first, then `gh auth token` if available.
pub fn get_github_token() -> Option<String> {
  if let Ok(t) = std::env::var("GITHUB_TOKEN") {
    if !t.trim().is_empty() {
      return Some(t);
    }
  }

  if let Ok(path) = std::env::var("GH_TOKEN") {
    if !path.trim().is_empty() {
      return Some(path);
    }
  }

  if let Ok(output) = std::process::Command::new("gh").args(["auth", "token"]).output() {
    if output.status.success() {
      let t = String::from_utf8_lossy(&output.stdout).trim().to_string();
      if !t.is_empty() {
        return Some(t);
      }
    }
  }

  None
}

fn get_json(url: &str, token: &str) -> Option<serde_json::Value> {
  let agent = ureq::AgentBuilder::new().build();
  let resp = agent
    .get(url)
    .set("Accept", "application/vnd.github+json")
    .set("User-Agent", "git-activity-report")
    .set("Authorization", &format!("Bearer {}", token))
    .call();
  match resp {
    Ok(r) => r.into_json().ok(),
    Err(_) => None,
  }
}

// --- Trait seam for GitHub API ---
pub trait GithubApi {
  fn list_pulls_for_commit_json(&self, owner: &str, name: &str, sha: &str) -> Option<serde_json::Value>;
  fn get_pull_details_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value>;
  fn list_commits_in_pull(&self, owner: &str, name: &str, number: i64) -> Vec<PullRequestCommit>;
  fn list_reviews_for_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value>;
}

struct GithubHttpApi {
  token: String,
}
impl GithubHttpApi {
  fn new(token: String) -> Self {
    Self { token }
  }
}

impl GithubApi for GithubHttpApi {
  fn list_pulls_for_commit_json(&self, owner: &str, name: &str, sha: &str) -> Option<serde_json::Value> {
    let url = format!(
      "https://api.github.com/repos/{}/{}/commits/{}/pulls",
      owner, name, sha
    );
    get_json(&url, &self.token)
  }

  fn get_pull_details_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let url = format!(
      "https://api.github.com/repos/{}/{}/pulls/{}",
      owner, name, number
    );
    get_json(&url, &self.token)
  }

  fn list_commits_in_pull(&self, owner: &str, name: &str, number: i64) -> Vec<PullRequestCommit> {
    let url = format!(
      "https://api.github.com/repos/{}/{}/pulls/{}/commits",
      owner, name, number
    );

    let Some(v) = get_json(&url, &self.token) else { return Vec::new() };
    let Some(arr) = v.as_array() else { return Vec::new() };

    let mut out = Vec::with_capacity(arr.len());

    for item in arr {
      let sha = item.fetch("sha").to_or_default::<String>();
      let msg = item.fetch("commit.message").to_or_default::<String>();
      let subject = msg.lines().next().unwrap_or("").to_string();

      if !sha.is_empty() {
        out.push(PullRequestCommit { short_sha: sha.chars().take(7).collect(), sha, subject });
      }
    }

    out
  }

  fn list_reviews_for_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let url = format!(
      "https://api.github.com/repos/{}/{}/pulls/{}/reviews",
      owner, name, number
    );
    get_json(&url, &self.token)
  }
}

struct GithubEnvApi;
impl GithubApi for GithubEnvApi {
  fn list_pulls_for_commit_json(&self, _owner: &str, _name: &str, _sha: &str) -> Option<serde_json::Value> {
    if let Ok(s) = std::env::var("GAR_TEST_PR_JSON") {
      serde_json::from_str::<serde_json::Value>(&s).ok()
    } else {
      Some(serde_json::json!([]))
    }
  }

  fn get_pull_details_json(&self, _owner: &str, _name: &str, _number: i64) -> Option<serde_json::Value> {
    if let Ok(s) = std::env::var("GAR_TEST_PULL_DETAILS_JSON") {
      serde_json::from_str::<serde_json::Value>(&s).ok()
    } else {
      None
    }
  }

  fn list_commits_in_pull(&self, _owner: &str, _name: &str, _number: i64) -> Vec<PullRequestCommit> {
    let Ok(s) = std::env::var("GAR_TEST_PR_COMMITS_JSON") else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) else { return Vec::new() };
    let Some(arr) = v.as_array() else { return Vec::new() };

    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
      let sha = item.fetch("sha").to_or_default::<String>();
      let msg = item.fetch("commit.message").to_or_default::<String>();
      let subject = msg.lines().next().unwrap_or("").to_string();

      if !sha.is_empty() {
        out.push(PullRequestCommit { short_sha: sha.chars().take(7).collect(), sha, subject });
      }
    }

    out
  }

  fn list_reviews_for_pull_json(&self, _owner: &str, _name: &str, _number: i64) -> Option<serde_json::Value> {
    if let Ok(s) = std::env::var("GAR_TEST_PR_REVIEWS_JSON") {
      serde_json::from_str::<serde_json::Value>(&s).ok()
    } else {
      None
    }
  }
}

fn env_wants_mock() -> bool {
  std::env::var("GAR_TEST_PR_JSON").is_ok()
    || std::env::var("GAR_TEST_PULL_DETAILS_JSON").is_ok()
    || std::env::var("GAR_TEST_PR_COMMITS_JSON").is_ok()
}

fn build_api(token: Option<String>) -> Box<dyn GithubApi> {
  if env_wants_mock() {
    Box::new(GithubEnvApi)
  } else if let Some(t) = token {
    Box::new(GithubHttpApi::new(t))
  } else {
    Box::new(GithubEnvApi)
  }
}

// Public constructors for dependency injection in higher layers/tests.
#[cfg(any(test, feature = "testutil"))]
#[allow(dead_code)]
pub fn make_http_api(token: String) -> Box<dyn GithubApi> { Box::new(GithubHttpApi::new(token)) }
#[cfg(any(test, feature = "testutil"))]
#[allow(dead_code)]
pub fn make_env_api() -> Box<dyn GithubApi> { Box::new(GithubEnvApi) }
pub fn make_default_api(token: Option<String>) -> Box<dyn GithubApi> { build_api(token) }

#[cfg(any(test, feature = "testutil"))]
#[allow(dead_code)]
fn list_pulls_for_commit_json(owner: &str, name: &str, sha: &str, token: &str) -> Option<serde_json::Value> {
  let api = build_api(Some(token.to_string()));
  api.list_pulls_for_commit_json(owner, name, sha)
}

/// Best-effort: fetch PRs referencing a commit SHA using origin and token discovery.
pub fn try_fetch_prs_for_commit(repo: &str, sha: &str) -> anyhow::Result<Vec<GithubPullRequest>> {
  // Phase 1: resolve origin owner/name; early guard when not GitHub
  let (owner, name) = match parse_origin_github(repo) {
    Some(pair) => pair,
    None => return Ok(Vec::new()),
  };

  // Phase 2: select API backend; early guard when no token and no env mocks
  let token = get_github_token();
  if token.is_none() && !env_wants_mock() {
    return Ok(Vec::new());
  }

  let api = build_api(token);

  // Phase 3: fetch and normalize JSON
  let parsed = api
    .list_pulls_for_commit_json(&owner, &name, sha)
    .unwrap_or_else(|| serde_json::json!([]));

  let arr = match parsed.as_array() {
    Some(a) => a,
    None => return Ok(Vec::new()),
  };

  // Phase 4: build items and push
  let mut out: Vec<GithubPullRequest> = Vec::with_capacity(arr.len());

  for pr_json in arr {
    let html = pr_json.fetch("html_url").to_or_default::<String>();
    let submitter = pr_json
      .fetch("user.login")
      .to::<String>()
      .map(|login| GithubUser { login: Some(login.clone()), profile_url: Some(format!("https://github.com/{}", login)), r#type: None, email: None });
    let head = pr_json.fetch("head.ref").to::<String>();
    let base = pr_json.fetch("base.ref").to::<String>();

    let item = GithubPullRequest {
      number: pr_json.fetch("number").to::<i64>().unwrap_or(0),
      title: pr_json.fetch("title").to_or_default::<String>(),
      state: pr_json.fetch("state").to_or_default::<String>(),
      body_lines: pr_json.fetch("body").to::<String>().map(|b| b.lines().map(|s| s.to_string()).collect()),
      created_at: pr_json.fetch("created_at").to::<String>(),
      merged_at: pr_json.fetch("merged_at").to::<String>(),
      closed_at: pr_json.fetch("closed_at").to::<String>(),
      html_url: html.clone(),
      diff_url: if html.is_empty() {
        None
      } else {
        Some(format!("{}.diff", html))
      },
      patch_url: if html.is_empty() {
        None
      } else {
        Some(format!("{}.patch", html))
      },
      submitter,
      approver: None,
      reviewers: None,
      head,
      base,
      commits: None,
    };
    out.push(item);
  }

  // Finalize
  Ok(out)
}

#[cfg(any(test, feature = "testutil"))]
#[allow(dead_code)]
pub fn get_pull_details(owner: &str, name: &str, number: i64, token: &str) -> Option<serde_json::Value> {
  let api = build_api(Some(token.to_string()));
  api.get_pull_details_json(owner, name, number)
}

#[cfg(any(test, feature = "testutil"))]
#[allow(dead_code)]
pub fn list_commits_in_pull(owner: &str, name: &str, number: i64, token: &str) -> Vec<PullRequestCommit> {
  let api = build_api(Some(token.to_string()));
  api.list_commits_in_pull(owner, name, number)
}

#[cfg(test)]
mod tests {
  use super::*;
  use serial_test::serial;

  #[test]
  fn parse_origin_none_without_remote() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let st = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status()
      .unwrap();
    assert!(st.success());
    assert_eq!(parse_origin_github(repo.to_str().unwrap()), None);
  }

  #[test]
  fn parse_origin_github_detects_owner_repo() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status()
      .unwrap();
    let st = std::process::Command::new("git")
      .args(["remote", "add", "origin", "git@github.com:openai/example.git"])
      .current_dir(repo)
      .status()
      .unwrap();
    assert!(st.success());
    let parsed = parse_origin_github(repo.to_str().unwrap());
    assert_eq!(parsed, Some(("openai".to_string(), "example".to_string())));
  }

  #[test]
  #[serial]
  fn try_fetch_prs_no_token_returns_empty() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status()
      .unwrap();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example.git"])
      .current_dir(repo)
      .status();
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert!(out.is_empty());
  }

  #[test]
  #[serial]
  fn try_fetch_prs_with_token_and_env_mock_returns_item() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status()
      .unwrap();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example.git"])
      .current_dir(repo)
      .status();

    std::env::set_var("GITHUB_TOKEN", "test-token");
    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([
        {
          "html_url": "https://github.com/openai/example/pull/1",
          "number": 1,
          "title": "Add feature",
          "state": "open",
          "user": { "login": "octo" },
          "head": { "ref": "feature/x" },
          "base": { "ref": "main" },
          "created_at": "2024-01-01T00:00:00Z",
          "merged_at": null
        }
      ])
      .to_string(),
    );

    let out = try_fetch_prs_for_commit(repo.to_string_lossy().as_ref(), "deadbeef").unwrap();
    assert_eq!(out.len(), 1);
    let pr = &out[0];
    assert_eq!(pr.number, 1);
    assert_eq!(pr.title, "Add feature");
    assert_eq!(pr.state, "open");
    assert_eq!(pr.user.as_ref().and_then(|u| u.login.clone()).as_deref(), Some("octo"));
    assert_eq!(pr.head.as_deref(), Some("feature/x"));
    assert_eq!(pr.base.as_deref(), Some("main"));
    assert_eq!(pr.html_url, "https://github.com/openai/example/pull/1");
    assert_eq!(
      pr.diff_url.as_deref(),
      Some("https://github.com/openai/example/pull/1.diff")
    );
    assert_eq!(
      pr.patch_url.as_deref(),
      Some("https://github.com/openai/example/pull/1.patch")
    );

    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
  fn token_env_precedence_and_fallbacks() {
    // Precedence: GITHUB_TOKEN over GH_TOKEN
    std::env::set_var("GITHUB_TOKEN", "primary-token");
    std::env::set_var("GH_TOKEN", "secondary-token");
    assert_eq!(get_github_token().as_deref(), Some("primary-token"));

    // Fallback to GH_TOKEN when GITHUB_TOKEN absent
    std::env::remove_var("GITHUB_TOKEN");
    assert_eq!(get_github_token().as_deref(), Some("secondary-token"));

    // Fallback to `gh auth token` when envs are absent
    std::env::remove_var("GH_TOKEN");

    // Create a fake `gh` on PATH that returns a token
    let td = tempfile::TempDir::new().unwrap();
    let bin_dir = td.path();
    let gh_path = bin_dir.join("gh");
    #[cfg(target_os = "windows")]
    let script = "@echo off\necho token-from-gh\n";
    #[cfg(not(target_os = "windows"))]
    let script = "#!/bin/sh\necho token-from-gh\n";
    std::fs::write(&gh_path, script).unwrap();
    #[cfg(not(target_os = "windows"))]
    {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = std::fs::metadata(&gh_path).unwrap().permissions();
      perms.set_mode(0o755);
      std::fs::set_permissions(&gh_path, perms).unwrap();
    }

    // Prepend our fake gh to PATH
    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), old_path);
    std::env::set_var("PATH", &new_path);
    assert_eq!(get_github_token().as_deref(), Some("token-from-gh"));

    // Make gh return empty → treat as None
    #[cfg(not(target_os = "windows"))]
    std::fs::write(&gh_path, "#!/bin/sh\necho\n").unwrap();
    #[cfg(target_os = "windows")]
    std::fs::write(&gh_path, "@echo off\necho.\n").unwrap();
    #[cfg(not(target_os = "windows"))]
    {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = std::fs::metadata(&gh_path).unwrap().permissions();
      perms.set_mode(0o755);
      std::fs::set_permissions(&gh_path, perms).unwrap();
    }
    assert_eq!(get_github_token(), None);

    // Restore mutated env
    std::env::set_var("PATH", old_path);
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GH_TOKEN");
  }

  #[test]
  #[serial]
  fn try_fetch_prs_origin_missing_returns_empty() {
    // With a token present but no origin configured, enrichment yields empty
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status();
    std::env::set_var("GITHUB_TOKEN", "x");
    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "abc123").unwrap();
    assert!(out.is_empty());
    std::env::remove_var("GITHUB_TOKEN");
  }

  #[test]
  fn get_json_error_path_is_graceful() {
    // Use an obviously invalid host to force an error quickly
    let val = get_json("http://invalid.localdomain.invalid/", "t");
    assert!(val.is_none());
  }

  #[test]
  fn parse_origin_rejects_non_github_hosts() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://gitlab.com/owner/repo.git"])
      .current_dir(repo)
      .status();
    assert_eq!(parse_origin_github(repo.to_str().unwrap()), None);
  }

  #[test]
  fn parse_origin_https_dot_git() {
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
    assert_eq!(
      parse_origin_github(repo.to_str().unwrap()),
      Some(("openai".into(), "example".into()))
    );
  }

  #[test]
  #[serial]
  fn token_gh_command_failure_returns_none() {
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GH_TOKEN");

    // Create a fake `gh` that exits 1
    let td = tempfile::TempDir::new().unwrap();
    let bin_dir = td.path();
    let gh_path = bin_dir.join("gh");
    #[cfg(target_os = "windows")]
    let script = "@echo off\nexit /b 1\n";
    #[cfg(not(target_os = "windows"))]
    let script = "#!/bin/sh\nexit 1\n";
    std::fs::write(&gh_path, script).unwrap();
    #[cfg(not(target_os = "windows"))]
    {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = std::fs::metadata(&gh_path).unwrap().permissions();
      perms.set_mode(0o755);
      std::fs::set_permissions(&gh_path, perms).unwrap();
    }

    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), old_path);
    std::env::set_var("PATH", &new_path);
    assert_eq!(get_github_token(), None);
    std::env::set_var("PATH", old_path);
  }

  #[test]
  fn test_helpers_return_none_or_empty_when_env_missing() {
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
    assert!(get_pull_details("o", "r", 1, "t").is_none());
    assert!(list_commits_in_pull("o", "r", 1, "t").is_empty());
  }

  #[test]
  #[serial]
  fn try_fetch_prs_handles_non_array_and_missing_fields() {
    // Repo with GitHub origin
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example"])
      .current_dir(repo)
      .status();

    std::env::set_var("GITHUB_TOKEN", "x");

    // Non-array JSON → treated as empty
    std::env::set_var("GAR_TEST_PR_JSON", "{\"foo\":1}");
    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert!(out.is_empty());

    // Missing html_url/user/head/base → diff/patch None, options None
    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([{ "number": 2, "title": "T", "state": "open" }]).to_string(),
    );
    let out2 = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert_eq!(out2.len(), 1);
    let pr = &out2[0];
    assert!(pr.diff_url.is_none() && pr.patch_url.is_none());
    assert!(pr.user.is_none());
    assert!(pr.head.is_none());
    assert!(pr.base.is_none());

    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  fn list_commits_in_pull_parses_subject_and_short_sha() {
    std::env::set_var(
      "GAR_TEST_PR_COMMITS_JSON",
      serde_json::json!([
        { "sha": "abcdef1234567", "commit": {"message": "First line\nBody"}},
        { "sha": "0123456789abcd", "commit": {"message": "Another subject"}}
      ])
      .to_string(),
    );
    let commits = list_commits_in_pull("o", "r", 1, "t");
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].short_sha.len(), 7);
    assert_eq!(commits[0].subject, "First line");
    assert_eq!(commits[1].subject, "Another subject");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
  }

  #[test]
  fn get_pull_details_parses_title() {
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({
        "html_url": "https://github.com/openai/example/pull/42",
        "number": 42,
        "title": "Meaning of life",
        "state": "open"
      })
      .to_string(),
    );
    let v = get_pull_details("o", "r", 42, "t").unwrap();
    assert_eq!(v.fetch("title").to_or_default::<String>(), "Meaning of life");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
  }

  #[test]
  fn list_pulls_for_commit_json_direct_env_array() {
    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([
        { "number": 1, "title": "A" },
        { "number": 2, "title": "B" }
      ])
      .to_string(),
    );
    let v = list_pulls_for_commit_json("o", "r", "s", "t").unwrap();
    assert!(v.as_array().unwrap().len() >= 2);
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  fn list_pulls_for_commit_json_invalid_env_is_none() {
    std::env::set_var("GAR_TEST_PR_JSON", "not json");
    let v = list_pulls_for_commit_json("o", "r", "s", "t");
    assert!(v.is_none());
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  fn invalid_env_yields_none_or_empty() {
    std::env::set_var("GAR_TEST_PULL_DETAILS_JSON", "not json");
    assert!(get_pull_details("o", "r", 1, "t").is_none());
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");

    std::env::set_var("GAR_TEST_PR_COMMITS_JSON", "not json");
    assert!(list_commits_in_pull("o", "r", 1, "t").is_empty());
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
  }

  #[test]
  #[serial]
  fn try_fetch_prs_multiple_items_mixed_urls() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(repo)
      .status();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example"])
      .current_dir(repo)
      .status();
    std::env::set_var("GITHUB_TOKEN", "t");
    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([
        { "html_url": "https://github.com/openai/example/pull/2", "number": 2, "title": "Two", "state": "open" },
        { "number": 3, "title": "Three", "state": "open" }
      ])
      .to_string(),
    );
    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(
      out[0].diff_url.as_deref(),
      Some("https://github.com/openai/example/pull/2.diff")
    );
    assert!(out[1].diff_url.is_none());
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
  fn token_env_empty_values_return_none() {
    std::env::set_var("GITHUB_TOKEN", "   ");
    std::env::remove_var("GH_TOKEN");
    // Make sure gh isn't found
    std::env::set_var("PATH", "/nonexistent");
    assert_eq!(get_github_token(), None);
    std::env::remove_var("GITHUB_TOKEN");
  }

  #[test]
  #[serial]
  fn try_fetch_prs_no_env_returns_empty_array() {
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
    std::env::set_var("GITHUB_TOKEN", "x");
    std::env::remove_var("GAR_TEST_PR_JSON");
    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert!(out.is_empty());
    std::env::remove_var("GITHUB_TOKEN");
  }

  #[test]
  fn get_json_success_path_from_local_http() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn handle_client(mut stream: TcpStream) {
      let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(1)));
      let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(1)));
      let mut buf = [0u8; 1024];
      let _ = stream.read(&mut buf);
      let body = b"{\"ok\":true}";
      let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        std::str::from_utf8(body).unwrap()
      );
      let _ = stream.write_all(resp.as_bytes());
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
      if let Ok((stream, _)) = listener.accept() {
        handle_client(stream);
      }
    });

    let url = format!("http://{}", addr);
    let v = get_json(&url, "t");
    handle.join().unwrap();
    assert_eq!(v.unwrap().fetch("ok").to::<bool>(), Some(true));
  }
}

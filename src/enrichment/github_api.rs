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
use crate::model::{GithubPullRequest, PullRequestCommit, GithubUser};
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

#[cfg(not(test))]
fn list_pulls_for_commit_json(owner: &str, name: &str, sha: &str, token: &str) -> Option<serde_json::Value> {
  let url = format!(
    "https://api.github.com/repos/{}/{}/commits/{}/pulls",
    owner, name, sha
  );
  get_json(&url, token)
}

#[cfg(test)]
fn list_pulls_for_commit_json(_owner: &str, _name: &str, _sha: &str, _token: &str) -> Option<serde_json::Value> {
  if let Ok(s) = std::env::var("GAR_TEST_PR_JSON") {
    serde_json::from_str::<serde_json::Value>(&s).ok()
  } else {
    Some(serde_json::json!([]))
  }
}

/// Best-effort: fetch PRs referencing a commit SHA using origin and token discovery.
pub fn try_fetch_prs_for_commit(repo: &str, sha: &str) -> anyhow::Result<Vec<GithubPullRequest>> {
  let mut out: Vec<GithubPullRequest> = Vec::new();

  let (owner, name) = match parse_origin_github(repo) {
    Some(pair) => pair,
    None => return Ok(out),
  };

  let token = match get_github_token() {
    Some(t) => t,
    None => return Ok(out),
  };

  let parsed = list_pulls_for_commit_json(&owner, &name, sha, &token).unwrap_or_else(|| serde_json::json!([]));
  let arr = match parsed.as_array() { Some(a) => a, None => return Ok(out) };

  for pr_json in arr {
    let html = pr_json.fetch("html_url").to_or_default::<String>();
    let user_login = pr_json.fetch("user.login").to::<String>();
    let user = user_login.map(|login| GithubUser { login: Some(login) });
    let head = pr_json.fetch("head.ref").to::<String>();
    let base = pr_json.fetch("base.ref").to::<String>();

    let item = GithubPullRequest {
      number: pr_json.fetch("number").to::<i64>().unwrap_or(0),
      title: pr_json.fetch("title").to_or_default::<String>(),
      state: pr_json.fetch("state").to_or_default::<String>(),
      body: pr_json.fetch("body").to::<String>(),
      created_at: pr_json.fetch("created_at").to::<String>(),
      merged_at: pr_json.fetch("merged_at").to::<String>(),
      closed_at: pr_json.fetch("closed_at").to::<String>(),
      html_url: html.clone(),
      diff_url: if html.is_empty() { None } else { Some(format!("{}.diff", html)) },
      patch_url: if html.is_empty() { None } else { Some(format!("{}.patch", html)) },
      user,
      head,
      base,
      commits: None,
    };
    out.push(item);
  }

  Ok(out)
}

pub fn get_pull_details(owner: &str, name: &str, number: i64, token: &str) -> Option<serde_json::Value> {
  let url = format!(
    "https://api.github.com/repos/{}/{}/pulls/{}",
    owner, name, number
  );
  get_json(&url, token)
}

pub fn list_commits_in_pull(owner: &str, name: &str, number: i64, token: &str) -> Vec<PullRequestCommit> {
  let mut out = Vec::new();
  let url = format!(
    "https://api.github.com/repos/{}/{}/pulls/{}/commits",
    owner, name, number
  );
  if let Some(v) = get_json(&url, token) {
    if let Some(arr) = v.as_array() {
      for item in arr {
        let sha = item.fetch("sha").to_or_default::<String>();
        let msg = item.fetch("commit.message").to_or_default::<String>();
        let subject = msg.lines().next().unwrap_or("").to_string();
        if !sha.is_empty() {
          out.push(PullRequestCommit {
            short_sha: sha.chars().take(7).collect(),
            sha,
            subject,
          });
        }
      }
    }
  }
  out
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
    assert_eq!(pr.diff_url.as_deref(), Some("https://github.com/openai/example/pull/1.diff"));
    assert_eq!(pr.patch_url.as_deref(), Some("https://github.com/openai/example/pull/1.patch"));

    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }
}

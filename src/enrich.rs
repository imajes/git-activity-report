use crate::model::GithubPullRequest;
use crate::ext::serde_json::JsonFetch;
use crate::util::run_git;
use anyhow::Result;

fn parse_origin_github(repo: &str) -> Option<(String, String)> {
  if let Ok(url) = run_git(repo, &["config".into(), "--get".into(), "remote.origin.url".into()]) {
    let u = url.trim();
    let re1 = regex::Regex::new(r"^(?:git@github\.com:|https?://github\.com/)([^/]+)/([^/]+?)(?:\.git)?$").unwrap();

    if let Some(c) = re1.captures(u) {
      let owner = c.get(1).unwrap().as_str().to_string();
      let repo_name = c.get(2).unwrap().as_str().to_string();

      return Some((owner, repo_name));
    }
  }
  None
}

#[cfg(not(test))]
fn fetch_pulls_json(owner: &str, name: &str, sha: &str, token: &str) -> Result<serde_json::Value> {
  let url = format!(
    "https://api.github.com/repos/{}/{}/commits/{}/pulls",
    owner, name, sha
  );
  let agent = ureq::AgentBuilder::new().build();

  let response = agent
    .get(&url)
    .set("Accept", "application/vnd.github+json")
    .set("User-Agent", "git-activity-report")
    .set("Authorization", &format!("Bearer {}", token))
    .call();

  match response {
    Ok(resp) => Ok(resp.into_json()?),
    Err(_) => Ok(serde_json::json!([])),
  }
}

#[cfg(test)]
fn fetch_pulls_json(_owner: &str, _name: &str, _sha: &str, _token: &str) -> Result<serde_json::Value> {
  if let Ok(s) = std::env::var("GAR_TEST_PR_JSON") {
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    Ok(v)
  } else {
    Ok(serde_json::json!([]))
  }
}

pub fn try_fetch_prs(repo: &str, sha: &str) -> Result<Vec<GithubPullRequest>> {
  let mut out: Vec<GithubPullRequest> = Vec::new();

  // Guard 1: repository must be a GitHub origin
  let (owner, name) = match parse_origin_github(repo) {
    Some(pair) => pair,
    None => return Ok(out),
  };

  // Guard 2: token must be present
  let token = match std::env::var("GITHUB_TOKEN") {
    Ok(t) => t,
    Err(_) => return Ok(out),
  };

  // Fetch PRs JSON (network in prod; env-provided in tests)
  let parsed_json: serde_json::Value = fetch_pulls_json(&owner, &name, sha, &token)?;

  // Guard 5: top-level value must be an array
  let json_pull_requests = match parsed_json.as_array() {
    Some(a) => a,
    None => return Ok(out),
  };

  // Map JSON → GithubPullRequest
  for pr_json in json_pull_requests {
    let html = pr_json.fetch("html_url").to_or_default::<String>();

    let pr_user_login = pr_json.fetch("user.login").to::<String>();
    let pr_user = pr_user_login.map(|login| crate::model::GithubUser {
      login: Some(login),
    });

    let pr_head = pr_json.fetch("head.ref").to::<String>();

    let pr_base = pr_json.fetch("base.ref").to::<String>();

    let pr_item = GithubPullRequest {
      user: pr_user,
      head: pr_head,
      base: pr_base,

      number: pr_json.fetch("number").to::<i64>().unwrap_or(0),
      title: pr_json.fetch("title").to_or_default::<String>(),
      state: pr_json.fetch("state").to_or_default::<String>(),
      created_at: pr_json.fetch("created_at").to::<String>(),
      merged_at: pr_json.fetch("merged_at").to::<String>(),

      html_url: html.clone(),
      diff_url: if html.is_empty() { None } else { Some(format!("{}.diff", html)) },
      patch_url: if html.is_empty() { None } else { Some(format!("{}.patch", html)) },
    };

    out.push(pr_item);
  }

  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;
  use serial_test::serial;

  #[test]
  fn parse_origin_none_without_remote() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let st = std::process::Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap();
    assert!(st.success());
    assert_eq!(parse_origin_github(repo.to_str().unwrap()), None);
  }

  #[test]
  fn parse_origin_github_detects_owner_repo() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap();
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
    let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example.git"])
      .current_dir(repo)
      .status();
    // Ensure token and mocked JSON are not present for this test
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
    let out = try_fetch_prs(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert!(out.is_empty());
  }

  #[test]
  #[serial]
  fn try_fetch_prs_with_token_and_env_mock_returns_item() {
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap();
    let _ = std::process::Command::new("git")
      .args(["remote", "add", "origin", "https://github.com/openai/example.git"])
      .current_dir(repo)
      .status();

    std::env::set_var("GITHUB_TOKEN", "test-token");

    let body = serde_json::json!([{
      "html_url": "https://github.com/openai/example/pull/1",
      "number": 1,
      "title": "Add feature",
      "state": "open",
      "user": {"login": "octo"},
      "head": {"ref": "feature/x"},
      "base": {"ref": "main"},
      "created_at": "2024-01-01T00:00:00Z",
      "merged_at": null
    }]).to_string();
    std::env::set_var("GAR_TEST_PR_JSON", body);

    let out = try_fetch_prs(repo.to_string_lossy().as_ref(), "deadbeef").unwrap();
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
    // Cleanup env for other tests
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GAR_TEST_PR_JSON");
  }
}

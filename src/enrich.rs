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

  // Build request
  let url = format!(
    "https://api.github.com/repos/{}/{}/commits/{}/pulls",
    owner, name, sha
  );
  let agent = ureq::AgentBuilder::new().build();

  // Guard 3: HTTP call must succeed
  let response = match agent
    .get(&url)
    .set("Accept", "application/vnd.github+json")
    .set("User-Agent", "git-activity-report")
    .set("Authorization", &format!("Bearer {}", token))
    .call()
  {
    Ok(resp) => resp,
    Err(_) => return Ok(out),
  };

  // Guard 4: response must parse as JSON
  let parsed_json: serde_json::Value = match response.into_json() {
    Ok(v) => v,
    Err(_) => return Ok(out),
  };

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

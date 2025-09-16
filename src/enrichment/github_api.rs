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
use crate::util::diff_seconds;
use crate::util::run_git;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

/// Parse `remote.origin.url` to extract (owner, repo) when hosted on GitHub.
type OriginCache = Mutex<std::collections::HashMap<String, Option<(String, String)>>>;

pub fn parse_origin_github(repo: &str) -> Option<(String, String)> {
  static RE_ORIGIN: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^(?:git@github\.com:|https?://github\.com/)([^/]+)/([^/]+?)(?:\.git)?$").unwrap());
  static CACHE: Lazy<OriginCache> = Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

  if let Some(cached) = CACHE.lock().ok().and_then(|m| m.get(repo).cloned()) {
    return cached;
  }

  let out = run_git(repo, &["config".into(), "--get".into(), "remote.origin.url".into()]);

  let res = match out {
    Ok(url) => {
      let u = url.trim();
      let re1 = &*RE_ORIGIN;

      if let Some(c) = re1.captures(u) {
        let owner = c.get(1).map(|m| m.as_str().to_string());
        let repo_name = c.get(2).map(|m| m.as_str().to_string());

        if let (Some(o), Some(r)) = (owner, repo_name) {
          Some((o, r))
        } else {
          None
        }
      } else {
        None
      }
    }
    Err(_) => None,
  };

  if let Ok(mut map) = CACHE.lock() {
    map.insert(repo.to_string(), res.clone());
  }

  res
}

/// Discover a GitHub token: env var first, then `gh auth token` if available.
pub fn get_github_token() -> Option<String> {
  if let Ok(t) = std::env::var("GITHUB_TOKEN") {
    if !t.trim().is_empty() {
      return Some(t);
    }
  }

  if let Ok(gh_token) = std::env::var("GH_TOKEN") {
    if !gh_token.trim().is_empty() {
      return Some(gh_token);
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
  let agent: ureq::Agent = ureq::Agent::config_builder().build().into();

  let resp = agent
    .get(url)
    .header("Accept", "application/vnd.github+json")
    .header("User-Agent", "git-activity-report")
    .header("Authorization", &format!("Bearer {}", token))
    .call();

  match resp {
    Ok(mut r) => r.body_mut().read_json::<serde_json::Value>().ok(),
    Err(_) => None,
  }
}

// --- Trait seam for GitHub API ---
pub trait GithubApi {
  fn list_pulls_for_commit_json(&self, owner: &str, name: &str, sha: &str) -> Option<serde_json::Value>;
  fn get_pull_details_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value>;
  fn list_commits_in_pull(&self, owner: &str, name: &str, number: i64) -> Vec<PullRequestCommit>;
  fn list_reviews_for_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value>;
  fn list_commits_in_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value>;
  fn get_user_json(&self, login: &str) -> Option<serde_json::Value>;
}

// --- Lightweight in-memory caching wrapper ---
// Caches remote API responses per run to avoid duplicate HTTP calls.
struct GithubCachedApi {
  inner: Box<dyn GithubApi>,
  pulls_for_commit_json: RefCell<HashMap<String, Option<serde_json::Value>>>,
  pull_details_json: RefCell<HashMap<String, Option<serde_json::Value>>>,
  pull_reviews_json: RefCell<HashMap<String, Option<serde_json::Value>>>,
  pull_commits_json: RefCell<HashMap<String, Option<serde_json::Value>>>,
  pull_commits_typed: RefCell<HashMap<String, Vec<PullRequestCommit>>>,
  user_json: RefCell<HashMap<String, Option<serde_json::Value>>>,
}

impl GithubCachedApi {
  fn new(inner: Box<dyn GithubApi>) -> Self {
    Self {
      inner,
      pulls_for_commit_json: RefCell::new(HashMap::new()),
      pull_details_json: RefCell::new(HashMap::new()),
      pull_reviews_json: RefCell::new(HashMap::new()),
      pull_commits_json: RefCell::new(HashMap::new()),
      pull_commits_typed: RefCell::new(HashMap::new()),
      user_json: RefCell::new(HashMap::new()),
    }
  }

  #[inline]
  fn key3(a: &str, b: &str, c: &str) -> String {
    format!("{}:{}:{}", a, b, c)
  }

  #[inline]
  fn key_num(a: &str, b: &str, n: i64) -> String {
    format!("{}:{}:{}", a, b, n)
  }
}

impl GithubApi for GithubCachedApi {
  fn list_pulls_for_commit_json(&self, owner: &str, name: &str, sha: &str) -> Option<serde_json::Value> {
    let key = Self::key3(owner, name, sha);

    if let Some(v) = self.pulls_for_commit_json.borrow().get(&key).cloned() {
      return v;
    }
    let v = self.inner.list_pulls_for_commit_json(owner, name, sha);
    self.pulls_for_commit_json.borrow_mut().insert(key, v.clone());

    v
  }

  fn get_pull_details_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let key = Self::key_num(owner, name, number);

    if let Some(v) = self.pull_details_json.borrow().get(&key).cloned() {
      return v;
    }
    let v = self.inner.get_pull_details_json(owner, name, number);
    self.pull_details_json.borrow_mut().insert(key, v.clone());

    v
  }

  fn list_commits_in_pull(&self, owner: &str, name: &str, number: i64) -> Vec<PullRequestCommit> {
    let key = Self::key_num(owner, name, number);

    if let Some(v) = self.pull_commits_typed.borrow().get(&key).cloned() {
      return v;
    }
    let v = self.inner.list_commits_in_pull(owner, name, number);
    self.pull_commits_typed.borrow_mut().insert(key, v.clone());

    v
  }

  fn list_reviews_for_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let key = Self::key_num(owner, name, number);

    if let Some(v) = self.pull_reviews_json.borrow().get(&key).cloned() {
      return v;
    }
    let v = self.inner.list_reviews_for_pull_json(owner, name, number);
    self.pull_reviews_json.borrow_mut().insert(key, v.clone());

    v
  }

  fn list_commits_in_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let key = Self::key_num(owner, name, number);

    if let Some(v) = self.pull_commits_json.borrow().get(&key).cloned() {
      return v;
    }
    let v = self.inner.list_commits_in_pull_json(owner, name, number);
    self.pull_commits_json.borrow_mut().insert(key, v.clone());

    v
  }

  fn get_user_json(&self, login: &str) -> Option<serde_json::Value> {
    if let Some(v) = self.user_json.borrow().get(login).cloned() {
      return v;
    }
    let v = self.inner.get_user_json(login);
    self.user_json.borrow_mut().insert(login.to_string(), v.clone());

    v
  }
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
    let url = format!("https://api.github.com/repos/{}/{}/commits/{}/pulls", owner, name, sha);
    get_json(&url, &self.token)
  }

  fn get_pull_details_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let url = format!("https://api.github.com/repos/{}/{}/pulls/{}", owner, name, number);
    get_json(&url, &self.token)
  }

  fn list_commits_in_pull(&self, owner: &str, name: &str, number: i64) -> Vec<PullRequestCommit> {
    let url = format!(
      "https://api.github.com/repos/{}/{}/pulls/{}/commits",
      owner, name, number
    );

    let Some(v) = get_json(&url, &self.token) else {
      return Vec::new();
    };
    let Some(arr) = v.as_array() else { return Vec::new() };

    let mut out = Vec::with_capacity(arr.len());

    for item in arr {
      let sha = item.fetch("sha").to_or_default::<String>();
      let msg = item.fetch("commit.message").to_or_default::<String>();
      let subject = msg.lines().next().unwrap_or("").to_string();

      if !sha.is_empty() {
        let pr_commit = PullRequestCommit {
          short_sha: sha.chars().take(7).collect(),
          sha,
          subject,
        };

        out.push(pr_commit);
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

  fn list_commits_in_pull_json(&self, owner: &str, name: &str, number: i64) -> Option<serde_json::Value> {
    let url = format!(
      "https://api.github.com/repos/{}/{}/pulls/{}/commits",
      owner, name, number
    );
    get_json(&url, &self.token)
  }

  fn get_user_json(&self, login: &str) -> Option<serde_json::Value> {
    let url = format!("https://api.github.com/users/{}", login);
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
    let Ok(s) = std::env::var("GAR_TEST_PR_COMMITS_JSON") else {
      return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) else {
      return Vec::new();
    };
    let Some(arr) = v.as_array() else { return Vec::new() };

    let mut out = Vec::with_capacity(arr.len());

    for item in arr {
      let sha = item.fetch("sha").to_or_default::<String>();
      let msg = item.fetch("commit.message").to_or_default::<String>();
      let subject = msg.lines().next().unwrap_or("").to_string();

      if !sha.is_empty() {
        let pr_commit = PullRequestCommit {
          short_sha: sha.chars().take(7).collect(),
          sha,
          subject,
        };

        out.push(pr_commit);
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

  fn list_commits_in_pull_json(&self, _owner: &str, _name: &str, _number: i64) -> Option<serde_json::Value> {
    if let Ok(s) = std::env::var("GAR_TEST_PR_COMMITS_JSON") {
      serde_json::from_str::<serde_json::Value>(&s).ok()
    } else {
      None
    }
  }

  fn get_user_json(&self, login: &str) -> Option<serde_json::Value> {
    // Prefer a consolidated map when provided
    if let Ok(map_s) = std::env::var("GAR_TEST_USERS_JSON") {
      if let Ok(map_v) = serde_json::from_str::<serde_json::Value>(&map_s) {
        if let Some(obj) = map_v.as_object() {
          if let Some(u) = obj.get(login) {
            return Some(u.clone());
          }
        }
      }
    }
    // Fallback to per-login var: GAR_TEST_USER_JSON_<login>
    let key = format!("GAR_TEST_USER_JSON_{}", login);

    if let Ok(s) = std::env::var(key) {
      return serde_json::from_str::<serde_json::Value>(&s).ok();
    }
    None
  }
}

fn env_wants_mock() -> bool {
  if std::env::var("GAR_TEST_PR_JSON").is_ok()
    || std::env::var("GAR_TEST_PULL_DETAILS_JSON").is_ok()
    || std::env::var("GAR_TEST_PR_COMMITS_JSON").is_ok()
    || std::env::var("GAR_TEST_USERS_JSON").is_ok()
  {
    return true;
  }

  // Detect any per-login user fixtures
  for (k, _) in std::env::vars() {
    if k.starts_with("GAR_TEST_USER_JSON_") {
      return true;
    }
  }
  false
}

fn build_api(token: Option<String>) -> Box<dyn GithubApi> {
  let inner: Box<dyn GithubApi> = if env_wants_mock() {
    Box::new(GithubEnvApi)
  } else if let Some(t) = token {
    Box::new(GithubHttpApi::new(t))
  } else {
    Box::new(GithubEnvApi)
  };

  Box::new(GithubCachedApi::new(inner))
}

// Public constructors for dependency injection in higher layers/tests.
#[cfg(any(test, feature = "testutil"))]
pub fn make_http_api(token: String) -> Box<dyn GithubApi> {
  let inner: Box<dyn GithubApi> = Box::new(GithubHttpApi::new(token));
  Box::new(GithubCachedApi::new(inner))
}
#[cfg(any(test, feature = "testutil"))]
pub fn make_env_api() -> Box<dyn GithubApi> {
  let inner: Box<dyn GithubApi> = Box::new(GithubEnvApi);
  Box::new(GithubCachedApi::new(inner))
}
#[cfg(any(test, feature = "testutil"))]
pub fn make_default_api(token: Option<String>) -> Box<dyn GithubApi> {
  build_api(token)
}

#[cfg(any(test, feature = "testutil"))]
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
    // Extract common display fields first
    let common = build_common_pr_fields(pr_json);
    let submitter_login = pr_json.fetch("user.login").to::<String>();

    // Pull details and reviews for metrics & classification (best‑effort)
    let details = api.get_pull_details_json(&owner, &name, common.number);
    let reviews = api.list_reviews_for_pull_json(&owner, &name, common.number);
    let commits_json = api.list_commits_in_pull_json(&owner, &name, common.number);

    // Compute metrics
    let mut review_count: Option<i64> = None;
    let mut approval_count: Option<i64> = None;
    let mut change_request_count: Option<i64> = None;
    let mut time_to_first_review_seconds: Option<i64> = None;
    let mut time_to_merge_seconds: Option<i64> = None;

    let mut approver: Option<GithubUser> = None;
    let mut reviewers_vec: Vec<GithubUser> = Vec::new();

    if let Some(rev_arr) = reviews.as_ref().and_then(|v| v.as_array()) {
      let (rc, ac, cc, first_ts, app_opt, reviewers) = process_reviews(api.as_ref(), rev_arr, details.as_ref());
      review_count = Some(rc);
      approval_count = Some(ac);
      change_request_count = Some(cc);
      time_to_first_review_seconds = first_ts;
      approver = app_opt;
      reviewers_vec = reviewers;
    }

    // Submitter
    let submitter = submitter_login
      .as_ref()
      .map(|login| build_submitter_user(api.as_ref(), login, commits_json.as_ref(), details.as_ref()));

    // time_to_merge
    if let Some(d) = &details {
      if let (Some(created), Some(merged)) = (
        d.fetch("created_at").to::<String>(),
        d.fetch("merged_at").to::<String>(),
      ) {
        time_to_merge_seconds = diff_seconds(&created, &merged);
      }
    }

    let (created_at, merged_at, closed_at) = resolve_timestamps(pr_json, details.as_ref());

    let reviewers = if reviewers_vec.is_empty() {
      None
    } else {
      Some(reviewers_vec)
    };

    let commits_vec = api.list_commits_in_pull(&owner, &name, common.number);
    let commits_opt = (!commits_vec.is_empty()).then_some(commits_vec);

    let item = GithubPullRequest {
      number: common.number,
      title: common.title,
      state: common.state,
      body_lines: common.body_lines.clone(),
      created_at,
      merged_at,
      closed_at,
      html_url: common.html_url.clone(),
      diff_url: common.diff_url.clone(),
      patch_url: common.patch_url.clone(),
      submitter,
      approver,
      reviewers,
      head: common.head.clone(),
      base: common.base.clone(),
      commits: commits_opt,
      review_count,
      approval_count,
      change_request_count,
      time_to_first_review_seconds,
      time_to_merge_seconds,
      estimated_minutes: None,
      estimated_minutes_min: None,
      estimated_minutes_max: None,
      estimate_confidence: None,
      estimate_basis: None,
    };
    out.push(item);
  }

  Ok(out)
}

/// Derive diff/patch URLs from a PR `html_url`.
fn urls_from_html(html: &str) -> (Option<String>, Option<String>) {
  if html.is_empty() {
    (None, None)
  } else {
    (Some(format!("{}.diff", html)), Some(format!("{}.patch", html)))
  }
}

/// Common, display-oriented PR fields used across enrichment paths.
///
/// Field grouping follows repo conventions:
/// - Identity/relations: `head`, `base`
/// - Core scalars: `number`, `title`, `state`
/// - Temporal: (resolved later by `resolve_timestamps`)
/// - Links: `html_url`, `diff_url`, `patch_url`
/// - Optional/derived: `body_lines`
#[derive(Debug, Clone)]
struct PrCommonFields {
  /// PR number (identity)
  number: i64,
  /// PR title (scalar)
  title: String,
  /// PR state (open/closed) (scalar)
  state: String,
  /// Submitter message body split by lines (optional/derived)
  body_lines: Option<Vec<String>>,
  /// HTML URL (links)
  html_url: String,
  /// Diff URL derived from HTML URL (links)
  diff_url: Option<String>,
  /// Patch URL derived from HTML URL (links)
  patch_url: Option<String>,
  /// Source branch name (relation)
  head: Option<String>,
  /// Base branch name (relation)
  base: Option<String>,
}

/// Extract common PR fields from the GitHub API PR JSON object.
fn build_common_pr_fields(pr_json: &serde_json::Value) -> PrCommonFields {
  let number = pr_json.fetch("number").to::<i64>().unwrap_or(0);
  let title = pr_json.fetch("title").to_or_default::<String>();
  let state = pr_json.fetch("state").to_or_default::<String>();

  let html_url = pr_json.fetch("html_url").to_or_default::<String>();
  let (diff_url, patch_url) = urls_from_html(&html_url);

  let body_lines = pr_json
    .fetch("body")
    .to::<String>()
    .map(|b| b.lines().map(|s| s.to_string()).collect());

  let head = pr_json.fetch("head.ref").to::<String>();
  let base = pr_json.fetch("base.ref").to::<String>();

  PrCommonFields {
    number,
    title,
    state,
    body_lines,
    html_url,
    diff_url,
    patch_url,
    head,
    base,
  }
}

/// Aggregate review counts/approver/reviewers and compute the time to first review.
fn process_reviews(
  api: &dyn GithubApi,
  rev_arr: &[serde_json::Value],
  details: Option<&serde_json::Value>,
) -> (i64, i64, i64, Option<i64>, Option<GithubUser>, Vec<GithubUser>) {
  let mut approvals = 0i64;
  let mut changes = 0i64;
  let mut first_review_ts: Option<String> = None;
  let mut latest_approved_ts: Option<String> = None;
  let mut latest_approved_login: Option<String> = None;
  use std::collections::BTreeSet;
  let mut seen_logins: BTreeSet<String> = BTreeSet::new();
  let mut reviewers_vec: Vec<GithubUser> = Vec::new();

  for r in rev_arr {
    let state_str = r.fetch("state").to_or_default::<String>();
    let login_opt = r.fetch("user.login").to::<String>();
    let assoc_opt = r.fetch("author_association").to::<String>();
    let submitted_at = r.fetch("submitted_at").to::<String>();

    if let Some(ts) = &submitted_at {
      if first_review_ts.as_ref().map(|cur| ts < cur).unwrap_or(true) {
        first_review_ts = Some(ts.clone());
      }
    }

    if state_str.eq_ignore_ascii_case("APPROVED") {
      approvals += 1;
      if let Some(ts) = &submitted_at {
        if latest_approved_ts.as_ref().map(|cur| ts > cur).unwrap_or(true) {
          latest_approved_ts = Some(ts.clone());
          latest_approved_login = login_opt.clone();
        }
      }
    } else if state_str.eq_ignore_ascii_case("CHANGES_REQUESTED") {
      changes += 1;
    }

    let Some(login) = login_opt else { continue };

    if !seen_logins.insert(login.clone()) {
      continue;
    }

    let assoc = assoc_opt.unwrap_or_default();
    let mut user_type = classify_user(&login, Some(&assoc));
    let user_json = api.get_user_json(&login);
    let email = user_json.as_ref().and_then(|u| u.fetch("email").to::<String>());

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
    let reviewer = GithubUser {
      login: Some(login.clone()),
      profile_url: Some(format!("https://github.com/{}", login)),
      r#type: Some(user_type),
      email,
    };
    reviewers_vec.push(reviewer);
  }

  let mut approver: Option<GithubUser> = None;

  if let Some(login) = latest_approved_login {
    let approver_email = api.get_user_json(&login).and_then(|u| u.fetch("email").to::<String>());
    let user_type = details
      .map(|_| classify_user(&login, None))
      .unwrap_or_else(|| "unknown".into());
    approver = Some(GithubUser {
      login: Some(login.clone()),
      profile_url: Some(format!("https://github.com/{}", login)),
      r#type: Some(user_type),
      email: approver_email,
    });
  } else if let Some(d) = details {
    if let Some(mby) = d.fetch("merged_by.login").to::<String>() {
      let merged_by_email = api.get_user_json(&mby).and_then(|u| u.fetch("email").to::<String>());
      approver = Some(GithubUser {
        login: Some(mby.clone()),
        profile_url: Some(format!("https://github.com/{}", mby)),
        r#type: Some(classify_user(&mby, None)),
        email: merged_by_email,
      });
    }
  }

  let time_to_first = if let (Some(first_ts), Some(created)) = (
    first_review_ts,
    details.and_then(|d| d.fetch("created_at").to::<String>()),
  ) {
    diff_seconds(&created, &first_ts)
  } else {
    None
  };

  (
    rev_arr.len() as i64,
    approvals,
    changes,
    time_to_first,
    approver,
    reviewers_vec,
  )
}

/// Resolve created/merged/closed timestamps from PR JSON with optional details override.
fn resolve_timestamps(
  pr_json: &serde_json::Value,
  details: Option<&serde_json::Value>,
) -> (Option<String>, Option<String>, Option<String>) {
  let created_at_primary = pr_json.fetch("created_at").to::<String>();
  let created_at = created_at_primary.or_else(|| details.and_then(|d| d.fetch("created_at").to::<String>()));

  let merged_at_primary = pr_json.fetch("merged_at").to::<String>();
  let merged_at = merged_at_primary.or_else(|| details.and_then(|d| d.fetch("merged_at").to::<String>()));

  let closed_at_primary = pr_json.fetch("closed_at").to::<String>();
  let closed_at = closed_at_primary.or_else(|| details.and_then(|d| d.fetch("closed_at").to::<String>()));

  (created_at, merged_at, closed_at)
}

/// Build a `GithubUser` for the PR submitter, attempting to classify and resolve email.
fn build_submitter_user(
  api: &dyn GithubApi,
  login: &str,
  commits_json: Option<&serde_json::Value>,
  details: Option<&serde_json::Value>,
) -> GithubUser {
  let user_type = details
    .and_then(|d| d.fetch("author_association").to::<String>())
    .as_deref()
    .map(classify_assoc)
    .unwrap_or_else(|| classify_user(login, None));

  let email_from_user = api.get_user_json(login).and_then(|u| u.fetch("email").to::<String>());

  let email_from_commits = submitter_email_fallback(commits_json, login);

  let resolved_email = email_from_user.or(email_from_commits);

  GithubUser {
    login: Some(login.to_string()),
    profile_url: Some(format!("https://github.com/{}", login)),
    r#type: Some(user_type),
    email: resolved_email,
  }
}

// Extracted helper: find submitter email fallback from pull commits JSON.
// Looks for a commit authored by `login` and returns `commit.author.email` if present.
/// Fallback email resolution from the list of PR commits. Returns the commit author email
/// for the entry whose `author.login` matches `login`.
fn submitter_email_fallback(commits_json: Option<&serde_json::Value>, login: &str) -> Option<String> {
  let arr = commits_json.and_then(|c| c.as_array())?;

  let email_opt = arr.iter().find_map(|commit_item| {
    let author_login = commit_item.fetch("author.login").to::<String>();

    if author_login.as_deref() == Some(login) {
      return commit_item.fetch("commit.author.email").to::<String>();
    }

    None
  });

  email_opt
}

fn classify_user(login: &str, assoc_opt: Option<&str>) -> String {
  if login.ends_with("[bot]") {
    return "bot".into();
  }
  if let Some(a) = assoc_opt {
    return classify_assoc(a);
  }
  "unknown".into()
}

fn classify_assoc(a: &str) -> String {
  let s = a.to_ascii_uppercase();

  match s.as_str() {
    "OWNER" | "MEMBER" | "COLLABORATOR" => "member".into(),
    "CONTRIBUTOR" | "FIRST_TIME_CONTRIBUTOR" | "FIRST_TIMER" => "contributor".into(),
    _ => "other".into(),
  }
}

// diff_seconds now lives in crate::util

#[cfg(any(test, feature = "testutil"))]
pub fn get_pull_details(owner: &str, name: &str, number: i64, token: &str) -> Option<serde_json::Value> {
  let api = build_api(Some(token.to_string()));
  api.get_pull_details_json(owner, name, number)
}

#[cfg(any(test, feature = "testutil"))]
pub fn list_commits_in_pull(owner: &str, name: &str, number: i64, token: &str) -> Vec<PullRequestCommit> {
  let api = build_api(Some(token.to_string()));
  api.list_commits_in_pull(owner, name, number)
}

#[cfg(test)]
mod tests {
  use super::*;
  use serial_test::serial;

  #[test]
  #[serial]
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
  #[serial]
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
    assert_eq!(
      pr.submitter.as_ref().and_then(|u| u.login.clone()).as_deref(),
      Some("octo")
    );
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
  #[serial]
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
  #[serial]
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
  fn token_gh_command_success_returns_token() {
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GH_TOKEN");

    let td = tempfile::TempDir::new().unwrap();
    let bin_dir = td.path();
    let gh_path = bin_dir.join("gh");

    #[cfg(target_os = "windows")]
    let script = "@echo off\necho my-gh-token\nexit /b 0\n";
    #[cfg(not(target_os = "windows"))]
    let script = "#!/bin/sh\necho my-gh-token\nexit 0\n";
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

    assert_eq!(get_github_token(), Some("my-gh-token".to_string()));

    std::env::set_var("PATH", old_path);
  }

  #[test]
  #[serial]
  fn submitter_email_fallback_from_commits() {
    // Repo with GitHub origin
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

    // No token; enable env-backed API and simulate missing email in users JSON
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GH_TOKEN");

    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([{ "html_url": "https://github.com/openai/example/pull/1", "number": 1, "title": "T", "state": "open", "user": {"login": "octo"} }]).to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({"created_at": "2024-01-01T00:00:00Z"}).to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_COMMITS_JSON",
      serde_json::json!([
        { "author": {"login": "octo"}, "commit": {"author": {"email": "octo@example.com"}, "message": "Subj\nBody"}, "sha": "abc1234" }
      ]).to_string(),
    );
    // Users JSON with no email for octo → forces fallback from commits JSON
    std::env::set_var(
      "GAR_TEST_USERS_JSON",
      serde_json::json!({ "octo": {"type": "User"} }).to_string(),
    );

    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert_eq!(out.len(), 1);
    let pr = &out[0];
    let submitter_email = pr.submitter.as_ref().and_then(|u| u.email.clone());
    assert_eq!(submitter_email.as_deref(), Some("octo@example.com"));

    std::env::remove_var("GAR_TEST_PR_JSON");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_COMMITS_JSON");
    std::env::remove_var("GAR_TEST_USERS_JSON");
  }

  #[test]
  #[serial]
  fn reviews_dedup_bot_and_first_review_time() {
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
    std::env::remove_var("GH_TOKEN");

    std::env::set_var(
      "GAR_TEST_PR_JSON",
      serde_json::json!([{ "html_url": "https://github.com/openai/example/pull/3", "number": 3, "title": "T", "state": "open" }]).to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PULL_DETAILS_JSON",
      serde_json::json!({"created_at": "2024-02-01T00:00:00Z"}).to_string(),
    );
    std::env::set_var(
      "GAR_TEST_PR_REVIEWS_JSON",
      serde_json::json!([
        {"state": "APPROVED", "user": {"login": "ci-bot[bot]"}, "submitted_at": "2024-02-01T01:00:00Z"},
        {"state": "APPROVED", "user": {"login": "ci-bot[bot]"}, "submitted_at": "2024-02-01T02:00:00Z"},
        {"state": "COMMENTED", "user": {"login": "alice"}, "author_association": "CONTRIBUTOR", "submitted_at": "2024-02-01T03:00:00Z"}
      ]).to_string(),
    );
    std::env::set_var(
      "GAR_TEST_USERS_JSON",
      serde_json::json!({
        "ci-bot[bot]": {"type": "Bot"},
        "alice": {"email": "alice@example.com", "type": "User"}
      })
      .to_string(),
    );

    let out = try_fetch_prs_for_commit(repo.to_str().unwrap(), "deadbeef").unwrap();
    assert_eq!(out.len(), 1);
    let pr = &out[0];
    assert_eq!(pr.review_count, Some(3));
    assert_eq!(pr.approval_count, Some(2));

    // first review 1h after created
    assert_eq!(pr.time_to_first_review_seconds, Some(3600));

    let reviewers = pr.reviewers.as_ref().unwrap();
    assert_eq!(reviewers.len(), 2);
    let bot = reviewers.iter().find(|u| u.login.as_deref() == Some("ci-bot[bot]"));
    assert_eq!(bot.and_then(|u| u.r#type.clone()).as_deref(), Some("bot"));
    let human = reviewers.iter().find(|u| u.login.as_deref() == Some("alice"));
    assert_eq!(human.and_then(|u| u.r#type.clone()).as_deref(), Some("contributor"));

    std::env::remove_var("GAR_TEST_PR_JSON");
    std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
    std::env::remove_var("GAR_TEST_PR_REVIEWS_JSON");
    std::env::remove_var("GAR_TEST_USERS_JSON");
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
    assert!(pr.submitter.is_none());
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
  #[serial]
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
  #[serial]
  fn list_pulls_for_commit_json_invalid_env_is_none() {
    std::env::set_var("GAR_TEST_PR_JSON", "not json");
    let v = list_pulls_for_commit_json("o", "r", "s", "t");
    assert!(v.is_none());
    std::env::remove_var("GAR_TEST_PR_JSON");
  }

  #[test]
  #[serial]
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

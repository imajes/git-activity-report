use assert_cmd::Command;
use serde_json::json;
use test_support;

#[test]
fn estimates_present_when_flag_enabled() {
  test_support::init_tracing();
  test_support::init_insta();
  let _env = test_support::with_env(&[("TZ", "UTC")]);

  // Provide env-backed API payloads to ensure a PR is attached
  std::env::set_var(
    "GAR_TEST_PR_JSON",
    json!([{ "html_url": "https://github.com/openai/example/pull/1", "number": 1, "title": "T", "state": "closed" }]).to_string(),
  );
  std::env::set_var(
    "GAR_TEST_PULL_DETAILS_JSON",
    json!({
      "html_url": "https://github.com/openai/example/pull/1",
      "number": 1,
      "title": "T",
      "state": "closed",
      "user": {"login": "octo"},
      "author_association": "MEMBER",
      "created_at": "2024-01-01T00:00:00Z",
      "merged_at": "2024-01-01T02:00:00Z",
      "closed_at": "2024-01-01T02:00:00Z",
      "head": {"ref": "f"},
      "base": {"ref": "m"}
    })
    .to_string(),
  );
  std::env::set_var(
    "GAR_TEST_PR_REVIEWS_JSON",
    json!([
      {"state": "COMMENTED", "user": {"login": "alice"}, "author_association": "CONTRIBUTOR", "submitted_at": "2024-01-01T01:00:00Z"},
      {"state": "APPROVED", "user": {"login": "bob"}, "author_association": "MEMBER", "submitted_at": "2024-01-01T01:30:00Z"}
    ])
    .to_string(),
  );

  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--tz",
      "utc",
      "--now-override",
      "2025-08-15T12:00:00",
      "--estimate-effort",
      "--github-prs",
    ])
    .output()
    .unwrap();
  assert!(out.status.success(), "cli run failed: {}", String::from_utf8_lossy(&out.stderr));

  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let commits = v["commits"].as_array().expect("commits array");
  // At least one commit has estimated_minutes
  assert!(commits.iter().any(|c| c.get("estimated_minutes").and_then(|x| x.as_f64()).is_some()));

  // And at least one PR on any commit has estimated_minutes when PRs are present
  let any_pr_est = commits.iter().flat_map(|c| {
    c.get("github")
      .and_then(|g| g.get("pull_requests"))
      .and_then(|a| a.as_array().cloned())
      .unwrap_or_default()
  }).any(|pr| pr.get("estimated_minutes").and_then(|x| x.as_f64()).is_some());
  assert!(any_pr_est, "expected some PR estimation present");

  // Clean env
  std::env::remove_var("GAR_TEST_PR_JSON");
  std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
  std::env::remove_var("GAR_TEST_PR_REVIEWS_JSON");
}


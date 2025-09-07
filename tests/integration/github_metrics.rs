use assert_cmd::Command;
use serde_json::json;
use test_support;

#[test]
fn enrichment_populates_review_metrics_and_user_fields() {
  test_support::init_tracing();
  test_support::init_insta();
  let _env = test_support::with_env(&[("TZ", "UTC")]);

  // Env-backed API payloads
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
      {"state": "APPROVED", "user": {"login": "bob"}, "author_association": "MEMBER", "submitted_at": "2024-01-01T01:30:00Z"},
      {"state": "APPROVED", "user": {"login": "carol"}, "author_association": "MEMBER", "submitted_at": "2024-01-01T01:45:00Z"}
    ])
    .to_string(),
  );
  std::env::set_var(
    "GAR_TEST_USERS_JSON",
    json!({
      "octo": {"login":"octo", "email":"octo@example.com", "type":"User"},
      "bob": {"login":"bob", "email":"bob@example.com", "type":"User"}
    })
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
      "--detailed"
    ])
    .output()
    .unwrap();
  assert!(out.status.success(), "cli run failed: {}", String::from_utf8_lossy(&out.stderr));

  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let commits = v["commits"].as_array().unwrap();
  // Find first commit with a PR attached
  let mut found = None;
  for c in commits {
    if let Some(prs) = c.get("github").and_then(|g| g.get("pull_requests")).and_then(|p| p.as_array()) {
      if !prs.is_empty() {
        found = Some(prs[0].clone());
        break;
      }
    }
  }
  let pr = found.expect("at least one PR in enriched output");
  assert_eq!(pr["review_count"].as_i64().unwrap(), 3);
  assert_eq!(pr["approval_count"].as_i64().unwrap(), 2);
  assert_eq!(pr["change_request_count"].as_i64().unwrap_or(0), 0);
  assert_eq!(pr["time_to_first_review_seconds"].as_i64().unwrap(), 3600);
  assert_eq!(pr["time_to_merge_seconds"].as_i64().unwrap(), 7200);
  assert_eq!(pr["submitter"]["login"].as_str().unwrap(), "octo");
  assert_eq!(pr["submitter"]["type"].as_str().unwrap(), "member");
  assert_eq!(pr["submitter"]["email"].as_str().unwrap(), "octo@example.com");

  // Clean env
  std::env::remove_var("GAR_TEST_PR_JSON");
  std::env::remove_var("GAR_TEST_PULL_DETAILS_JSON");
  std::env::remove_var("GAR_TEST_PR_REVIEWS_JSON");
  std::env::remove_var("GAR_TEST_USERS_JSON");
}


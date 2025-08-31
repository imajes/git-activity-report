mod common;
use assert_cmd::Command;

#[test]
fn simple_mode_outputs_expected_shape() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();

  cmd.args([
    "--simple",
    "--since",
    "2025-08-01",
    "--until",
    "2025-09-01",
    "--repo",
    repo_path,
  ]);

  let out = cmd.output().unwrap();

  assert!(out.status.success());

  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  assert_eq!(v["mode"], "simple");

  let since = v["range"]["since"].as_str().unwrap();
  let until = v["range"]["until"].as_str().unwrap();
  assert!(since.starts_with("2025-08-01"));
  assert!(until.starts_with("2025-09-01"));

  assert!(v["count"].as_i64().unwrap() >= 1);

  // Ensure timestamps block present in first commit
  let commits = v["commits"].as_array().unwrap();

  if let Some(first) = commits.first() {
    assert!(first["timestamps"]["author"].is_number());
    assert!(first["patch_ref"]["git_show_cmd"].is_string());
  }
}

#[test]
fn simple_mode_writes_to_file_and_validates_shape() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let tmpdir = tempfile::TempDir::new().unwrap();
  let outfile = tmpdir.path().join("out.json");

  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--simple",
    "--since",
    "2025-08-01",
    "--until",
    "2025-09-01",
    "--repo",
    repo_path,
    "--out",
    outfile.to_str().unwrap(),
    "--tz",
    "utc",
    "--include-merges",
  ]);

  let out = cmd.output().unwrap();
  assert!(out.status.success());

  let data = std::fs::read(&outfile).unwrap();
  let v: serde_json::Value = serde_json::from_slice(&data).unwrap();

  // Top-level shape
  assert_eq!(v["mode"], "simple");
  assert!(v["repo"].as_str().is_some());
  assert!(v["include_merges"].as_bool().is_some());
  assert!(v["include_patch"].as_bool().is_some());
  assert!(v["count"].as_u64().unwrap() >= 1);

  let range = &v["range"];
  assert!(range["since"].as_str().unwrap().starts_with("2025-08-01"));
  assert!(range["until"].as_str().unwrap().starts_with("2025-09-01"));

  let authors = &v["authors"];
  assert!(authors.is_object());
  assert!(!authors.as_object().unwrap().is_empty());

  let summary = &v["summary"];
  assert!(summary["additions"].as_i64().is_some());
  assert!(summary["deletions"].as_i64().is_some());
  assert!(summary["files_touched"].as_u64().is_some());

  // Commit shape (inspect first)
  let commits = v["commits"].as_array().unwrap();
  let c0 = commits.first().expect("at least one commit");
  assert!(c0["sha"].as_str().unwrap().len() >= 7);
  assert!(c0["short_sha"].as_str().unwrap().len() >= 7);
  assert!(c0["parents"].as_array().is_some());
  for who in ["author", "committer"] {
    let p = &c0[who];
    assert!(p["name"].as_str().is_some());
    assert!(p["email"].as_str().is_some());
    assert!(p["date"].as_str().is_some());
  }
  let ts = &c0["timestamps"];
  assert!(ts["author"].as_i64().is_some());
  assert!(ts["commit"].as_i64().is_some());
  assert_eq!(ts["timezone"].as_str().unwrap(), "utc");
  assert!(ts["author_local"].as_str().is_some());
  assert!(ts["commit_local"].as_str().is_some());
  assert!(c0["subject"].as_str().is_some());
  assert!(c0["files"].as_array().is_some());
  if let Some(f0) = c0["files"].as_array().unwrap().first() {
    assert!(f0["file"].as_str().is_some());
    assert!(f0["status"].as_str().is_some());
    // additions/deletions may be null depending on file
    assert!(f0.get("additions").is_some());
    assert!(f0.get("deletions").is_some());
  }
  assert!(c0["diffstat_text"].as_str().is_some());
  let pr = &c0["patch_ref"];
  assert_eq!(pr["embed"].as_bool().unwrap(), false);
  assert!(pr["git_show_cmd"].as_str().unwrap().starts_with("git show"));
  // patch fields are absent when not embedding
  assert!(c0.get("patch").is_none());
  assert!(c0.get("patch_clipped").is_none());
}

mod common;
use std::process::Command;

#[test]
fn simple_mode_outputs_expected_shape() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let mut cmd = Command::new(common::bin_path());

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

mod common;
use std::process::Command;

#[test]
fn simple_patch_clipping_sets_flag() {
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
    "--include-patch",
    "--max-patch-bytes",
    "10",
  ]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let commits = v["commits"].as_array().unwrap();
  assert!(!commits.is_empty());
  assert!(commits[0]["patch_clipped"].as_bool().unwrap());
}

#[test]
fn simple_save_patches_sets_local_path() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let patch_dir = tempfile::TempDir::new().unwrap();
  let mut cmd = Command::new(common::bin_path());
  cmd.args([
    "--simple",
    "--since",
    "2025-08-01",
    "--until",
    "2025-09-01",
    "--repo",
    repo_path,
    "--save-patches",
    patch_dir.path().to_str().unwrap(),
  ]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let commits = v["commits"].as_array().unwrap();
  assert!(!commits.is_empty());
  let path = commits[0]["patch_ref"]["local_patch_file"].as_str().unwrap();
  assert!(std::path::Path::new(path).exists());
}

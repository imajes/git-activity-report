use assert_cmd::Command;
use test_support;

#[test]
fn full_mode_writes_manifest_and_shards_with_unmerged() {
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--split-apart",
    "--since",
    "2025-08-01",
    "--until",
    "2025-09-01",
    "--repo",
    repo_path,
    "--out",
    out_path,
    "--include-merges",
    "--include-unmerged",
  ]);
  let output = cmd.output().unwrap();
  assert!(output.status.success());
  let top: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let dir = top["dir"].as_str().unwrap();
  let file = top["file"].as_str().unwrap();
  let report_path = std::path::Path::new(dir).join(file);
  assert!(report_path.exists(), "report should exist");
  let mf: serde_json::Value = serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();

  // Top-level manifest shape
  assert!(mf["summary"].is_object());
  let range = &mf["summary"]["range"];
  assert!(range["start"].as_str().unwrap().starts_with("2025-08-01"));
  assert!(range["end"].as_str().unwrap().starts_with("2025-09-01"));
  assert!(mf["summary"]["repo"].as_str().is_some());
  assert_eq!(mf["summary"]["report_options"]["include_merges"].as_bool().unwrap(), true);
  assert_eq!(mf["summary"]["report_options"]["include_patch"].as_bool().unwrap(), false);
  assert!(mf["summary"]["count"].as_u64().unwrap() >= 1);
  assert!(mf["authors"].is_object());
  let summary_changes = &mf["summary"]["changeset"];
  assert!(summary_changes["additions"].as_i64().is_some());
  assert!(summary_changes["deletions"].as_i64().is_some());
  assert!(summary_changes["files_touched"].as_u64().is_some());

  // Items shape
  let items = mf["items"].as_array().unwrap();
  assert!(!items.is_empty());
  let it0 = &items[0];
  assert!(it0["sha"].as_str().unwrap().len() >= 7);
  assert!(it0["file"].as_str().unwrap().ends_with(".json"));
  assert!(it0["subject"].as_str().is_some());

  // Unmerged activity structure (if present)
  if let Some(ua) = mf.get("unmerged_activity") {
    assert!(ua["branches_scanned"].as_u64().is_some());
    assert!(ua["total_unmerged_commits"].as_u64().is_some());
    let branches = ua["branches"].as_array().unwrap();
    for b in branches.iter() {
      assert!(b["name"].as_str().is_some());
      assert!(b["items"].as_array().is_some());
    }
  }

  // Validate one shard file contains a full commit object shape
  let shard_rel = items[0]["file"].as_str().unwrap();
  let shard_path = std::path::Path::new(dir).join(shard_rel);
  let c: serde_json::Value = serde_json::from_slice(&std::fs::read(&shard_path).unwrap()).unwrap();
  assert!(c["sha"].as_str().is_some());
  assert!(c["timestamps"]["timezone"].as_str().is_some());
  assert!(
    c["patch_references"]["git_show_cmd"].as_str().unwrap().starts_with("git show")
  );
  if let Some(f0) = c["files"].as_array().and_then(|a| a.first()) {
    assert!(f0["file"].as_str().is_some());
    assert!(f0["status"].as_str().is_some());
  }
}

#[test]
fn full_mode_month_label_and_manifest_filename() {
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--split-apart",
    "--month",
    "2025-08",
    "--repo",
    repo_path,
    "--out",
    out_path,
  ]);
  let output = cmd.output().unwrap();
  assert!(output.status.success());
  let top: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(top["file"].as_str().unwrap(), "report-2025-08.json");
}

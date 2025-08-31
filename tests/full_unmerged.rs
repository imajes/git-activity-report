mod common;
use assert_cmd::Command;

#[test]
fn full_mode_writes_manifest_and_shards_with_unmerged() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--full",
    "--since",
    "2025-08-01",
    "--until",
    "2025-09-01",
    "--repo",
    repo_path,
    "--split-out",
    out_path,
    "--include-merges",
    "--include-unmerged",
  ]);
  let output = cmd.output().unwrap();
  assert!(output.status.success());
  let top: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let dir = top["dir"].as_str().unwrap();
  let manifest_file = top["manifest"].as_str().unwrap();
  assert!(manifest_file.starts_with("manifest-"));
  let manifest_path = std::path::Path::new(dir).join(manifest_file);
  assert!(manifest_path.exists(), "manifest should exist");
  let mf: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

  // Top-level manifest shape
  assert_eq!(mf["mode"], "full");
  assert_eq!(mf["label"].as_str().unwrap(), "window");
  let range = &mf["range"];
  assert!(range["since"].as_str().unwrap().starts_with("2025-08-01"));
  assert!(range["until"].as_str().unwrap().starts_with("2025-09-01"));
  assert!(mf["repo"].as_str().is_some());
  assert_eq!(mf["include_merges"].as_bool().unwrap(), true);
  assert_eq!(mf["include_patch"].as_bool().unwrap(), false);
  assert!(mf["count"].as_u64().unwrap() >= 1);
  assert!(mf["authors"].is_object());
  let summary = &mf["summary"];
  assert!(summary["additions"].as_i64().is_some());
  assert!(summary["deletions"].as_i64().is_some());
  assert!(summary["files_touched"].as_u64().is_some());

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
  let label = mf["label"].as_str().unwrap();
  let shard_path1 = std::path::Path::new(dir).join(label).join(shard_rel);
  let shard_path2 = std::path::Path::new(dir).join(shard_rel);
  assert!(shard_path1.exists() || shard_path2.exists());
  let shard_path = if shard_path1.exists() { shard_path1 } else { shard_path2 };
  let c: serde_json::Value = serde_json::from_slice(&std::fs::read(&shard_path).unwrap()).unwrap();
  assert!(c["sha"].as_str().is_some());
  assert!(c["timestamps"]["timezone"].as_str().is_some());
  assert!(c["patch_ref"]["git_show_cmd"].as_str().unwrap().starts_with("git show"));
  if let Some(f0) = c["files"].as_array().and_then(|a| a.first()) {
    assert!(f0["file"].as_str().is_some());
    assert!(f0["status"].as_str().is_some());
  }
}

#[test]
fn full_mode_month_label_and_manifest_filename() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--full",
    "--month",
    "2025-08",
    "--repo",
    repo_path,
    "--split-out",
    out_path,
  ]);
  let output = cmd.output().unwrap();
  assert!(output.status.success());
  let top: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(top["manifest"].as_str().unwrap(), "manifest-2025-08.json");
  let manifest_path = std::path::Path::new(top["dir"].as_str().unwrap()).join("manifest-2025-08.json");
  let mf: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
  assert_eq!(mf["label"].as_str().unwrap(), "2025-08");
}

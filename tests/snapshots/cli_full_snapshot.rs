mod common;
use assert_cmd::Command;

#[test]
fn cli_full_top_and_manifest_snapshot() {
  let repo = common::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      "--full",
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--out",
      out_path,
      "--tz",
      "utc",
      "--include-merges",
      "--include-unmerged",
    ])
    .output()
    .unwrap();

  assert!(out.status.success());

  let mut top: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  if let Some(obj) = top.as_object_mut() {
    obj.insert("dir".into(), serde_json::Value::String("<dir>".into()));
  }
  // Top-level response
  insta::assert_json_snapshot!("cli_full_top", top);

  // Manifest snapshot
  let dir = serde_json::from_slice::<serde_json::Value>(&out.stdout)
    .unwrap()["dir"].as_str().unwrap().to_string();
  let manifest = serde_json::from_slice::<serde_json::Value>(&out.stdout)
    .unwrap()["manifest"].as_str().unwrap().to_string();
  let path = std::path::Path::new(&dir).join(&manifest);
  let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }
  insta::assert_json_snapshot!("cli_full_manifest", v, {
    ".authors" => insta::sorted_redaction(),
    ".items[*].sha" => "[sha]",
  });
}

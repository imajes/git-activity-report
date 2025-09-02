use test_support;

#[test]
fn cli_full_top_and_manifest_snapshot() {
  test_support::init_tracing();
  test_support::init_insta();
  let _env = test_support::with_env(&[("TZ", "UTC")]);
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();

  let mut cmd = test_support::cmd_bin("git-activity-report");
  let out = cmd
    .args([
      "--split-apart",
      "--for",
      "every month for the last 2 months",
      "--repo",
      repo_path,
      "--out",
      out_path,
      "--tz",
      "utc",
      "--now-override",
      "2025-09-01T12:00:00",
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
  let top_v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top_v["dir"].as_str().unwrap().to_string();
  let pointer = top_v
    .get("manifest")
    .and_then(|v| v.as_str())
    .or_else(|| top_v.get("file").and_then(|v| v.as_str()))
    .expect("pointer file or manifest");
  let path = std::path::Path::new(&dir).join(pointer);
  let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
    obj.insert("generated_at".into(), serde_json::Value::String("[generated]".into()));
  }
  // Normalize items[].sha for stability
  if let Some(items) = v.get_mut("items").and_then(|i| i.as_array_mut()) {
    for it in items.iter_mut() {
      if let Some(obj) = it.as_object_mut() {
        obj.insert("sha".into(), serde_json::Value::String("[sha]".into()));
      }
    }
  }
  insta::assert_json_snapshot!("cli_full_manifest", v, { ".authors" => insta::sorted_redaction() });
}

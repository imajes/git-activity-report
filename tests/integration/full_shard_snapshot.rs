use test_support;

#[test]
fn snapshot_first_shard_commit() {
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
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--out",
      out_path,
      "--include-merges",
      "--tz",
      "utc",
      "--now-override",
      "2025-08-15T12:00:00",
    ])
    .output()
    .unwrap();

  assert!(out.status.success());
  let top: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top["dir"].as_str().unwrap();
  // Open the per-range report (single split) or overall manifest (multi-range)
  let pointer = top.get("file").and_then(|v| v.as_str())
    .or_else(|| top.get("manifest").and_then(|v| v.as_str()))
    .expect("pointer file or manifest");
  let pointed_json: serde_json::Value = serde_json::from_slice(
    &std::fs::read(std::path::Path::new(dir).join(pointer)).unwrap(),
  ).unwrap();

  // In single split mode, shard list is in report.items; in multi-range, items in overall manifest
  let label = pointed_json.get("label").and_then(|v| v.as_str()).unwrap_or("");
  let items = pointed_json["items"].as_array().expect("items array");
  let rel = items.first().expect("one shard")["file"].as_str().unwrap();
  let shard_path = std::path::Path::new(dir).join(label).join(rel);
  let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&shard_path).unwrap()).unwrap();

  // Redact unstable fields for snapshot stability
  if let Some(obj) = v.as_object_mut() {
    obj.insert("sha".into(), serde_json::Value::String("[sha]".into()));
    obj.insert("short_sha".into(), serde_json::Value::String("[short]".into()));
  }
  if let Some(arr) = v.get_mut("parents").and_then(|p| p.as_array_mut()) {
    for p in arr.iter_mut() {
      *p = serde_json::Value::String("[sha]".into());
    }
  }
  if let Some(ts) = v.get_mut("timestamps").and_then(|t| t.as_object_mut()) {
    ts.insert("author".into(), serde_json::Value::Number(0.into()));
    ts.insert("commit".into(), serde_json::Value::Number(0.into()));
    ts.insert("author_local".into(), serde_json::Value::String("[local]".into()));
    ts.insert("commit_local".into(), serde_json::Value::String("[local]".into()));
  }
  if let Some(pr) = v.get_mut("patch_ref").and_then(|o| o.as_object_mut()) {
    pr.insert("git_show_cmd".into(), serde_json::Value::String("[git-show]".into()));
  }

  insta::assert_json_snapshot!(v);
}

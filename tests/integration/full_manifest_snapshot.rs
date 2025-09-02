use test_support;

#[test]
fn full_manifest_snapshot() {
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
      "--include-merges",
      "--include-unmerged",
      "--tz",
      "utc",
      "--now-override",
      "2025-09-01T12:00:00",
    ])
    .output()
    .unwrap();
  assert!(out.status.success());

  let top: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top.get("dir").unwrap().as_str().unwrap();
  let pointer = top
    .get("manifest")
    .and_then(|v| v.as_str())
    .or_else(|| top.get("file").and_then(|v| v.as_str()))
    .expect("pointer file or manifest");
  let path = std::path::Path::new(dir).join(pointer);
  let data = std::fs::read(&path).unwrap();
  let mut v: serde_json::Value = serde_json::from_slice(&data).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
    obj.insert("generated_at".into(), serde_json::Value::String("[generated]".into()));
  }
  insta::assert_json_snapshot!(v, { ".authors" => insta::sorted_redaction() });
}

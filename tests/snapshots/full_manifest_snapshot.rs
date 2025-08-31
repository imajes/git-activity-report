mod common;

use git_activity_report::render::{run_full, FullParams};

#[test]
fn full_manifest_snapshot() {
  let repo = common::fixture_repo();
  let tmpdir = tempfile::TempDir::new().unwrap();

  let params = FullParams {
    repo: repo.path().to_string_lossy().to_string(),
    label: Some("window".into()),
    since: "2025-08-01".into(),
    until: "2025-09-01".into(),
    include_merges: true,
    include_patch: false,
    max_patch_bytes: 0,
    tz_local: false,
    split_out: Some(tmpdir.path().to_string_lossy().to_string()),
    include_unmerged: true,
    save_patches: false,
    github_prs: false,
  };

  let out = run_full(&params).unwrap();
  let dir = out.get("dir").unwrap().as_str().unwrap();
  let manifest = out.get("manifest").unwrap().as_str().unwrap();
  let path = std::path::Path::new(dir).join(manifest);
  let data = std::fs::read(&path).unwrap();
  let mut v: serde_json::Value = serde_json::from_slice(&data).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }

  insta::assert_json_snapshot!(v, {
    ".authors" => insta::sorted_redaction(),
  });
}


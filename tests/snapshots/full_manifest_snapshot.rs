mod common;

use git_activity_report::render::{run_report, ReportParams};

#[test]
fn full_manifest_snapshot() {
  let repo = common::fixture_repo();
  let tmpdir = tempfile::TempDir::new().unwrap();

  let params = ReportParams {
    repo: repo.to_string_lossy().to_string(),
    label: Some("window".into()),
    since: "2025-08-01".into(),
    until: "2025-09-01".into(),
    include_merges: true,
    include_patch: false,
    max_patch_bytes: 0,
    tz_local: false,
    split_apart: true,
    split_out: Some(tmpdir.path().to_string_lossy().to_string()),
    include_unmerged: true,
    save_patches_dir: None,
    github_prs: false,
    now_local: None,
  };

  let out = run_report(&params).unwrap();
  let dir = out.get("dir").unwrap().as_str().unwrap();
  let file = out.get("file").unwrap().as_str().unwrap();
  let path = std::path::Path::new(dir).join(file);
  let data = std::fs::read(&path).unwrap();
  let mut v: serde_json::Value = serde_json::from_slice(&data).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }

  insta::assert_json_snapshot!(v, {
    ".authors" => insta::sorted_redaction(),
  });
}

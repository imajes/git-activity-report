mod common;

use git_activity_report::render::{run_simple, SimpleParams};

#[test]
fn simple_report_snapshot() {
  let repo = common::fixture_repo();
  let params = SimpleParams {
    repo: repo.to_string_lossy().to_string(),
    since: "2025-08-01".into(),
    until: "2025-09-01".into(),
    include_merges: true,
    include_patch: false,
    max_patch_bytes: 0,
    tz_local: false,
    save_patches_dir: None,
    github_prs: false,
  };

  let report = run_simple(&params).unwrap();
  let mut v = serde_json::to_value(&report).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }

  insta::assert_json_snapshot!(v, {
    ".authors" => insta::sorted_redaction(),
  });
}

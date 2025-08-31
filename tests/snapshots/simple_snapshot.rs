mod common;

use git_activity_report::render::{run_report, ReportParams};

#[test]
fn simple_report_snapshot() {
  let repo = common::fixture_repo();
  let params = ReportParams {
    repo: repo.to_string_lossy().to_string(),
    since: "2025-08-01".into(),
    until: "2025-09-01".into(),
    include_merges: true,
    include_patch: false,
    max_patch_bytes: 0,
    tz_local: false,
    split_apart: false,
    split_out: None,
    include_unmerged: false,
    save_patches_dir: None,
    github_prs: false,
    label: Some("window".into()),
    now_local: None,
  };

  let v = run_report(&params).unwrap();
  let mut v: serde_json::Value = v;
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }

  insta::assert_json_snapshot!(v, {
    ".authors" => insta::sorted_redaction(),
  });
}

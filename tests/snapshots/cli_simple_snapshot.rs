mod common;
use assert_cmd::Command;

#[test]
fn cli_simple_snapshot() {
  let repo = common::fixture_repo();
  let repo_path = repo.path().to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      "--simple",
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--tz",
      "utc",
      "--include-merges",
    ])
    .output()
    .unwrap();

  assert!(out.status.success());
  let mut v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }

  insta::assert_json_snapshot!(v, {
    ".authors" => insta::sorted_redaction(),
    ".commits[*].sha" => "[sha]",
    ".commits[*].short_sha" => "[short]",
    ".commits[*].parents[*]" => "[sha]",
    ".commits[*].author.date" => "[date]",
    ".commits[*].committer.date" => "[date]",
    ".commits[*].timestamps.author" => 0,
    ".commits[*].timestamps.commit" => 0,
    ".commits[*].timestamps.author_local" => "[local]",
    ".commits[*].timestamps.commit_local" => "[local]",
    ".commits[*].patch_ref.git_show_cmd" => "[git-show]",
  });
}


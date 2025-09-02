use test_support;

#[test]
fn simple_report_snapshot() {
  test_support::init_tracing();
  test_support::init_insta();
  let _env = test_support::with_env(&[("TZ", "UTC")]);
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let mut cmd = test_support::cmd_bin("git-activity-report");
  let out = cmd
    .args([
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--include-merges",
      "--tz",
      "utc",
      "--now-override",
      "2025-08-15T12:00:00",
    ])
    .output()
    .unwrap();
  assert!(out.status.success());
  let mut v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  if let Some(obj) = v.as_object_mut() {
    obj.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }
  if let Some(summary) = v.get_mut("summary").and_then(|s| s.as_object_mut()) {
    summary.insert("repo".into(), serde_json::Value::String("<repo>".into()));
  }
  // Normalize unstable fields
  if let Some(commits) = v.get_mut("commits").and_then(|c| c.as_array_mut()) {
    for c in commits.iter_mut() {
      if let Some(obj) = c.as_object_mut() {
        obj.insert("sha".into(), serde_json::Value::String("[sha]".into()));
        obj.insert("short_sha".into(), serde_json::Value::String("[short]".into()));
        if let Some(par) = obj.get_mut("parents").and_then(|p| p.as_array_mut()) {
          for p in par.iter_mut() {
            *p = serde_json::Value::String("[sha]".into());
          }
        }
        if let Some(author) = obj.get_mut("author").and_then(|a| a.as_object_mut()) {
          author.insert("date".into(), serde_json::Value::String("[date]".into()));
        }
        if let Some(committer) = obj.get_mut("committer").and_then(|a| a.as_object_mut()) {
          committer.insert("date".into(), serde_json::Value::String("[date]".into()));
        }
        if let Some(ts) = obj.get_mut("timestamps").and_then(|t| t.as_object_mut()) {
          ts.insert("author".into(), serde_json::Value::Number(0.into()));
          ts.insert("commit".into(), serde_json::Value::Number(0.into()));
          ts.insert("author_local".into(), serde_json::Value::String("[local]".into()));
          ts.insert("commit_local".into(), serde_json::Value::String("[local]".into()));
        }
        if let Some(pr) = obj.get_mut("patch_references").and_then(|p| p.as_object_mut()) {
          if let Some(cmd) = pr.get("git_show_cmd").and_then(|v| v.as_str()) {
            let prefix = "git show --patch --format= --no-color ";
            let redacted = if cmd.starts_with(prefix) {
              format!("{}[git-sha]", prefix)
            } else {
              "[git-show]".to_string()
            };
            pr.insert("git_show_cmd".into(), serde_json::Value::String(redacted));
          }
        }
      }
    }
  }

  insta::assert_json_snapshot!(v, { ".authors" => insta::sorted_redaction() });
}

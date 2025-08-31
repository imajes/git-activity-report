use assert_cmd::Command;
mod common;

#[test]
fn errors_when_no_time_selection() {
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.arg("--simple");
  let out = cmd.output().unwrap();
  assert!(!out.status.success());
  let err = String::from_utf8_lossy(&out.stderr);
  assert!(err.contains("Provide one of --month, --for, or (--since AND --until)"));
}

#[test]
fn for_phrase_last_week_simple_smoke() {
  let repo = common::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args(["--simple", "--for", "last week", "--repo", repo_path, "--tz", "utc"]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  assert_eq!(v["mode"], "simple");
}

#[test]
fn month_simple_smoke() {
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args(["--simple", "--month", "2025-08", "--repo", "."]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  assert!(String::from_utf8_lossy(&out.stdout).contains("\"mode\": \"simple\""));
}

#[test]
fn full_mode_warns_out_ignored() {
  let repo = common::init_fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--full",
    "--month",
    "2025-08",
    "--repo",
    repo_path,
    "--out",
    "some.json",
  ]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  let err = String::from_utf8_lossy(&out.stderr);
  assert!(err.contains("--out is ignored in --full mode"));
}

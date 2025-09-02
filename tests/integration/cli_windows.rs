use assert_cmd::Command;
use test_support;

#[test]
fn errors_when_no_time_selection() {
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  // No time selection provided
  let out = cmd.output().unwrap();
  assert!(!out.status.success());
  let err = String::from_utf8_lossy(&out.stderr);
  assert!(err.contains("Provide one of --month, --for, or (--since AND --until)"));
}

#[test]
fn for_phrase_last_week_simple_smoke() {
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args(["--for", "last week", "--repo", repo_path, "--tz", "utc"]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  assert!(v["commits"].is_array());
}

#[test]
fn month_simple_smoke() {
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args(["--month", "2025-08", "--repo", "."]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  assert!(String::from_utf8_lossy(&out.stdout).contains("\"range\""));
}

#[test]
fn full_mode_accepts_out_dir() {
  let repo = test_support::init_fixture_repo();
  let repo_path = repo.path().to_str().unwrap();
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  cmd.args([
    "--split-apart",
    "--month",
    "2025-08",
    "--repo",
    repo_path,
    "--out",
    "tests/.tmp/activity-report",
  ]);
  let out = cmd.output().unwrap();
  assert!(out.status.success());
  // Should not warn about ignoring --out in full mode anymore
  let err = String::from_utf8_lossy(&out.stderr);
  assert!(!err.contains("ignored"));
}

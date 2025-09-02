use assert_cmd::Command;

#[test]
fn cli_generates_man_page() {
  let mut cmd = Command::cargo_bin("git-activity-report").unwrap();
  let out = cmd.args(["--gen-man"]).output().unwrap();
  assert!(out.status.success());
  let s = String::from_utf8_lossy(&out.stdout);
  // clap_mangen emits a roff manpage starting with .TH and mentions the binary name
  assert!(s.contains(".TH") || s.contains(".Nm"));
  assert!(s.contains("git-activity-report"));
}

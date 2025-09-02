use test_support;

#[test]
fn gen_man_outputs_troff() {
  test_support::init_tracing();
  let mut cmd = test_support::cmd_bin("git-activity-report");
  let out = cmd.args(["--gen-man"]).output().unwrap();
  assert!(out.status.success());
  let text = String::from_utf8_lossy(&out.stdout);
  assert!(text.starts_with(".TH"), "expected troff man header");
}


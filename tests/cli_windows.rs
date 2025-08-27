use std::process::Command;
mod common;

#[test]
fn errors_when_no_time_selection() {
    let mut cmd = Command::new(common::bin_path());
    cmd.arg("--simple");
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Provide one of --month, --for, or (--since AND --until)"));
}

#[test]
fn errors_for_for_phrase_unimplemented() {
    let mut cmd = Command::new(common::bin_path());
    cmd.args(["--simple", "--for", "last week", "--repo", "."]);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not implemented"));
}

#[test]
fn month_simple_smoke() {
    let mut cmd = Command::new(common::bin_path());
    cmd.args(["--simple", "--month", "2025-08", "--repo", "."]);
    let out = cmd.output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"mode\": \"simple\""));
}

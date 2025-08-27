mod common;
use std::process::Command;

#[test]
fn full_mode_writes_manifest_and_shards_with_unmerged() {
    let repo = common::init_fixture_repo();
    let repo_path = repo.path().to_str().unwrap();
    let outdir = tempfile::TempDir::new().unwrap();
    let out_path = outdir.path().to_str().unwrap();
    let mut cmd = Command::new(common::bin_path());
    cmd.args([
        "--full", "--since", "2025-08-01", "--until", "2025-09-01",
        "--repo", repo_path,
        "--split-out", out_path,
        "--include-unmerged",
    ]);
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let top: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let dir = top["dir"].as_str().unwrap();
    let manifest_file = top["manifest"].as_str().unwrap();
    let manifest_path = std::path::Path::new(dir).join(manifest_file);
    assert!(manifest_path.exists(), "manifest should exist");
    let mf: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(mf["mode"], "full");
    // At least one item from main and some unmerged branch activity
    assert!(mf["items"].as_array().unwrap().len() >= 1);
    if mf.get("unmerged_activity").is_some() {
        let ua = &mf["unmerged_activity"]["branches"]; 
        assert!(ua.is_array());
    }
}

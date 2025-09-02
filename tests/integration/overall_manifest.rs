use test_support;
use assert_cmd::Command;
use jsonschema::validator_for;

fn compile_overall_schema() -> jsonschema::Validator {
  let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let path = manifest_dir
    .join("tests")
    .join("schemas")
    .join("git-activity-report.overall.schema.json");
  let data = std::fs::read(&path).expect("schema file");
  let v: serde_json::Value = serde_json::from_slice(&data).expect("schema json");
  validator_for(&v).expect("compile schema")
}

#[test]
fn overall_manifest_schema_validates_and_files_exist() {
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      "--split-apart",
      "--for",
      "every month for the last 2 months",
      "--repo",
      repo_path,
      "--out",
      out_path,
    ])
    .output()
    .unwrap();

  assert!(
    out.status.success(),
    "cli run failed: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let top_ptr: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top_ptr["dir"].as_str().expect("dir string");
  let manifest_file = top_ptr["manifest"].as_str().expect("manifest string");
  assert_eq!(manifest_file, "manifest.json");

  let top_json_path = std::path::Path::new(dir).join(manifest_file);
  assert!(top_json_path.exists(), "top manifest should exist");
  let top: serde_json::Value = serde_json::from_slice(&std::fs::read(&top_json_path).unwrap()).unwrap();

  let compiled = compile_overall_schema();
  compiled.validate(&top).expect("top manifest schema validation failed");

  let ranges = top["ranges"].as_array().expect("ranges array");
  assert!(!ranges.is_empty());
  for r in ranges {
    let file = r["file"].as_str().expect("range file");
    let p = std::path::Path::new(dir).join(file);
    assert!(p.exists(), "range file path should exist");
  }
}

#[test]
fn simple_overall_manifest_writes_files() {
  let repo = test_support::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      // not split-apart; still generates an overall manifest for multi-range
      "--for",
      "every week for the last 2 weeks",
      "--repo",
      repo_path,
      "--out",
      out_path,
    ])
    .output()
    .unwrap();

  assert!(
    out.status.success(),
    "cli run failed: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let top_ptr: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top_ptr["dir"].as_str().expect("dir string");
  let manifest_file = top_ptr["manifest"].as_str().expect("manifest string");
  assert_eq!(manifest_file, "manifest.json");

  let top_json_path = std::path::Path::new(dir).join(manifest_file);
  assert!(top_json_path.exists(), "top manifest should exist");
  let top: serde_json::Value = serde_json::from_slice(&std::fs::read(&top_json_path).unwrap()).unwrap();
  let ranges = top["ranges"].as_array().unwrap();
  assert!(!ranges.is_empty());
  for r in ranges {
    let file = r["file"].as_str().expect("range file");
    let p = std::path::Path::new(dir).join(file);
    assert!(p.exists(), "range file path should exist");
  }
}

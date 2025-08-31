mod common;

use assert_cmd::Command;
use jsonschema::validator_for;

fn read_schema(name: &str) -> serde_json::Value {
  let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let path = manifest_dir.join("tests").join("schemas").join(name);
  let data = std::fs::read(&path).expect("schema file");
  serde_json::from_slice(&data).expect("valid schema JSON")
}

fn compile_schema(name: &str) -> jsonschema::Validator {
  let schema = read_schema(name);
  validator_for(&schema).expect("compile schema")
}

#[test]
fn simple_json_conforms_to_schema() {
  let repo = common::fixture_repo();
  let repo_path = repo.to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--include-merges",
    ])
    .output()
    .unwrap();

  assert!(out.status.success());
  let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

  let compiled = compile_schema("git-activity-report.report.schema.json");
  compiled.validate(&v).expect("schema validation failed for simple JSON");
}

#[test]
fn multi_range_overall_and_reports_conform_to_schemas() {
  let repo = common::fixture_repo();
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
      "--include-merges",
      "--include-unmerged",
    ])
    .output()
    .unwrap();

  assert!(out.status.success());
  // Pointer currently printed; load the file and validate against new overall schema
  let top_ptr: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top_ptr["dir"].as_str().unwrap();
  let manifest = top_ptr["manifest"].as_str().unwrap();
  let overall_path = std::path::Path::new(dir).join(manifest);
  let overall: serde_json::Value = serde_json::from_slice(&std::fs::read(&overall_path).unwrap()).unwrap();
  let compiled_overall = compile_schema("git-activity-report.overall.schema.json");
  compiled_overall
    .validate(&overall)
    .expect("overall manifest schema validation failed");

  // Validate each shard against commit schema
  let compiled_commit = compile_schema("git-activity-report.commit.schema.json");
  // For each range entry in the overall manifest, open the referenced file and validate as a report
  let compiled_report = compile_schema("git-activity-report.report.schema.json");
  let ranges = overall["ranges"].as_array().expect("ranges array");
  for r in ranges {
    let file = r["file"].as_str().expect("range file");
    let path = std::path::Path::new(dir).join(file);
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    compiled_report.validate(&report).expect("range report schema");

    // If split-apart, the report should include `items` pointing to commit shards; validate the shards
    if let Some(items) = report.get("items").and_then(|v| v.as_array()) {
      for it in items {
        let rel = it["file"].as_str().unwrap();
        let shard_path = std::path::Path::new(dir).join(rel);
        let shard: serde_json::Value = serde_json::from_slice(&std::fs::read(&shard_path).unwrap()).unwrap();
        compiled_commit.validate(&shard).expect("commit shard schema");
      }
    }
  }
}

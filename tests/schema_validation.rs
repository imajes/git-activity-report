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
      "--simple",
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

  let compiled = compile_schema("git-activity-report.simple.schema.json");
  compiled.validate(&v).expect("schema validation failed for simple JSON");
}

#[test]
fn full_manifest_conforms_to_schema_and_shards_conform() {
  let repo = common::fixture_repo();
  let repo_path = repo.to_str().unwrap();
  let outdir = tempfile::TempDir::new().unwrap();
  let out_path = outdir.path().to_str().unwrap();

  let out = Command::cargo_bin("git-activity-report")
    .unwrap()
    .args([
      "--full",
      "--since",
      "2025-08-01",
      "--until",
      "2025-09-01",
      "--repo",
      repo_path,
      "--split-out",
      out_path,
      "--include-merges",
      "--include-unmerged",
    ])
    .output()
    .unwrap();

  assert!(out.status.success());
  let top: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let dir = top["dir"].as_str().unwrap();
  let manifest = top["manifest"].as_str().unwrap();
  let manifest_path = std::path::Path::new(dir).join(manifest);
  let manifest_json: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

  // Validate manifest against full range schema
  let compiled_range = compile_schema("git-activity-report.full.range.schema.json");
  compiled_range
    .validate(&manifest_json)
    .expect("schema validation failed for full manifest");

  // Validate each shard against commit schema
  let compiled_commit = compile_schema("git-activity-report.commit.schema.json");
  let label = manifest_json.get("label").and_then(|v| v.as_str()).unwrap_or("");

  // Helper closure to validate a shard from a relative path
  let validate_shard = |rel: &str| {
    let shard_path1 = std::path::Path::new(dir).join(label).join(rel);
    let shard_path2 = std::path::Path::new(dir).join(rel);
    let shard_path = if shard_path1.exists() { shard_path1 } else { shard_path2 };
    let content = std::fs::read(&shard_path).expect("shard read");
    let v: serde_json::Value = serde_json::from_slice(&content).expect("shard json");
    compiled_commit
      .validate(&v)
      .expect("schema validation failed for shard");
  };

  for it in manifest_json["items"].as_array().unwrap() {
    let rel = it["file"].as_str().unwrap();
    validate_shard(rel);
  }

  if let Some(ua) = manifest_json.get("unmerged_activity") {
    if let Some(branches) = ua.get("branches").and_then(|v| v.as_array()) {
      for b in branches {
        if let Some(items) = b.get("items").and_then(|v| v.as_array()) {
          for it in items {
            let rel = it["file"].as_str().unwrap();
            validate_shard(rel);
          }
        }
      }
    }
  }
}

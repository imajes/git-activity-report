// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Build and write overall manifest for multi-range runs
// role: persistence/manifest
// inputs: repo id, generated_at, flags snapshot, base_dir, RangeEntry[]
// outputs: manifest.json file written under base_dir
// side_effects: Writes to filesystem
// invariants:
// - manifest contains ranges[] in chronological order of entries provided
// - file paths in entries are relative to base_dir and point to report-<label>.json
// - generated_at is serialized in %Y-%m-%dT%H:%M:%S (local)
// errors: IO errors surfaced with full path context
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use anyhow::Result;
use chrono::{DateTime, Local};

/// Helper to build and write the overall/top manifest for multi-bucket runs.
pub struct OverallManifest {
  value: serde_json::Value,
}

impl OverallManifest {
  pub fn new(
    repo: &str,
    generated_at: DateTime<Local>,
    split_apart: bool,
    include_merges: bool,
    include_patch: bool,
    include_unmerged: bool,
  ) -> Self {
    let mut v = serde_json::json!({
      "repo": repo,
      "generated_at": generated_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
      "split_apart": split_apart,
      "include_merges": include_merges,
      "include_patch": include_patch,
      "include_unmerged": include_unmerged,
      "ranges": [],
    });
    // ensure ranges is an array
    let _ = v["ranges"].as_array_mut().expect("ranges array");
    Self { value: v }
  }

  pub fn push_simple_entry(&mut self, label: String, start: String, end: String, file_path: &str) {
    let entry = serde_json::json!({
      "label": label,
      "range": {"start": start, "end": end},
      "file": file_path,
    });
    self.value["ranges"].as_array_mut().unwrap().push(entry);
  }

  pub fn write_to(&self, base_dir: &str) -> Result<std::path::PathBuf> {
    let path = std::path::Path::new(base_dir).join("manifest.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&self.value)?)?;
    Ok(path)
  }

  #[allow(dead_code)]
  pub fn as_value(&self) -> &serde_json::Value {
    &self.value
  }
}

pub struct RangeEntry {
  pub label: String,
  pub start: String,
  pub end: String,
  pub file: String,
}

/// Build and write an overall manifest given pre-computed entries.
#[allow(clippy::too_many_arguments)]
pub fn write_overall_manifest(
  repo: &str,
  generated_at: DateTime<Local>,
  split_apart: bool,
  include_merges: bool,
  include_patch: bool,
  include_unmerged: bool,
  base_dir: &str,
  entries: &[RangeEntry],
) -> Result<std::path::PathBuf> {
  let mut overall = OverallManifest::new(
    repo,
    generated_at,
    split_apart,
    include_merges,
    include_patch,
    include_unmerged,
  );
  for e in entries {
    overall.push_simple_entry(e.label.clone(), e.start.clone(), e.end.clone(), &e.file);
  }
  overall.write_to(base_dir)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn write_overall_manifest_writes_file_and_entries() {
    let td = tempfile::TempDir::new().unwrap();
    let base = td.path().to_string_lossy().to_string();
    let gen_at = chrono::NaiveDateTime::parse_from_str("2025-08-15T12:00:00", "%Y-%m-%dT%H:%M:%S")
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();
    let entries = vec![
      RangeEntry {
        label: "2025-07".into(),
        start: "2025-07-01T00:00:00".into(),
        end: "2025-08-01T00:00:00".into(),
        file: "report-2025-07.json".into(),
      },
      RangeEntry {
        label: "2025-08".into(),
        start: "2025-08-01T00:00:00".into(),
        end: "2025-09-01T00:00:00".into(),
        file: "report-2025-08.json".into(),
      },
    ];
    let path =
      write_overall_manifest("<repo>", gen_at, true, true, false, false, &base, &entries).expect("write manifest");
    assert!(path.ends_with("manifest.json"));
    let buf = std::fs::read(path).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(v["repo"].as_str().unwrap(), "<repo>");
    let ranges = v["ranges"].as_array().unwrap();
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0]["file"].as_str().unwrap(), "report-2025-07.json");
    assert_eq!(ranges[1]["file"].as_str().unwrap(), "report-2025-08.json");
  }
}

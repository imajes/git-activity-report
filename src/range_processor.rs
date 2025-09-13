// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Orchestrate per-range processing: generate report JSON and save artifacts; assemble overall manifest for multi-range runs
// role: processing/orchestrator
// inputs: EffectiveConfig (with split_apart and multi_windows), Vec<LabeledRange>, optional now
// outputs: Files on disk (reports, shards), optional manifest.json; stdout pointer or JSON per state
// side_effects: Creates directories; writes JSON files; prints to stdout
// invariants:
// - base_dir is prepared when split_apart || multi_windows
// - per-range report file name is report-<label>.json when written to disk
// - multi_windows ⇒ manifest.json exists and pointer {dir, manifest} printed
// - single split ⇒ pointer {dir, file} printed; single non-split ⇒ JSON printed or written to --out
// errors: Propagates generation/save/write errors with file path context
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use anyhow::Result;

use crate::cli;
use crate::manifest::{RangeEntry, write_overall_manifest};
use crate::range_windows::LabeledRange;
use crate::render::build_report_params;
use crate::render::run_report;
use crate::util;

fn commit_count(report: &serde_json::Value) -> u64 {
  report
    .get("summary")
    .and_then(|s| s.get("count"))
    .and_then(|v| v.as_u64())
    .unwrap_or(0)
}

fn write_pretty_json<P: AsRef<std::path::Path>>(path: P, v: &serde_json::Value) -> anyhow::Result<()> {
  std::fs::write(path.as_ref(), serde_json::to_vec_pretty(v)?)?;

  Ok(())
}

/// Outcome of saving a per-range report.
///
/// - `entry`: manifest entry when `multi_windows` is set.
/// - `to_print`: JSON to print to stdout for single runs, or pointer when single split.
pub struct SaveOutcome {
  pub entry: Option<RangeEntry>,
  pub to_print: Option<serde_json::Value>,
}

/// Resolve the relative report filename for a range, depending on mode.
///
/// - When `split_apart` is true, the renderer returns a JSON pointer with a `file` we reuse.
/// - When `multi_windows` (non-split) is active, standardize to `report-<label>.json`.
/// - For a single non-split run, there is no relative file (we print JSON or write to `--out`).
fn resolve_file_rel(
  report_json: &serde_json::Value,
  cfg: &cli::EffectiveConfig,
  range: &LabeledRange,
  base_dir_opt: Option<&str>,
) -> Option<String> {
  if cfg.split_apart {
    let file_json = report_json.get("file");
    let file_rel = file_json.and_then(|v| v.as_str());

    return file_rel.map(|s| s.to_string());
  }

  if base_dir_opt.is_some() {
    let file_rel = format!("report-{}.json", range.label);

    return Some(file_rel);
  }

  None
}

/// Write report to `--out` (file or dir) or return it for stdout when appropriate.
///
/// Returns `Some(report_json)` when the caller should print; `None` when written to disk.
fn write_or_print(
  out_path_or_dir: &str,
  report_json: serde_json::Value,
  label: &str,
) -> anyhow::Result<Option<serde_json::Value>> {
  if out_path_or_dir == "-" {
    return Ok(Some(report_json));
  }

  let out_path = std::path::Path::new(out_path_or_dir);
  let is_dir_like = out_path_or_dir.ends_with('/') || out_path.is_dir();

  if is_dir_like {
    std::fs::create_dir_all(out_path)?;

    let file_path = out_path.join(format!("report-{}.json", label));
    let count = commit_count(&report_json);

    if count == 0 {
      return Ok(Some(report_json));
    }

    write_pretty_json(&file_path, &report_json)?;

    return Ok(None);
  }

  if let Some(parent) = out_path.parent() {
    std::fs::create_dir_all(parent)?;
  }

  let count = commit_count(&report_json);

  if count == 0 {
    return Ok(Some(report_json));
  }

  write_pretty_json(out_path, &report_json)?;

  Ok(None)
}

pub fn generate_range_report(
  cfg: &cli::EffectiveConfig,
  range: &LabeledRange,
  now_opt: Option<chrono::DateTime<chrono::Local>>,
  base_dir_opt: Option<&str>,
) -> Result<serde_json::Value> {
  let mut params = build_report_params(cfg, range.since.clone(), range.until.clone());
  params.label = Some(range.label.clone());
  params.now_local = now_opt;
  params.split_apart = cfg.split_apart;
  if cfg.split_apart {
    if let Some(dir) = base_dir_opt {
      params.split_out = Some(dir.to_string());
    } else {
      let base_dir = util::prepare_out_dir(&cfg.out, now_opt)?;
      params.split_out = Some(base_dir);
    }
  }
  run_report(&params)
}

pub fn save_range_report(
  cfg: &cli::EffectiveConfig,
  range: &LabeledRange,
  report: serde_json::Value,
  base_dir_opt: Option<&str>,
) -> Result<SaveOutcome> {
  let file_rel = resolve_file_rel(&report, cfg, range, base_dir_opt);

  let mut print_json: Option<serde_json::Value> = None;

  if !cfg.split_apart {
    if let Some(base_dir) = base_dir_opt {
      let file_name = file_rel.as_ref().expect("file name for multi");
      let file_path = std::path::Path::new(base_dir).join(file_name);

      write_pretty_json(&file_path, &report)?;
    } else {
      print_json = write_or_print(&cfg.out, report, &range.label)?;
    }
  } else if !cfg.multi_windows {
    print_json = Some(report);
  }

  let entry = if cfg.multi_windows {
    Some(RangeEntry {
      label: range.label.clone(),
      start: range.since.clone(),
      end: range.until.clone(),
      file: file_rel.expect("file name for multi"),
    })
  } else {
    None
  };

  let outcome = SaveOutcome {
    entry,
    to_print: print_json,
  };

  Ok(outcome)
}

pub fn process_ranges(
  cfg: &cli::EffectiveConfig,
  ranges: Vec<LabeledRange>,
  now_opt: Option<chrono::DateTime<chrono::Local>>,
) -> Result<()> {
  let base_dir_opt = if cfg.split_apart || cfg.multi_windows {
    Some(util::prepare_out_dir(&cfg.out, now_opt)?)
  } else {
    None
  };

  let mut entries: Vec<RangeEntry> = Vec::new();
  let mut last_single_output: Option<serde_json::Value> = None;

  for r in ranges.iter() {
    let out = generate_range_report(cfg, r, now_opt, base_dir_opt.as_deref())?;
    let outcome = save_range_report(cfg, r, out, base_dir_opt.as_deref())?;

    if let Some(e) = outcome.entry {
      entries.push(e);
    }

    if let Some(v) = outcome.to_print {
      last_single_output = Some(v);
    }
  }

  if cfg.multi_windows {
    let base_dir = base_dir_opt.as_deref().expect("base_dir for multi");
    let _manifest_path = write_overall_manifest(
      &cfg.repo,
      util::effective_now(now_opt),
      cfg.split_apart,
      cfg.include_merges,
      cfg.include_patch,
      cfg.include_unmerged,
      base_dir,
      &entries,
    )?;
    println!(
      "{}",
      serde_json::to_string_pretty(&serde_json::json!({"dir": base_dir, "manifest": "manifest.json"}))?
    );

    return Ok(());
  }

  if let Some(v) = last_single_output {
    println!("{}", serde_json::to_string_pretty(&v)?);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cli::EffectiveConfig;
  use crate::range_windows::WindowSpec;

  fn base_cfg(repo: String) -> EffectiveConfig {
    EffectiveConfig {
      repo,
      window: WindowSpec::SinceUntil {
        since: "1970-01-01".into(),
        until: "2100-01-01".into(),
      },
      multi_windows: false,
      split_apart: false,
      include_merges: true,
      include_patch: false,
      max_patch_bytes: 0,
      save_patches: None,
      out: "-".into(),
      github_prs: false,
      include_unmerged: false,
      tz: "utc".into(),
      now_override: None,
      estimate_effort: false,
    }
  }

  fn fixture_repo() -> String {
    if let Ok(dir) = std::env::var("GAR_FIXTURE_REPO_DIR") {
      return dir;
    }
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/.tmp/tmpdir");
    std::fs::read_to_string(p).expect("fixture path").trim().to_string()
  }

  #[test]
  fn generate_and_save_single_non_split_stdout() {
    let repo = fixture_repo();
    let mut cfg = base_cfg(repo);
    cfg.split_apart = false;
    cfg.multi_windows = false;
    let range = LabeledRange {
      label: "window".into(),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
    };

    let out = generate_range_report(&cfg, &range, None, None).expect("gen");
    let outcome = save_range_report(&cfg, &range, out, None).expect("save");
    assert!(outcome.to_print.is_some());
  }

  #[test]
  fn generate_and_save_single_split_pointer() {
    let repo = fixture_repo();
    let mut cfg = base_cfg(repo);
    cfg.split_apart = true;
    cfg.multi_windows = false;
    let td = tempfile::TempDir::new().unwrap();
    cfg.out = td.path().to_string_lossy().to_string();
    let range = LabeledRange {
      label: "window".into(),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
    };
    let out = generate_range_report(&cfg, &range, None, Some(&cfg.out)).expect("gen");
    let outcome = save_range_report(&cfg, &range, out.clone(), Some(&cfg.out)).expect("save");
    assert!(outcome.entry.is_none(), "single split should not create manifest entry");
    assert!(
      outcome.to_print.is_some(),
      "single split should return pointer to print"
    );
    let file = out.get("file").and_then(|v| v.as_str()).unwrap();
    assert!(std::path::Path::new(&cfg.out).join(file).exists());
  }

  #[test]
  fn generate_and_save_multi_non_split_writes_file_and_entry() {
    let repo = fixture_repo();
    let mut cfg = base_cfg(repo);
    cfg.split_apart = false;
    cfg.multi_windows = true;
    let td = tempfile::TempDir::new().unwrap();
    cfg.out = td.path().to_string_lossy().to_string();
    let range = LabeledRange {
      label: "2025-08".into(),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
    };
    let out = generate_range_report(&cfg, &range, None, Some(&cfg.out)).expect("gen");
    let outcome = save_range_report(&cfg, &range, out, Some(&cfg.out)).expect("save");
    assert!(outcome.to_print.is_none());
    let e = outcome.entry.expect("entry");
    assert_eq!(e.file, "report-2025-08.json");
    assert!(std::path::Path::new(&cfg.out).join(&e.file).exists());
  }
}

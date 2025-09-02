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
use crate::params::build_report_params;
use crate::range_windows::LabeledRange;
use crate::render::run_report;
use crate::util;

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
) -> Result<(Option<RangeEntry>, Option<serde_json::Value>)> {
  let file_rel = if cfg.split_apart {
    report.get("file").and_then(|v| v.as_str()).map(|s| s.to_string())
  } else if base_dir_opt.is_some() {
    Some(format!("report-{}.json", range.label))
  } else {
    None
  };

  let mut print_json: Option<serde_json::Value> = None;
  if !cfg.split_apart {
    if let Some(base_dir) = base_dir_opt {
      let file_path = std::path::Path::new(base_dir).join(file_rel.as_ref().expect("file name for multi"));
      std::fs::write(&file_path, serde_json::to_vec_pretty(&report)?)?;
    } else if cfg.out != "-" {
      let out_path = std::path::Path::new(&cfg.out);
      let is_dir_like = cfg.out.ends_with('/') || out_path.is_dir();
      if is_dir_like {
        let label = &range.label;
        std::fs::create_dir_all(out_path)?;
        let file_path = out_path.join(format!("report-{}.json", label));
        // If count==0, do not write a file; print JSON instead
        let count = report
          .get("summary")
          .and_then(|s| s.get("count"))
          .and_then(|v| v.as_u64())
          .unwrap_or(0);
        if count == 0 {
          print_json = Some(report);
        } else {
          std::fs::write(&file_path, serde_json::to_vec_pretty(&report)?)?;
        }
      } else {
        if let Some(parent) = out_path.parent() {
          std::fs::create_dir_all(parent)?;
        }
        let count = report
          .get("summary")
          .and_then(|s| s.get("count"))
          .and_then(|v| v.as_u64())
          .unwrap_or(0);
        if count == 0 {
          print_json = Some(report);
        } else {
          std::fs::write(out_path, serde_json::to_vec_pretty(&report)?)?;
        }
      }
    } else {
      print_json = Some(report);
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
  Ok((entry, print_json))
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
    let (entry, to_print) = save_range_report(cfg, r, out, base_dir_opt.as_deref())?;
    if let Some(e) = entry {
      entries.push(e);
    }
    if let Some(v) = to_print {
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
    let (_entry, print) = save_range_report(&cfg, &range, out, None).expect("save");
    assert!(print.is_some());
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
    let (entry, print) = save_range_report(&cfg, &range, out.clone(), Some(&cfg.out)).expect("save");
    assert!(entry.is_none(), "single split should not create manifest entry");
    assert!(print.is_some(), "single split should return pointer to print");
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
    let (entry, print) = save_range_report(&cfg, &range, out, Some(&cfg.out)).expect("save");
    assert!(print.is_none());
    let e = entry.expect("entry");
    assert_eq!(e.file, "report-2025-08.json");
    assert!(std::path::Path::new(&cfg.out).join(&e.file).exists());
  }
}

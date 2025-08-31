use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod enrich;
mod ext;
mod gitio;
mod model;
mod render;
mod util;
mod window;

use crate::window::{Tz, WindowSpec};
use chrono::Local;

/// CLI entry — parses flags, normalizes config, and (for now) prints the
/// normalized configuration as JSON. This compiles cleanly and is ready
/// to have git IO, sharding, and enrichment wired in.
#[derive(Parser, Debug)]
#[command(
    name = "git-activity-report",
    version,
    about = "Export Git activity to JSON (simple or sharded)",
    long_about = None
)]
struct Cli {
  /// Path to a Git repository (default: current dir)
  #[arg(long, default_value = ".")]
  repo: PathBuf,

  /// Calendar month, e.g. 2025-08
  #[arg(long)]
  month: Option<String>,

  /// Natural language window, e.g. "last week" or "every month for the last 6 months"
  #[arg(long = "for")]
  for_str: Option<String>,

  /// Custom since (Git approxidate ok); must be paired with --until
  #[arg(long)]
  since: Option<String>,

  /// Custom until (exclusive); must be paired with --since
  #[arg(long)]
  until: Option<String>,

  /// Single JSON output (quick)
  #[arg(long)]
  simple: bool,

  /// Sharded output (per-commit files + manifest)
  #[arg(long)]
  full: bool,

  /// Include merge commits
  #[arg(long)]
  include_merges: bool,

  /// Embed unified patches in JSON (big)
  #[arg(long)]
  include_patch: bool,

  /// Per-commit patch cap (0 = no limit)
  #[arg(long, default_value_t = 0)]
  max_patch_bytes: usize,

  /// Directory to write .patch files (referenced in JSON)
  #[arg(long)]
  save_patches: Option<PathBuf>,

  /// Base directory for sharded output (full mode). If omitted, auto-named.
  #[arg(long)]
  split_out: Option<PathBuf>,

  /// File for --simple (default stdout "-")
  #[arg(long, default_value = "-")]
  out: String,

  /// Try to enrich with GitHub PRs (quietly ignored if not available)
  #[arg(long)]
  github_prs: bool,

  /// Scan local branches for commits in the window not reachable from HEAD; include separately.
  #[arg(long)]
  include_unmerged: bool,

  /// Timezone for local ISO timestamps in output (label only)
  #[arg(long, value_enum, default_value_t = Tz::Local)]
  tz: Tz,

  /// Emit a troff man page to stdout (internal; for packaging)
  #[arg(long, hide = true)]
  gen_man: bool,

  /// Override the "now" instant for natural-language parsing (hidden; tests only)
  #[arg(long = "now-override", hide = true)]
  now_override: Option<String>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Mode {
  Simple,
  Full,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EffectiveConfig {
  repo: String, // absolute path for stability
  window: WindowSpec,
  mode: Mode,
  include_merges: bool,
  include_patch: bool,
  max_patch_bytes: usize,
  save_patches: Option<String>,
  split_out: Option<String>,
  out: String,
  github_prs: bool,
  include_unmerged: bool,
  pub tz: Tz,
  now_override: Option<String>,
}

fn normalize(cli: Cli) -> Result<EffectiveConfig> {
  // Validate window selection
  let window = match (&cli.month, &cli.for_str, &cli.since, &cli.until) {
    (Some(ym), None, None, None) => WindowSpec::Month { ym: ym.clone() },
    (None, Some(p), None, None) => WindowSpec::ForPhrase { phrase: p.clone() },
    (None, None, Some(s), Some(u)) => WindowSpec::SinceUntil {
      since: s.clone(),
      until: u.clone(),
    },
    (None, None, None, None) => {
      bail!("Provide one of --month, --for, or (--since AND --until)")
    }
    // Invalid mixes fall through
    _ => bail!("Ambiguous time selection: choose only one of --month | --for | --since/--until"),
  };

  // Mode selection (default simple)
  let mode = match (cli.simple, cli.full) {
    (true, true) => bail!("Use --simple OR --full, not both"),
    (true, false) => Mode::Simple,
    (false, true) => Mode::Full,
    (false, false) => Mode::Simple,
  };

  // Gentle note if --out is provided in full mode
  if matches!(mode, Mode::Full) && cli.out != "-" {
    eprintln!("note: --out is ignored in --full mode (writing shards + manifest)");
  }

  let repo = util::canonicalize_lossy(&cli.repo);

  Ok(EffectiveConfig {
    repo,
    window,
    mode,
    include_merges: cli.include_merges,
    include_patch: cli.include_patch,
    max_patch_bytes: cli.max_patch_bytes,
    save_patches: cli.save_patches.as_deref().map(util::canonicalize_lossy),
    split_out: cli.split_out.as_deref().map(util::canonicalize_lossy),
    out: cli.out,
    github_prs: cli.github_prs,
    include_unmerged: cli.include_unmerged,
    tz: cli.tz,
    now_override: cli.now_override.clone(),
  })
}

pub(crate) fn run_with_cli(cli: Cli) -> Result<()> {
  let cfg = normalize(cli).context("validating CLI flags")?;

  // If --for is a multi-bucket phrase, handle top-manifest orchestration
  if let WindowSpec::ForPhrase { phrase } = &cfg.window {
    let now_opt = parse_now_override(&cfg);
    if let Some(buckets) = window::for_phrase_buckets(phrase, now_opt) {
      let is_full = matches!(cfg.mode, Mode::Full);
      // Choose top directory
      let base_dir = if let Some(dir) = &cfg.split_out { dir.clone() } else {
        let tmp = std::env::temp_dir();
        tmp
          .join(format!("activity-{}", Local::now().format("%Y%m%d-%H%M%S")))
          .to_string_lossy()
          .to_string()
      };
      std::fs::create_dir_all(&base_dir)?;

      // Assemble top manifest structure incrementally
      let mut top = serde_json::json!({
        "repo": cfg.repo,
        "multi": true,
        "generated_at": Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "mode": if is_full { "full" } else { "simple" },
        "include_merges": cfg.include_merges,
        "include_patch": cfg.include_patch,
        "include_unmerged": cfg.include_unmerged,
        "buckets": [],
      });

      if is_full {
        for b in buckets {
          let params = build_full_params(&cfg, b.since.clone(), b.until.clone());
          // Override label and split_out to keep all ranges under the same top dir
          let params = render::FullParams { split_out: Some(base_dir.clone()), label: Some(b.label.clone()), ..params };
          let result = render::run_full(&params)?;
          let entry = serde_json::json!({
            "label": b.label,
            "range": {"since": b.since, "until": b.until},
            "manifest": result.get("manifest").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            "dir": result.get("dir").and_then(|v| v.as_str()).unwrap_or(&base_dir).to_string(),
          });
          top["buckets"].as_array_mut().unwrap().push(entry);
        }
      } else {
        for b in buckets {
          let params = build_simple_params(&cfg, b.since.clone(), b.until.clone());
          let report = render::run_simple(&params)?;
          let file_path = std::path::Path::new(&base_dir).join(format!("{}.json", b.label));
          std::fs::write(&file_path, serde_json::to_vec_pretty(&report)?)?;
          let entry = serde_json::json!({
            "label": b.label,
            "range": {"since": b.since, "until": b.until},
            "file": file_path.to_string_lossy().to_string(),
          });
          top["buckets"].as_array_mut().unwrap().push(entry);
        }
      }

      // Write top manifest and print pointer
      let top_manifest_path = std::path::Path::new(&base_dir).join("manifest.json");
      std::fs::write(&top_manifest_path, serde_json::to_vec_pretty(&top)?)?;
      println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "dir": base_dir,
        "manifest": "manifest.json"
      }))?);

      return Ok(());
    }
  }

  // Shared: compute window bounds once
  let (since, until) = window::compute_window_strings(&cfg.window, parse_now_override(&cfg))?;

  // Generate the report JSON value in the match
  let (json, allow_file_output) = match cfg.mode {
    Mode::Simple => {
      let params = build_simple_params(&cfg, since.clone(), until.clone());
      let report = render::run_simple(&params)?;
      (serde_json::to_value(&report)?, true)
    }
    Mode::Full => {
      let params = build_full_params(&cfg, since.clone(), until.clone());
      let res = render::run_full(&params)?;
      (res, false) // file output is ignored in full mode
    }
  };

  // Shared: final output step
  if allow_file_output && cfg.out != "-" {
    std::fs::write(&cfg.out, serde_json::to_vec_pretty(&json)?)?;
  } else {
    println!("{}", serde_json::to_string_pretty(&json)?);
  }

  Ok(())
}

fn parse_now_override(cfg: &EffectiveConfig) -> Option<chrono::DateTime<chrono::Local>> {
  cfg
    .now_override
    .as_ref()
    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&chrono::Local)))
    .or_else(|| {
      cfg
        .now_override
        .as_ref()
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok())
        .and_then(|ndt| ndt.and_local_timezone(chrono::Local).single())
    })
}

fn build_simple_params(cfg: &EffectiveConfig, since: String, until: String) -> render::SimpleParams {
  render::SimpleParams {
    repo: cfg.repo.clone(),
    since,
    until,
    include_merges: cfg.include_merges,
    include_patch: cfg.include_patch,
    max_patch_bytes: cfg.max_patch_bytes,
    tz_local: matches!(cfg.tz, Tz::Local),
    save_patches_dir: cfg.save_patches.clone(),
    github_prs: cfg.github_prs,
  }
}

fn build_full_params(cfg: &EffectiveConfig, since: String, until: String) -> render::FullParams {
  let label = match &cfg.window {
    WindowSpec::Month { ym } => Some(ym.clone()),
    _ => Some("window".into()),
  };
  render::FullParams {
    repo: cfg.repo.clone(),
    label,
    since,
    until,
    include_merges: cfg.include_merges,
    include_patch: cfg.include_patch,
    max_patch_bytes: cfg.max_patch_bytes,
    tz_local: matches!(cfg.tz, Tz::Local),
    split_out: cfg.split_out.clone(),
    include_unmerged: cfg.include_unmerged,
    save_patches: cfg.save_patches.is_some(),
    github_prs: cfg.github_prs,
  }
}

fn main() -> Result<()> {
  let cli = Cli::parse();

  if cli.gen_man {
    let cmd = Cli::command();
    // Render a section-1 man page
    let man = clap_mangen::Man::new(cmd);
    let mut buf: Vec<u8> = Vec::new();
    man.render(&mut buf).expect("render manpage");
    print!("{}", String::from_utf8_lossy(&buf));
    return Ok(());
  }

  run_with_cli(cli)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  fn base_cli() -> Cli {
    Cli {
      repo: PathBuf::from("."),
      month: None,
      for_str: None,
      since: None,
      until: None,
      simple: false,
      full: false,
      include_merges: false,
      include_patch: false,
      max_patch_bytes: 0,
      save_patches: None,
      split_out: None,
      out: "-".into(),
      github_prs: false,
      include_unmerged: false,
      tz: Tz::Utc,
      gen_man: false,
      now_override: None,
    }
  }

  #[test]
  fn normalize_month_defaults_to_simple() {
    let mut cli = base_cli();
    cli.month = Some("2025-08".into());
    let cfg = normalize(cli).unwrap();
    assert!(matches!(cfg.mode, Mode::Simple));
    // Ensure window preserved from CLI; avoid testing window logic here
    match cfg.window {
      WindowSpec::Month { ref ym } => assert_eq!(ym, "2025-08"),
      _ => panic!("expected Month window"),
    }
  }

  #[test]
  fn normalize_conflicting_modes_errors() {
    let mut cli = base_cli();
    cli.month = Some("2025-08".into());
    cli.simple = true;
    cli.full = true;
    assert!(normalize(cli).is_err());
  }

  // Integration tests under tests/ cover run_with_cli end-to-end
}

use anyhow::{Result, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::util;
use crate::range_windows::{Tz, WindowSpec};

#[derive(Parser, Debug)]
#[command(
    name = "git-activity-report",
    version,
    about = "Export Git activity to JSON (simple or sharded)",
    long_about = None
)]
pub struct Cli {
  /// Path to a Git repository (default: current dir)
  #[arg(long, default_value = ".")]
  pub repo: PathBuf,

  /// Calendar month, e.g. 2025-08
  #[arg(long)]
  pub month: Option<String>,

  /// Natural language window, e.g. "last week" or "every month for the last 6 months"
  #[arg(long = "for")]
  pub for_str: Option<String>,

  /// Custom since (Git approxidate ok); must be paired with --until
  #[arg(long, alias = "start")]
  pub since: Option<String>,

  /// Custom until (exclusive); must be paired with --since
  #[arg(long, alias = "end")]
  pub until: Option<String>,

  /// Split output into multiple files (per-commit shards) and include an items index in the report.
  #[arg(long)]
  pub split_apart: bool,

  /// Convenience: turn on all enrichment/detail flags (unmerged, github, patches, etc.).
  #[arg(long)]
  pub detailed: bool,

  /// Include merge commits
  #[arg(long)]
  pub include_merges: bool,

  /// Embed unified patches in JSON (big)
  #[arg(long)]
  pub include_patch: bool,

  /// Per-commit patch cap (0 = no limit)
  #[arg(long, default_value_t = 0)]
  pub max_patch_bytes: usize,

  /// Directory to write .patch files (referenced in JSON)
  #[arg(long)]
  pub save_patches: Option<PathBuf>,

  /// Output location:
  /// - without `--split-apart` (single report): file path (default stdout "-")
  /// - with `--split-apart` or multi-range runs: base directory (default: auto-named temp dir)
  #[arg(long, default_value = "-")]
  pub out: String,

  /// Try to enrich with GitHub PRs (quietly ignored if not available)
  #[arg(long)]
  pub github_prs: bool,

  /// Scan local branches for commits in the window not reachable from HEAD; include separately.
  #[arg(long)]
  pub include_unmerged: bool,

  /// Timezone for local ISO timestamps in output (label only)
  #[arg(long, value_enum, default_value_t = Tz::Local)]
  pub tz: Tz,

  /// Emit a troff man page to stdout (internal; for packaging)
  #[arg(long, hide = true)]
  pub gen_man: bool,

  /// Override the "now" instant for natural-language parsing (hidden; tests only)
  #[arg(long = "now-override", hide = true)]
  pub now_override: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EffectiveConfig {
  pub repo: String, // absolute path for stability
  pub window: WindowSpec,
  pub multi_windows: bool,
  pub split_apart: bool,
  pub include_merges: bool,
  pub include_patch: bool,
  pub max_patch_bytes: usize,
  pub save_patches: Option<String>,
  pub out: String,
  pub github_prs: bool,
  pub include_unmerged: bool,
  pub tz: Tz,
  pub now_override: Option<String>,
}

pub fn normalize(cli: Cli) -> Result<EffectiveConfig> {
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
    _ => bail!("Ambiguous time selection: choose only one of --month | --for | --since/--until"),
  };

  // Determine split_apart behavior (no back-compat flags kept)
  let split_apart = cli.split_apart;

  // Determine effective detail flags
  let include_unmerged = cli.include_unmerged || cli.detailed;
  let include_patch = cli.include_patch || cli.detailed;
  let github_prs = cli.github_prs || cli.detailed;

  let repo = util::canonicalize_lossy(&cli.repo);

  Ok(EffectiveConfig {
    repo,
    window,
    multi_windows: false, // NOTE: set as default but can be overriden
    split_apart,
    include_merges: cli.include_merges,
    include_patch,
    max_patch_bytes: cli.max_patch_bytes,
    save_patches: cli.save_patches.as_deref().map(util::canonicalize_lossy),
    out: cli.out,
    github_prs,
    include_unmerged,
    tz: cli.tz,
    now_override: cli.now_override.clone(),
  })
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
      split_apart: false,
      detailed: false,
      include_merges: false,
      include_patch: false,
      max_patch_bytes: 0,
      save_patches: None,
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
    assert_eq!(cfg.split_apart, false);
    match cfg.window {
      WindowSpec::Month { ref ym } => assert_eq!(ym, "2025-08"),
      _ => panic!("expected Month window"),
    }
  }

  #[test]
  fn detailed_implies_other_flags() {
    let mut cli = base_cli();
    cli.month = Some("2025-08".into());
    cli.detailed = true;
    let cfg = normalize(cli).unwrap();
    assert!(cfg.include_unmerged);
    assert!(cfg.include_patch);
    assert!(cfg.github_prs);
  }
}

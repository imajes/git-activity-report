use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod util;
mod model;
mod gitio;
mod render;

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
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Tz { Local, Utc }

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Mode { Simple, Full }

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WindowSpec {
    Month { ym: String },
    ForPhrase { phrase: String },
    SinceUntil { since: String, until: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EffectiveConfig {
    repo: String,           // absolute path for stability
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
}

fn month_bounds(ym: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = ym.split('-').collect();
    if parts.len() != 2 { bail!("invalid --month, expected YYYY-MM"); }
    let y: i32 = parts[0].parse().context("parsing year in --month")?;
    let m: i32 = parts[1].parse().context("parsing month in --month")?;
    if m < 1 || m > 12 { bail!("invalid month in --month"); }
    let next_y = if m == 12 { y + 1 } else { y };
    let next_m = if m == 12 { 1 } else { m + 1 };
    Ok((format!("{y:04}-{m:02}-01T00:00:00"), format!("{next_y:04}-{next_m:02}-01T00:00:00")))
}

pub fn compute_window_strings(cfg: &EffectiveConfig) -> Result<(String, String)> {
    match &cfg.window {
        WindowSpec::SinceUntil { since, until } => Ok((since.clone(), until.clone())),
        WindowSpec::Month { ym } => month_bounds(ym),
        WindowSpec::ForPhrase { .. } => bail!("--for phrase windows not implemented in Rust port yet; use --month or --since/--until"),
    }
}

fn normalize(cli: Cli) -> Result<EffectiveConfig> {
    // Validate window selection
    let window = match (&cli.month, &cli.for_str, &cli.since, &cli.until) {
        (Some(ym), None, None, None) => WindowSpec::Month { ym: ym.clone() },
        (None, Some(p), None, None) => WindowSpec::ForPhrase { phrase: p.clone() },
        (None, None, Some(s), Some(u)) => WindowSpec::SinceUntil { since: s.clone(), until: u.clone() },
        (None, None, None, None) => bail!("Provide one of --month, --for, or (--since AND --until)"),
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
        save_patches: cli.save_patches.as_deref().map(|p| util::canonicalize_lossy(p)),
        split_out: cli.split_out.as_deref().map(|p| util::canonicalize_lossy(p)),
        out: cli.out,
        github_prs: cli.github_prs,
        include_unmerged: cli.include_unmerged,
        tz: cli.tz,
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = normalize(cli).context("validating CLI flags")?;
    match cfg.mode {
        Mode::Simple => {
            let (since, until) = compute_window_strings(&cfg)?;
            let params = render::SimpleParams{
                repo: cfg.repo.clone(),
                since, until,
                include_merges: cfg.include_merges,
                include_patch: cfg.include_patch,
                max_patch_bytes: cfg.max_patch_bytes,
                tz_local: matches!(cfg.tz, Tz::Local),
            };
            let report = render::run_simple(&params)?;
            if cfg.out == "-" { println!("{}", serde_json::to_string_pretty(&report)?); }
            else { std::fs::write(&cfg.out, serde_json::to_vec_pretty(&report)?)?; }
            Ok(())
        }
        Mode::Full => {
            let (since, until) = compute_window_strings(&cfg)?;
            let label = match &cfg.window { WindowSpec::Month{ym} => Some(ym.clone()), _ => Some("window".into()) };
            let params = render::FullParams{
                repo: cfg.repo.clone(),
                label,
                since, until,
                include_merges: cfg.include_merges,
                include_patch: cfg.include_patch,
                max_patch_bytes: cfg.max_patch_bytes,
                tz_local: matches!(cfg.tz, Tz::Local),
                split_out: cfg.split_out.clone(),
                include_unmerged: cfg.include_unmerged,
            };
            let res = render::run_full(&params)?;
            println!("{}", serde_json::to_string_pretty(&res)?);
            Ok(())
        }
    }
}

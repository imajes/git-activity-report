use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

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
enum Tz { Local, Utc }

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum Mode { Simple, Full }

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WindowSpec {
    Month { ym: String },
    ForPhrase { phrase: String },
    SinceUntil { since: String, until: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct EffectiveConfig {
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
    tz: Tz,
}

fn canonicalize_lossy(p: &Path) -> String {
    // Try to canonicalize; if it fails (nonexistent), join with CWD and return a string
    std::fs::canonicalize(p)
        .or_else(|_| Ok(env::current_dir()?.join(p)))
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .to_string()
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

    let repo = canonicalize_lossy(&cli.repo);

    Ok(EffectiveConfig {
        repo,
        window,
        mode,
        include_merges: cli.include_merges,
        include_patch: cli.include_patch,
        max_patch_bytes: cli.max_patch_bytes,
        save_patches: cli.save_patches.as_deref().map(|p| canonicalize_lossy(Path::new(p))),
        split_out: cli.split_out.as_deref().map(|p| canonicalize_lossy(Path::new(p))),
        out: cli.out,
        github_prs: cli.github_prs,
        include_unmerged: cli.include_unmerged,
        tz: cli.tz,
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = normalize(cli).context("validating CLI flags")?;
    // For now, just print the normalized config as JSON (stderr has any notes)
    println!("{}", serde_json::to_string_pretty(&cfg)?);
    Ok(())
}



// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Entrypoint orchestrator; normalize → resolve ranges → process ranges; print final JSON or pointer
// role: entrypoint/orchestrator
// inputs: CLI flags (see crate::cli::Cli)
// outputs: Either full JSON to stdout, or a pointer {dir,file}/{dir,manifest} to stdout; files on disk when split/multi
// side_effects: Creates directories and writes files in split/multi modes via range_processor
// invariants:
// - when cfg.multi_windows == true, an overall manifest.json is written and a pointer with {dir, manifest} is printed
// - when cfg.split_apart == true and cfg.multi_windows == false, a pointer {dir, file} is printed for the range report
// - when cfg.split_apart == false and cfg.multi_windows == false, a full JSON report is printed to stdout or written to --out
// errors: Bubbles up normalize/resolve/process errors with context
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs (see AGENT_RUBRIC.md)
// === Module Header END ===

use anyhow::Result;
use clap::Parser;

mod cli;
mod commit;
mod enrich;
mod enrichment;
mod ext;
mod gitio;
mod manifest;
mod model;
mod range_processor;
mod range_windows;
mod render;
mod util;

use crate::cli::{Cli, normalize};

fn main() -> Result<()> {
  let cli = Cli::parse();

  if cli.gen_man {
    let page = util::render_man_page::<Cli>()?;
    print!("{}", page);

    return Ok(());
  }

  // Phase 1: normalize CLI
  let mut cfg = normalize(cli)?;

  // Phase 2: resolve now and ranges
  let now_opt = crate::range_windows::parse_now(cfg.now_override.as_deref());
  eprintln!("[gar] resolving ranges...");
  let ranges = crate::range_windows::resolve_ranges(&cfg.window, now_opt)?;
  cfg.multi_windows = ranges.len() > 1;

  // Phase 3: process ranges (single or multi) in a unified flow
  eprintln!("[gar] processing {} range(s)...", ranges.len());
  crate::range_processor::process_ranges(&cfg, ranges, now_opt)
}

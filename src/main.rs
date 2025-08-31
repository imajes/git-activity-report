use anyhow::Result;
use clap::Parser;

mod cli;
mod enrich;
mod enrichment;
mod commit;
mod ext;
mod gitio;
mod model;
mod manifest;
mod range_processor;
mod params;
mod render;
mod util;
mod range_windows;

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
  let ranges = crate::range_windows::resolve_ranges(&cfg.window, now_opt)?;
  cfg.multi_windows = ranges.len() > 1;

  // Phase 3: process ranges (single or multi) in a unified flow
  crate::range_processor::process_ranges(&cfg, ranges, now_opt)
}

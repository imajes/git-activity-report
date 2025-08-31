use crate::cli::EffectiveConfig;
use crate::render::ReportParams;
use crate::range_windows::Tz;

pub fn build_report_params(cfg: &EffectiveConfig, since: String, until: String) -> ReportParams {
  let label = match &cfg.window {
    crate::range_windows::WindowSpec::Month { ym } => Some(ym.clone()),
    _ => Some("window".into()),
  };
  ReportParams {
    repo: cfg.repo.clone(),
    label,
    since,
    until,
    include_merges: cfg.include_merges,
    include_patch: cfg.include_patch,
    max_patch_bytes: cfg.max_patch_bytes,
    tz_local: matches!(cfg.tz, Tz::Local),
    split_apart: cfg.split_apart,
    split_out: if cfg.out != "-" { Some(cfg.out.clone()) } else { None },
    include_unmerged: cfg.include_unmerged,
    save_patches_dir: cfg.save_patches.clone(),
    github_prs: cfg.github_prs,
    now_local: None,
  }
}

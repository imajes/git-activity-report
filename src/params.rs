// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Translate EffectiveConfig + [since,until] into ReportParams for render
// role: parameter mapping
// inputs: EffectiveConfig (read-only), since/until strings
// outputs: ReportParams with label, flags, out dir decisions
// side_effects: none
// invariants:
// - label derives from window type (Month => ym; else "window") unless overridden later
// - split_out is set when out != "-"; otherwise resolved by render
// errors: none (pure builder)
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

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
    estimate_effort: cfg.estimate_effort,
    verbose: cfg.verbose,
    estimate_review_approved_minutes: cfg.estimate_review_approved_minutes,
    estimate_review_changes_minutes: cfg.estimate_review_changes_minutes,
    estimate_review_commented_minutes: cfg.estimate_review_commented_minutes,
    estimate_files_overhead_per_review_minutes: cfg.estimate_files_overhead_per_review_minutes,
    estimate_day_drag_minutes: cfg.estimate_day_drag_minutes,
    estimate_pr_assembly_minutes: cfg.estimate_pr_assembly_minutes,
    estimate_approver_only_minutes: cfg.estimate_approver_only_minutes,
    estimate_cycle_time_cap: cfg.estimate_cycle_time_cap,
  }
}

// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Pure helpers to estimate developer effort (minutes) from commit/PR features
// role: enrichment/estimation
// outputs: EffortEstimate structs computed from in-memory model objects (no IO)
// invariants:
// - Best-effort, explainable, and additive-only (no schema changes here)
// - Deterministic math; bounded outputs; no panics
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use crate::model::{Commit, GithubPullRequest};

/// A lightweight, explainable estimate of time spent (in minutes).
#[derive(Debug, Clone, PartialEq)]
pub struct EffortEstimate {
  pub minutes: f64,
  pub min_minutes: f64,
  pub max_minutes: f64,
  pub confidence: f32, // 0.0 .. 1.0
  pub basis: String,   // short human string: e.g., "files=3 lines=120 lang=rust weight=1.25"
}

/// Static weights and knobs. Future: expose via CLI/env or calibration file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffortWeights {
  pub base_commit_min: f64,
  pub per_file_min: f64,
  pub per_file_tail_min: f64, // after 20 files
  pub sqrt_lines_coeff: f64,
  pub rename_discount: f64,
  pub heavy_delete_discount: f64,
  pub test_only_discount: f64,
  pub mixed_tests_uplift: f64,
}

impl Default for EffortWeights {
  fn default() -> Self {
    Self {
      base_commit_min: 5.0,
      per_file_min: 0.75,
      per_file_tail_min: 0.25,
      sqrt_lines_coeff: 0.9,
      rename_discount: 0.7,
      heavy_delete_discount: 0.8,
      test_only_discount: 0.9,
      mixed_tests_uplift: 1.05,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrEstimateParams {
  pub review_approved_min: f64,
  pub review_changes_min: f64,
  pub review_commented_min: f64,
  pub files_overhead_per_review_min: f64,
  pub day_drag_min: f64,
  pub pr_assembly_min: f64,
  pub approver_only_min: f64,
  pub cycle_time_cap_ratio: f64,
}

impl Default for PrEstimateParams {
  fn default() -> Self {
    Self {
      review_approved_min: 9.0,
      review_changes_min: 6.0,
      review_commented_min: 4.0,
      files_overhead_per_review_min: 0.2,
      day_drag_min: 7.0,
      pr_assembly_min: 10.0,
      approver_only_min: 10.0,
      cycle_time_cap_ratio: 0.5,
    }
  }
}

/// Return a language weight based on simple file extension heuristics.
fn language_weight(path: &str) -> f64 {
  let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();

  match ext.as_str() {
    // Higher cognitive load
    "rs" | "ts" | "go" | "java" | "scala" => 1.25,
    // Moderate
    "py" | "js" | "tsx" | "jsx" | "rb" | "kt" => 1.1,
    // Low
    "md" | "json" | "yaml" | "yml" | "toml" => 0.8,
    _ => 1.0,
  }
}

/// Identify whether a file path likely belongs to tests.
fn is_test_path(path: &str) -> bool {
  let p = path.to_ascii_lowercase();
  p.contains("/test/")
    || p.contains("/tests/")
    || p.ends_with("_test.rs")
    || p.ends_with("_test.py")
    || p.ends_with(".spec.ts")
    || p.ends_with(".spec.js")
}

/// Clamp a value to [min, max].
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
  v.max(lo).min(hi)
}

/// Estimate effort for a single commit using file stats and light heuristics.
pub fn estimate_commit_effort(commit: &Commit) -> EffortEstimate {
  let weights = EffortWeights::default();

  // Phase 1: trivial guards
  if commit.parents.len() > 1 {
    return EffortEstimate {
      minutes: 0.0,
      min_minutes: 0.0,
      max_minutes: 0.0,
      confidence: 0.5,
      basis: "merge commit".into(),
    };
  }

  // Phase 2: extract features (files/lines/tests/renames)
  let mut files = 0usize;
  let mut total_add = 0i64;
  let mut total_del = 0i64;
  let mut lang_weight_acc = 0.0f64;
  let mut renames = 0usize;
  let mut test_files = 0usize;

  for f in &commit.files {
    files += 1;
    total_add += f.additions.unwrap_or(0);
    total_del += f.deletions.unwrap_or(0);

    let w = language_weight(&f.file);
    lang_weight_acc += w;
    if is_test_path(&f.file) {
      test_files += 1;
    }

    if f.status.starts_with('R') {
      renames += 1;
    }
  }

  let total_lines = (total_add + total_del).max(0) as f64;
  let avg_lang_weight = if files > 0 { lang_weight_acc / files as f64 } else { 1.0 };
  let tests_ratio = if files > 0 {
    test_files as f64 / files as f64
  } else {
    0.0
  };
  let deletions_ratio = if total_lines > 0.0 {
    (total_del as f64 / total_lines).clamp(0.0, 1.0)
  } else {
    0.0
  };
  let rename_ratio = if files > 0 { renames as f64 / files as f64 } else { 0.0 };

  // Phase 3: base minutes with diminishing returns
  let mut minutes = weights.base_commit_min;

  if files > 0 {
    let tail = files.saturating_sub(20) as f64;
    let head = files.min(20) as f64;
    minutes += head * weights.per_file_min + tail * weights.per_file_tail_min;
  }

  minutes += total_lines.sqrt() * weights.sqrt_lines_coeff;
  minutes *= avg_lang_weight;

  if rename_ratio > 0.5 {
    minutes *= weights.rename_discount;
  }
  if deletions_ratio > 0.7 {
    minutes *= weights.heavy_delete_discount;
  }

  if tests_ratio >= 0.8 {
    minutes *= weights.test_only_discount;
  } else if tests_ratio > 0.0 {
    minutes *= weights.mixed_tests_uplift;
  }

  // Phase 4: finalize
  let minutes = clamp(minutes, 1.0, 240.0);
  let min_minutes = clamp(minutes * 0.8, 0.5, 240.0);
  let max_minutes = clamp(minutes * 1.25, 1.0, 360.0);

  let basis = format!(
    "files={} lines={} lang_w={:.2} tests={:.0}% renames={:.0}%",
    files,
    (total_add + total_del).max(0),
    avg_lang_weight,
    tests_ratio * 100.0,
    rename_ratio * 100.0
  );

  EffortEstimate {
    minutes,
    min_minutes,
    max_minutes,
    confidence: 0.55,
    basis,
  }
}

/// Derive review-counts triple (approved, changes, commented) from optional counters on PR.
fn derive_review_counts(pr: &GithubPullRequest) -> (i64, i64, i64) {
  let approvals = pr.approval_count.unwrap_or(0);
  let changes = pr.change_request_count.unwrap_or(0);
  let total = pr.review_count.unwrap_or(approvals + changes);
  let commented = (total - approvals - changes).max(0);
  (approvals, changes, commented)
}

/// Estimate effort for a single PR using commit estimates and review metadata.
pub fn estimate_pr_effort(pr: &GithubPullRequest, range_commits: &[Commit]) -> EffortEstimate {
  let _weights = EffortWeights::default();
  let params = PrEstimateParams::default();

  // Phase 1: collect commit estimates by matching sha
  let mut subtotal = 0.0f64;
  let mut matched = 0usize;
  let mut files_total = 0usize;
  use std::collections::BTreeSet;
  let mut distinct_days: BTreeSet<String> = BTreeSet::new();

  if let Some(pr_commits) = &pr.commits {
    for pc in pr_commits {
      if let Some(c) = range_commits.iter().find(|c| c.sha == pc.sha) {
        let est = estimate_commit_effort(c);
        subtotal += est.minutes;
        matched += 1;
        files_total += c.files.len();
        let day = c.timestamps.commit_local.chars().take(10).collect::<String>();
        distinct_days.insert(day);
      }
    }
  }

  // Phase 2: review overheads (approximate)
  let mut overhead = params.pr_assembly_min; // PR assembly + description

  let (approved, changes, commented) = derive_review_counts(pr);
  if approved > 0 || changes > 0 || commented > 0 {
    overhead += approved as f64 * params.review_approved_min;
    overhead += changes as f64 * params.review_changes_min;
    overhead += commented as f64 * params.review_commented_min;
    let total_reviews = (approved + changes + commented) as usize;
    if total_reviews > 1 {
      let extra = total_reviews - 1;
      overhead += (files_total as f64) * params.files_overhead_per_review_min * (extra as f64);
    }
  } else if pr.approver.is_some() {
    overhead += params.approver_only_min;
  }

  // Multi-day drag: additional context-switching cost per extra day
  if distinct_days.len() > 1 {
    overhead += (distinct_days.len() as f64 - 1.0) * params.day_drag_min;
  }

  // Phase 3: finalize
  let mut minutes = subtotal + overhead;

  // Cycle-time bounding (if created_at/merged_at available)
  if let (Some(created), Some(merged)) = (&pr.created_at, &pr.merged_at) {
    if let (Ok(ct), Ok(mt)) = (
      chrono::DateTime::parse_from_rfc3339(created),
      chrono::DateTime::parse_from_rfc3339(merged),
    ) {
      if mt > ct {
        let duration_mins = (mt - ct).num_minutes().max(0) as f64;
        let cap = duration_mins * params.cycle_time_cap_ratio; // bound to a fraction of wall time
        if minutes > cap {
          minutes = cap;
        }
      }
    }
  }

  let confidence = if matched > 0 { 0.65 } else { 0.45 };
  let min_minutes = clamp(minutes * 0.85, 1.0, 10000.0);
  let max_minutes = clamp(minutes * 1.2, 1.0, 10000.0);

  let basis = format!(
    "commits_matched={} subtotal={:.1} overhead={:.1}",
    matched, subtotal, overhead
  );

  EffortEstimate {
    minutes,
    min_minutes,
    max_minutes,
    confidence,
    basis,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn mk_commit(files: Vec<(&str, &str, i64, i64)>, parents: usize, date: &str) -> Commit {
    let mut c = Commit {
      sha: "s".into(),
      short_sha: "s".into(),
      parents: vec![],
      author: crate::model::Person {
        name: "A".into(),
        email: "a@ex".into(),
        date: "".into(),
      },
      committer: crate::model::Person {
        name: "A".into(),
        email: "a@ex".into(),
        date: "".into(),
      },
      timestamps: crate::model::Timestamps {
        author: 0,
        commit: 0,
        author_local: date.into(),
        commit_local: date.into(),
        timezone: "utc".into(),
      },
      subject: "s".into(),
      body: "".into(),
      files: vec![],
      diffstat_text: "".into(),
      patch_references: crate::model::PatchReferences {
        embed: false,
        git_show_cmd: "".into(),
        local_patch_file: None,
        github: None,
      },
      patch_clipped: None,
      patch_lines: None,
      body_lines: None,
      estimated_minutes: None,
      estimated_minutes_min: None,
      estimated_minutes_max: None,
      estimate_confidence: None,
      estimate_basis: None,
      github: None,
    };
    c.parents = (0..parents).map(|_| "p".into()).collect();
    c.files = files
      .into_iter()
      .map(|(file, status, add, del)| crate::model::FileEntry {
        file: file.into(),
        status: status.into(),
        old_path: None,
        additions: Some(add),
        deletions: Some(del),
      })
      .collect();
    c
  }

  #[test]
  fn commit_basic_weights() {
    let c = mk_commit(vec![("src/lib.rs", "M", 100, 20)], 1, "2025-09-01T00:00:00Z");
    let e = estimate_commit_effort(&c);
    assert!(e.minutes > 5.0);
    assert!(e.max_minutes > e.minutes);
    assert!(e.min_minutes < e.minutes);
  }

  #[test]
  fn commit_rename_discount_applies() {
    let c1 = mk_commit(vec![("a.txt", "M", 50, 0)], 1, "2025-09-01T00:00:00Z");
    let c2 = mk_commit(vec![("b.txt", "R100", 50, 0)], 1, "2025-09-01T00:00:00Z");
    let e1 = estimate_commit_effort(&c1);
    let e2 = estimate_commit_effort(&c2);
    assert!(e2.minutes < e1.minutes);
  }

  #[test]
  fn pr_estimation_uses_commits_reviews_and_days() {
    let c1 = mk_commit(vec![("src/lib.rs", "M", 10, 10)], 1, "2025-09-01T00:00:00Z");
    let mut c2 = mk_commit(vec![("src/main.rs", "M", 10, 10)], 1, "2025-09-02T00:00:00Z");
    c2.sha = "b".into();
    let mut c1b = c1.clone();
    c1b.sha = "a".into();
    let range = vec![c1b.clone(), c2.clone()];
    let pr = GithubPullRequest {
      number: 1,
      title: "t".into(),
      state: "closed".into(),
      body_lines: None,
      created_at: Some("2025-09-01T00:00:00Z".into()),
      merged_at: Some("2025-09-03T00:00:00Z".into()),
      closed_at: None,
      html_url: "".into(),
      diff_url: None,
      patch_url: None,
      submitter: None,
      approver: None,
      reviewers: None,
      head: None,
      base: None,
      commits: Some(vec![
        crate::model::PullRequestCommit {
          sha: c1b.sha.clone(),
          short_sha: c1b.short_sha.clone(),
          subject: c1b.subject.clone(),
        },
        crate::model::PullRequestCommit {
          sha: c2.sha.clone(),
          short_sha: c2.short_sha.clone(),
          subject: c2.subject.clone(),
        },
      ]),
      review_count: Some(3),
      approval_count: Some(2),
      change_request_count: Some(1),
      time_to_first_review_seconds: None,
      time_to_merge_seconds: None,
      estimated_minutes: None,
      estimated_minutes_min: None,
      estimated_minutes_max: None,
      estimate_confidence: None,
      estimate_basis: None,
    };
    let e = estimate_pr_effort(&pr, &range);
    assert!(e.minutes > 0.0);
    assert!(e.max_minutes >= e.minutes);
    assert!(e.min_minutes <= e.minutes);
  }
}

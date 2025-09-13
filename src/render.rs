// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Assemble per-range reports; when split_apart, write commit shards and per-range report, returning a pointer
// role: assembly/render
// inputs: ReportParams (repo, since/until, flags, label, out dir)
// outputs: SimpleReport JSON (non-split) or pointer {dir, file} (split)
// side_effects: In split mode, writes shard files under <base>/<label>/ and report-<label>.json; may write .patch files if requested
// invariants:
// - run_simple returns fully in-memory report consistent with schema
// - run_report returns pointer JSON when split; otherwise full report JSON; file names are stable
// - shard filenames follow YYYY.MM.DD-HH.MM-<shortsha>.json
// errors: Propagates git and IO errors with context (paths, git args)
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Local};

use crate::gitio;
use crate::model::{
  BranchItems, ChangeSet, Commit, ManifestItem, Person, RangeInfo, ReportOptions, ReportSummary, SimpleReport,
  UnmergedActivity,
};
use crate::util::format_shard_name;

// Clippy: factor complex tuple into a named alias for readability.
type ProcessRangeOut = (Vec<Commit>, Vec<ManifestItem>, ChangeSet, BTreeMap<String, i64>);

// --- Local helpers to unify repeated patterns ---
fn build_process_context<'a>(params: &'a ReportParams) -> ProcessContext<'a> {
  ProcessContext {
    repo: &params.repo,
    tz: &params.tz,
    github_prs: params.github_prs,
    include_patch: params.include_patch,
    max_patch_bytes: params.max_patch_bytes,
    estimate_effort: params.estimate_effort,
  }
}

fn build_report_options(params: &ReportParams) -> ReportOptions {
  ReportOptions {
    include_merges: params.include_merges,
    include_patch: params.include_patch,
    include_unmerged: params.include_unmerged,
    tz: params.tz.clone(),
  }
}

fn author_key_for(p: &Person) -> String {
  format!("{} <{}>", p.name, p.email)
}

/// Write a single commit shard JSON under `subdir`, named with `tz`-relative timestamp and short SHA.
fn write_commit_shard(subdir: &Path, commit: &Commit, tz: &str) -> anyhow::Result<String> {
  let fname = format_shard_name(commit.timestamps.commit, &commit.short_sha, tz);
  let shard_path = subdir.join(&fname);

  if let Some(parent) = shard_path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(&shard_path, serde_json::to_vec(&commit)?)?;

  Ok(fname)
}

/// Update `summary` and `files_touched` given `commit`'s file entries.
fn accumulate_summary_and_files(commit: &Commit, summary: &mut ChangeSet, files_touched: &mut HashSet<String>) {
  let (add, del) = crate::commit::sum_additions_deletions(&commit.files);
  summary.additions += add;
  summary.deletions += del;

  for f in &commit.files {
    files_touched.insert(f.file.clone());
  }
}

// --- Parameter Structs ---
// These remain unchanged as they define the public API.

#[derive(Debug)]
pub struct ReportParams {
  pub repo: String,
  pub label: Option<String>,
  pub since: String,
  pub until: String,
  pub include_merges: bool,
  pub include_patch: bool,
  pub max_patch_bytes: usize,
  pub tz: String,
  pub split_apart: bool,
  pub split_out: Option<String>,
  pub include_unmerged: bool,
  pub save_patches_dir: Option<String>,
  pub github_prs: bool,
  pub now_local: Option<DateTime<Local>>,
  pub estimate_effort: bool,
}

/// Build `ReportParams` from an `EffectiveConfig` and an explicit `[since, until]` window.
pub fn build_report_params(cfg: &crate::cli::EffectiveConfig, since: String, until: String) -> ReportParams {
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
    tz: cfg.tz.clone(),
    split_apart: cfg.split_apart,
    split_out: if cfg.out != "-" { Some(cfg.out.clone()) } else { None },
    include_unmerged: cfg.include_unmerged,
    save_patches_dir: cfg.save_patches.clone(),
    github_prs: cfg.github_prs,
    now_local: None,
    estimate_effort: cfg.estimate_effort,
  }
}

// --- Internal Context for Processing moved to crate::commit ---

// --- Reusable Helper Functions ---

// patch file writing moved to crate::commit::save_patch_to_disk
use crate::commit::{ProcessContext, process_commit};

// --- Report Generation Logic ---

/// Generates a `SimpleReport` containing all commit data in memory.
pub fn run_simple(params: &ReportParams) -> Result<SimpleReport> {
  let shas = gitio::rev_list(&params.repo, &params.since, &params.until, params.include_merges)?;
  let context = build_process_context(params);

  let mut commits: Vec<Commit> = Vec::with_capacity(shas.len());
  let mut authors: BTreeMap<String, i64> = BTreeMap::new();
  let mut changeset = ChangeSet {
    additions: 0,
    deletions: 0,
    files_touched: 0,
  };
  let mut files_touched: HashSet<String> = HashSet::new();

  for sha in shas.iter() {
    let mut commit = process_commit(sha, &context)?;

    if let Some(patches_dir_str) = &params.save_patches_dir {
      crate::commit::save_patch_to_disk(&mut commit, &params.repo, Path::new(patches_dir_str))?;
    }

    // Accumulate summary stats
    let author_key = author_key_for(&commit.author);
    *authors.entry(author_key).or_insert(0) += 1;

    for f in &commit.files {
      changeset.additions += f.additions.unwrap_or(0);
      changeset.deletions += f.deletions.unwrap_or(0);
      files_touched.insert(f.file.clone());
    }

    commits.push(commit);
  }

  // Optional: attach PR estimates using the full commit range context
  if params.github_prs && params.estimate_effort {
    attach_pr_estimates(&mut commits);
  }

  changeset.files_touched = files_touched.len();

  let range = RangeInfo {
    label: params.label.clone().unwrap_or_else(|| "window".into()),
    start: params.since.clone(),
    end: params.until.clone(),
  };
  let report_options = build_report_options(params);
  let summary = ReportSummary {
    repo: params.repo.clone(),
    range,
    count: commits.len(),
    report_options,
    changes: changeset,
  };

  let report = SimpleReport {
    summary,
    authors,
    commits,
    items: None,
    unmerged_activity: None,
  };

  Ok(report)
}

/// Unified entry: returns a report JSON; when split_apart, writes shards and returns a pointer {dir,file}.
pub fn run_report(params: &ReportParams) -> Result<serde_json::Value> {
  if !params.split_apart {
    let r = run_simple(params)?;
    return Ok(serde_json::to_value(&r)?);
  }
  let label = params.label.clone().unwrap_or_else(|| "window".to_string());
  let base_dir = if let Some(dir) = &params.split_out {
    dir.clone()
  } else {
    let tmp = std::env::temp_dir();
    let now_for_dir = params.now_local.unwrap_or_else(Local::now);
    tmp
      .join(format!("activity-{}", now_for_dir.format("%Y%m%d-%H%M%S")))
      .to_string_lossy()
      .to_string()
  };
  let subdir = Path::new(&base_dir).join(&label);
  std::fs::create_dir_all(&subdir)?;

  // Process the primary commit range: write shards and collect items/summary/authors/commits
  let (commits, items, summary, authors) = process_commit_range(params, &subdir, &label)?;

  // Optionally process unmerged branches
  let _unmerged_activity = if params.include_unmerged {
    Some(process_unmerged_branches(params, &subdir, &label)?)
  } else {
    None
  };

  // Build the unified report (simple + items) using already processed commits

  let range = RangeInfo {
    label: label.clone(),
    start: params.since.clone(),
    end: params.until.clone(),
  };
  let report_options = build_report_options(params);
  let summary = ReportSummary {
    repo: params.repo.clone(),
    range,
    count: commits.len(),
    report_options,
    changes: summary,
  };
  let report = SimpleReport {
    summary,
    authors,
    commits,
    items: Some(items),
    unmerged_activity: None,
  };

  let report_path = Path::new(&base_dir).join(format!("report-{}.json", label));
  std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

  Ok(serde_json::json!({ "dir": base_dir, "file": format!("report-{}.json", label) }))
}

// --- `run_full` Sub-logic ---

/// Helper for `run_full` to process the main list of commits.
fn process_commit_range(params: &ReportParams, subdir: &Path, label: &str) -> Result<ProcessRangeOut> {
  let shas = gitio::rev_list(&params.repo, &params.since, &params.until, params.include_merges)?;
  let context = build_process_context(params);

  let mut commits: Vec<Commit> = Vec::with_capacity(shas.len());
  let mut items = Vec::with_capacity(shas.len());
  let mut authors: BTreeMap<String, i64> = BTreeMap::new();
  let mut summary = ChangeSet {
    additions: 0,
    deletions: 0,
    files_touched: 0,
  };
  let mut files_touched: HashSet<String> = HashSet::new();

  for sha in shas.iter() {
    let mut commit = process_commit(sha, &context)?;

    if params.save_patches_dir.is_some() {
      let patch_dir = subdir.join("patches");
      crate::commit::save_patch_to_disk(&mut commit, &params.repo, &patch_dir)?;
    }

    // Write commit shard to disk
    let fname = write_commit_shard(subdir, &commit, &params.tz)?;

    // Accumulate manifest data
    items.push(ManifestItem {
      sha: commit.sha.clone(),
      file: Path::new(label).join(&fname).to_string_lossy().to_string(),
      subject: commit.subject.clone(),
    });
    let author_key = author_key_for(&commit.author);
    *authors.entry(author_key).or_insert(0) += 1;
    accumulate_summary_and_files(&commit, &mut summary, &mut files_touched);

    commits.push(commit);
  }

  // Optional: attach PR estimates using the full commit range context
  if params.github_prs && params.estimate_effort {
    attach_pr_estimates(&mut commits);
  }

  summary.files_touched = files_touched.len();

  Ok((commits, items, summary, authors))
}

/// Compute and attach PR-level effort estimates to each commit's PRs using the full range context.
fn attach_pr_estimates(commits: &mut [Commit]) {
  // Keep a snapshot of commits for estimation context
  let snapshot: Vec<Commit> = commits.to_vec();

  for c in commits.iter_mut() {
    if let Some(gh) = c.github.as_mut() {
      for pr in gh.pull_requests.iter_mut() {
        let est = crate::enrichment::effort::estimate_pr_effort(pr, &snapshot);
        pr.estimated_minutes = Some(est.minutes);
        pr.estimated_minutes_min = Some(est.min_minutes);
        pr.estimated_minutes_max = Some(est.max_minutes);
        pr.estimate_confidence = Some(est.confidence as f64);
        pr.estimate_basis = Some(est.basis);
      }
    }
  }
}

/// Helper for `run_full` to process unmerged branches.
fn process_unmerged_branches(params: &ReportParams, subdir: &Path, label: &str) -> Result<UnmergedActivity> {
  // Collect list of branches to scan (excluding current)
  let current_branch = gitio::current_branch(&params.repo)?;
  let branches: Vec<String> = gitio::list_local_branches(&params.repo)?
    .into_iter()
    .filter(|b| Some(b.as_str()) != current_branch.as_deref())
    .collect();

  let context = ProcessContext {
    repo: &params.repo,
    tz: &params.tz,
    github_prs: params.github_prs,
    include_patch: params.include_patch,
    max_patch_bytes: params.max_patch_bytes,
    estimate_effort: params.estimate_effort,
  };

  let mut unmerged_activity = UnmergedActivity {
    branches_scanned: branches.len(),
    total_unmerged_commits: 0,
    branches: Vec::new(),
  };

  for branch in branches {
    let unmerged_shas = collect_unmerged_shas(params, &branch)?;

    if unmerged_shas.is_empty() {
      continue;
    }

    let branch_dir_name = branch.replace('/', "__");
    let branch_dir = subdir.join("unmerged").join(&branch_dir_name);

    let branch_items = write_branch_shards(&context, params, label, &branch_dir_name, &branch_dir, &unmerged_shas)?;

    let (behind, ahead) = gitio::branch_ahead_behind(&params.repo, &branch)?;
    unmerged_activity.total_unmerged_commits += branch_items.len();

    unmerged_activity.branches.push(BranchItems {
      name: branch.clone(),
      merged_into_head: gitio::branch_merged_into_head(&params.repo, &branch)?,
      ahead_of_head: ahead,
      behind_head: behind,
      items: branch_items,
    });
  }

  Ok(unmerged_activity)
}

// --- Extracted Helpers (Unmerged Branches) ---

/// Collect SHAs for commits on `branch` not yet merged into `HEAD` within the configured time range.
fn collect_unmerged_shas(params: &ReportParams, branch: &str) -> anyhow::Result<Vec<String>> {
  let shas = gitio::unmerged_commits_in_range(
    &params.repo,
    branch,
    &params.since,
    &params.until,
    params.include_merges,
  )?;

  Ok(shas)
}

/// Process `unmerged_shas` for a branch: build commits, optionally save patches, write shards, and return manifest items.
fn write_branch_shards(
  context: &ProcessContext,
  params: &ReportParams,
  label: &str,
  branch_dir_name: &str,
  branch_dir: &Path,
  unmerged_shas: &[String],
) -> anyhow::Result<Vec<ManifestItem>> {
  let mut branch_items = Vec::with_capacity(unmerged_shas.len());

  for sha in unmerged_shas.iter() {
    let mut commit = process_commit(sha, context)?;

    if params.save_patches_dir.is_some() {
      let patch_dir = branch_dir.join("patches");
      crate::commit::save_patch_to_disk(&mut commit, &params.repo, &patch_dir)?;
    }

    let fname = write_commit_shard(branch_dir, &commit, &params.tz)?;

    let item = ManifestItem {
      sha: commit.sha.clone(),
      file: Path::new(label)
        .join("unmerged")
        .join(branch_dir_name)
        .join(fname)
        .to_string_lossy()
        .to_string(),
      subject: commit.subject.clone(),
    };

    branch_items.push(item);
  }

  Ok(branch_items)
}

// Shard filename helper lives in util; imported above.

// --- Tests ---

#[cfg(test)]
mod tests {
  use super::*;
  use crate::commit::build_file_entries_from;
  // shard_name test moved to util tests

  #[test]
  fn file_entries_fallback_uses_numstat() {
    use std::collections::HashMap as Map;
    let num_list = vec![("file.txt".to_string(), Some(1), Some(0))];
    let mut num_map = Map::new();
    num_map.insert("file.txt".to_string(), (Some(1), Some(0)));
    let name_status_list: Vec<Map<String, String>> = vec![];
    use crate::commit::build_file_entries_from;
    let entries = build_file_entries_from(num_list, num_map, name_status_list);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file, "file.txt");
    assert_eq!(entries[0].status, "M");
    assert_eq!(entries[0].additions, Some(1));
  }

  fn fixture_repo() -> String {
    if let Ok(dir) = std::env::var("GAR_FIXTURE_REPO_DIR") {
      return dir;
    }
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/.tmp/tmpdir");
    std::fs::read_to_string(p).expect("fixture path").trim().to_string()
  }

  #[test]
  fn run_simple_with_patch_and_save() {
    let repo = fixture_repo();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let params = ReportParams {
      repo,
      label: Some("window".into()),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
      include_merges: true,
      include_patch: true,
      max_patch_bytes: 16,
      tz: "utc".into(),
      split_apart: false,
      split_out: None,
      include_unmerged: false,
      save_patches_dir: Some(tmpdir.path().to_string_lossy().to_string()),
      github_prs: true,
      now_local: None,
      estimate_effort: false,
    };
    let report = run_simple(&params).unwrap();
    assert!(report.summary.count >= 1);
    assert!(report.summary.count >= 1);
    assert!(std::fs::read_dir(tmpdir.path()).unwrap().next().is_some());
    let clipped_any = report.commits.iter().any(|c| c.patch_clipped == Some(true));
    assert!(clipped_any);
  }

  #[test]
  fn run_simple_no_merges_no_patch_no_save() {
    let repo = fixture_repo();
    let params = ReportParams {
      repo,
      label: Some("window".into()),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
      include_merges: false,
      include_patch: false,
      max_patch_bytes: 0,
      tz: "local".into(),
      split_apart: false,
      split_out: None,
      include_unmerged: false,
      save_patches_dir: None,
      github_prs: false,
      now_local: None,
      estimate_effort: false,
    };
    let report = run_simple(&params).unwrap();
    assert!(report.summary.count >= 1);
    assert!(report.summary.count >= 1);
  }

  #[test]
  fn run_split_with_unmerged_and_patches() {
    let repo = fixture_repo();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let params = ReportParams {
      repo,
      label: Some("window".into()),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
      include_merges: true,
      include_patch: false,
      max_patch_bytes: 0,
      tz: "local".into(),
      split_apart: true,
      split_out: Some(tmpdir.path().to_string_lossy().to_string()),
      include_unmerged: true,
      save_patches_dir: Some(tmpdir.path().join("patches").to_string_lossy().to_string()),
      github_prs: false,
      now_local: None,
      estimate_effort: false,
    };
    let out = run_report(&params).unwrap();
    let dir = out.get("dir").unwrap().as_str().unwrap();
    let file = out.get("file").unwrap().as_str().unwrap();
    let path = std::path::Path::new(dir).join(file);
    assert!(path.exists());
  }

  #[test]
  fn run_split_embeds_patches() {
    let repo = fixture_repo();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let params = ReportParams {
      repo,
      label: Some("window".into()),
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
      include_merges: true,
      include_patch: true,
      max_patch_bytes: 32,
      tz: "utc".into(),
      split_apart: true,
      split_out: Some(tmpdir.path().to_string_lossy().to_string()),
      include_unmerged: false,
      save_patches_dir: None,
      github_prs: true,
      now_local: None,
      estimate_effort: false,
    };
    let out = run_report(&params).unwrap();
    let dir = out.get("dir").unwrap().as_str().unwrap();
    let file = out.get("file").unwrap().as_str().unwrap();
    let path = std::path::Path::new(dir).join(file);
    assert!(path.exists());
  }

  #[test]
  fn run_split_no_merges() {
    let repo = fixture_repo();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let params = ReportParams {
      repo,
      label: None,
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
      include_merges: false,
      include_patch: false,
      max_patch_bytes: 0,
      tz: "utc".into(),
      split_apart: true,
      split_out: Some(tmpdir.path().to_string_lossy().to_string()),
      include_unmerged: false,
      save_patches_dir: None,
      github_prs: false,
      now_local: None,
      estimate_effort: false,
    };
    let out = run_report(&params).unwrap();
    let dir = out.get("dir").unwrap().as_str().unwrap();
    assert!(std::path::Path::new(dir).exists());
  }

  #[test]
  fn run_split_with_real_unmerged_branch() {
    // Create a tiny repo with an unmerged branch having unique commits
    let td = tempfile::TempDir::new().unwrap();
    let repo = td.path();
    let sh = |args: &[&str]| {
      let st = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
      assert!(st.success(), "git {:?} failed", args);
    };
    sh(&["init", "-q", "-b", "main"]);
    sh(&["config", "user.name", "Fixture Bot"]);
    sh(&["config", "user.email", "fixture@example.com"]);
    sh(&["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    sh(&["add", "."]);
    sh(&["commit", "-q", "-m", "A"]);
    sh(&["checkout", "-q", "-b", "feature/x"]);
    std::fs::write(repo.join("b.txt"), "b\n").unwrap();
    sh(&["add", "."]);
    // Include a body to exercise body_lines derivation
    sh(&["commit", "-q", "-m", "B subject", "-m", "Body line 1\nBody line 2"]);
    sh(&["switch", "-q", "-C", "main"]);

    let tmpdir = tempfile::TempDir::new().unwrap();
    let params = ReportParams {
      repo: repo.to_string_lossy().to_string(),
      label: Some("window".into()),
      since: "1970-01-01".into(),
      until: "2100-01-01".into(),
      include_merges: true,
      include_patch: false,
      max_patch_bytes: 0,
      tz: "utc".into(),
      split_apart: true,
      split_out: Some(tmpdir.path().to_string_lossy().to_string()),
      include_unmerged: true,
      save_patches_dir: None,
      github_prs: false,
      now_local: None,
      estimate_effort: false,
    };
    let out = run_report(&params).unwrap();
    let dir = out.get("dir").unwrap().as_str().unwrap();
    let file = out.get("file").unwrap().as_str().unwrap();
    let path = std::path::Path::new(dir).join(file);
    assert!(path.exists());
    let data = std::fs::read(&path).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&data).unwrap();
    // Report should include summary.range and commits array
    assert!(v.get("summary").and_then(|s| s.get("range")).is_some());
    assert!(v.get("commits").is_some());
  }

  // clip_patch tests moved to util

  #[test]
  fn run_simple_with_empty_commit_exercises_name_status_fallback() {
    // Use pure helper-based test for fallback logic instead of creating a special repo.
    // This keeps the test deterministic and covers the intended branch.
    use std::collections::HashMap as Map;
    let num_list = vec![("file.txt".to_string(), Some(0), Some(0))];
    let mut num_map = Map::new();
    num_map.insert("file.txt".to_string(), (Some(0), Some(0)));
    let name_status_list: Vec<Map<String, String>> = vec![];
    let entries = build_file_entries_from(num_list, num_map, name_status_list);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file, "file.txt");
  }

  // proptests for clip_patch moved to util
}

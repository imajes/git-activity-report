// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Construct per-commit objects; enrich with optional PR links; embed or save patches as configured
// role: commit construction/enrichment
// inputs: repo path, commit sha, ProcessContext (tz, flags, limits)
// outputs: Commit structs with files/diffstat/patch_ref; optional patch text and saved patch files
// side_effects: Reads git; may write .patch files to disk (save_patch_to_disk)
// invariants:
// - clip_patch preserves UTF-8 boundaries; patch_clipped is accurate
// - body_lines derived when body is non-empty
// - enrichment is best-effort; absence of PRs leaves fields None
// errors: Propagates git IO errors; enrichment failures are swallowed (best-effort)
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use anyhow::Result;

use crate::gitio;
use crate::model::{Commit, FileEntry, PatchRef, Person, Timestamps};
use crate::util::{iso_in_tz, short_sha, clip_patch};
use crate::enrichment::github_pull_requests::enrich_with_github_prs;
use std::path::Path;

pub struct ProcessContext<'a> {
  pub repo: &'a str,
  pub tz_local: bool,
  pub github_prs: bool,
  pub include_patch: bool,
  pub max_patch_bytes: usize,
  pub estimate_effort: bool,
  pub verbose: bool,
}

/// Builds a vector of `FileEntry` structs for a given commit.
pub fn build_file_entries(repo: &str, sha: &str) -> Result<Vec<FileEntry>> {
  let (num_list, num_map) = gitio::commit_numstat(repo, sha)?;
  let name_status_list = gitio::commit_name_status(repo, sha)?;
  Ok(build_file_entries_from(num_list, num_map, name_status_list))
}

pub fn build_file_entries_from(
  num_list: Vec<(String, Option<i64>, Option<i64>)>,
  num_map: std::collections::HashMap<String, (Option<i64>, Option<i64>)>,
  name_status_list: Vec<std::collections::HashMap<String, String>>,
) -> Vec<FileEntry> {
  if !name_status_list.is_empty() {
    name_status_list
      .into_iter()
      .map(|entry| {
        let path = entry.get("file").cloned().unwrap_or_default();
        let (additions, deletions) = num_map.get(&path).cloned().unwrap_or((None, None));
        FileEntry {
          file: path,
          status: entry.get("status").cloned().unwrap_or_else(|| "M".to_string()),
          old_path: entry.get("old_path").cloned(),
          additions,
          deletions,
        }
      })
      .collect()
  } else {
    num_list
      .into_iter()
      .map(|(path, additions, deletions)| FileEntry {
        file: path,
        status: "M".to_string(),
        old_path: None,
        additions,
        deletions,
      })
      .collect()
  }
}

pub fn build_commit_object(sha: &str, context: &ProcessContext) -> Result<Commit> {
  let meta = gitio::commit_meta(context.repo, sha)?;
  let files = build_file_entries(context.repo, sha)?;
  let diffstat_text = gitio::commit_shortstat(context.repo, sha)?;

  let timestamps = Timestamps {
    author: meta.at,
    commit: meta.ct,
    author_local: iso_in_tz(meta.at, context.tz_local),
    commit_local: iso_in_tz(meta.ct, context.tz_local),
    timezone: if context.tz_local { "local".into() } else { "utc".into() },
  };

  let author = Person {
    name: meta.author_name,
    email: meta.author_email,
    date: meta.author_date,
  };

  let committer = Person {
    name: meta.committer_name,
    email: meta.committer_email,
    date: meta.committer_date,
  };

  let patch_ref = PatchRef {
    embed: context.include_patch,
    git_show_cmd: format!("git show --patch --format= --no-color {}", meta.sha),
    local_patch_file: None,
    github_diff_url: None,
    github_patch_url: None,
  };

  let commit = Commit {
    sha: meta.sha.clone(),
    short_sha: short_sha(&meta.sha),
    parents: meta.parents,
    author,
    committer,
    timestamps,
    subject: meta.subject,
    body: meta.body,
    files,
    diffstat_text,
    patch_ref,
    patch: None,
    patch_clipped: None,
    github_prs: None,
    body_lines: None,
    estimated_minutes: None,
    estimated_minutes_min: None,
    estimated_minutes_max: None,
    estimate_confidence: None,
    estimate_basis: None,
  };

  Ok(commit)
}

/// Processes a single git commit SHA and returns a fully populated `Commit` struct.
pub fn process_commit(sha: &str, context: &ProcessContext) -> Result<Commit> {
  let mut commit = build_commit_object(sha, context)?;

  if context.include_patch {
    let patch_text = gitio::commit_patch(context.repo, sha)?;
    let (patch, clipped) = clip_patch(patch_text, context.max_patch_bytes);
    commit.patch = patch;
    commit.patch_clipped = clipped;
  }

  if context.github_prs {
    enrich_with_github_prs(&mut commit, context.repo);
  }

  if !commit.body.is_empty() {
    commit.body_lines = Some(commit.body.lines().map(String::from).collect());
  }

  if context.estimate_effort {
    let weights = crate::enrichment::effort::EffortWeights::default();
    let est = crate::enrichment::effort::estimate_commit_effort(&commit, weights);
    commit.estimated_minutes = Some(est.minutes);
    commit.estimated_minutes_min = Some(est.min_minutes);
    commit.estimated_minutes_max = Some(est.max_minutes);
    commit.estimate_confidence = Some(est.confidence as f64);
    commit.estimate_basis = Some(est.basis.clone());
    if context.verbose {
      eprintln!(
        "[estimate] commit {}: {:.1}m (min {:.1}, max {:.1}, conf {:.2}) basis={}",
        commit.short_sha,
        est.minutes,
        est.min_minutes,
        est.max_minutes,
        est.confidence,
        est.basis
      );
    }
  }

  Ok(commit)
}

pub fn save_patch_to_disk(commit: &mut Commit, repo: &str, directory_path: &Path) -> Result<()> {
  std::fs::create_dir_all(directory_path)?;
  let path = directory_path.join(format!("{}.patch", commit.short_sha));
  let patch_content = gitio::commit_patch(repo, &commit.sha)?;
  std::fs::write(&path, patch_content)?;
  commit.patch_ref.local_patch_file = Some(path.to_string_lossy().to_string());
  Ok(())
}

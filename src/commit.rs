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
use chrono::TimeZone;

use crate::enrichment::github_pull_requests::enrich_with_github_prs;
use crate::gitio;
use crate::model::{Commit, FileEntry, PatchReferences, Person, Timestamps};
use crate::util::{clip_patch, iso_in_tz, short_sha};
use std::path::Path;

pub struct ProcessContext<'a> {
  pub repo: &'a str,
  pub tz: &'a str,
  pub github_prs: bool,
  pub include_patch: bool,
  pub max_patch_bytes: usize,
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

  // Synthesize a shortstat-like summary from numstat-derived entries to avoid an extra git call.
  let files_changed = files.len();
  let additions: i64 = files.iter().map(|f| f.additions.unwrap_or(0)).sum();
  let deletions: i64 = files.iter().map(|f| f.deletions.unwrap_or(0)).sum();
  let file_word = if files_changed == 1 { "file" } else { "files" };
  let ins_word = if additions == 1 { "insertion" } else { "insertions" };
  let del_word = if deletions == 1 { "deletion" } else { "deletions" };

  let mut parts: Vec<String> = Vec::new();

  if additions > 0 {
    parts.push(format!("{} {}(+)", additions, ins_word));
  }
  if deletions > 0 {
    parts.push(format!("{} {}(-)", deletions, del_word));
  }

  let suffix = if parts.is_empty() {
    String::new()
  } else {
    format!(", {}", parts.join(", "))
  };

  let diffstat_text = format!("{} {} changed{}", files_changed, file_word, suffix);

  let author_local = iso_in_tz(meta.at, context.tz);
  let commit_local = iso_in_tz(meta.ct, context.tz);
  let timezone = if context.tz.eq_ignore_ascii_case("local") {
    let dt = chrono::Local.timestamp_opt(meta.ct, 0).single().unwrap();
    dt.format("%Z").to_string()
  } else {
    context.tz.to_string()
  };

  let timestamps = Timestamps {
    author: meta.at,
    commit: meta.ct,
    author_local,
    commit_local,
    timezone,
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

  let patch_references = PatchReferences {
    embed: context.include_patch,
    git_show_cmd: format!("git show --patch --format= --no-color {}", meta.sha),
    local_patch_file: None,
    github: None,
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
    patch_references,
    patch_clipped: None,
    patch_lines: None,
    body_lines: None,
    github: None,
  };

  Ok(commit)
}

/// Processes a single git commit SHA and returns a fully populated `Commit` struct.
pub fn process_commit(sha: &str, context: &ProcessContext) -> Result<Commit> {
  let mut commit = build_commit_object(sha, context)?;

  if context.include_patch {
    let patch_text = gitio::commit_patch(context.repo, sha)?;
    let (maybe_patch, clipped) = clip_patch(patch_text, context.max_patch_bytes);
    commit.patch_lines = maybe_patch.map(|p| p.lines().map(String::from).collect());
    commit.patch_clipped = clipped;
  }

  if context.github_prs {
    enrich_with_github_prs(&mut commit, context.repo);
  }

  if !commit.body.is_empty() {
    commit.body_lines = Some(commit.body.lines().map(String::from).collect());
  }

  Ok(commit)
}

/// Save the full patch to disk and update `commit.patch_references.local_patch_file`.
///
/// Optimization: When this run already fetched the patch and it was not clipped
/// (i.e., `commit.patch_clipped == Some(false)`), write that in‑memory content
/// instead of spawning another `git show`. Fallback to `git show` when the patch
/// is clipped or not embedded.
pub fn save_patch_to_disk(commit: &mut Commit, repo: &str, directory_path: &Path) -> Result<()> {
  std::fs::create_dir_all(directory_path)?;
  let path = directory_path.join(format!("{}.patch", commit.short_sha));

  let content_from_memory = match (commit.patch_lines.as_ref(), commit.patch_clipped) {
    (Some(lines), Some(false)) => {
      let mut s = lines.join("\n");

      if !s.ends_with('\n') {
        s.push('\n');
      }

      Some(s)
    }
    _ => None,
  };

  let patch_content = if let Some(s) = content_from_memory {
    s
  } else {
    gitio::commit_patch(repo, &commit.sha)?
  };

  std::fs::write(&path, patch_content)?;
  commit.patch_references.local_patch_file = Some(path.to_string_lossy().to_string());

  Ok(())
}

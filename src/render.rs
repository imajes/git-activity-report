use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::Result;
use chrono::{Local, TimeZone, Utc};

use crate::enrich;
use crate::gitio;
use crate::model::{
  BranchItems, Commit, FileEntry, ManifestItem, PatchRef, Person, Range, RangeManifest, SimpleReport, Summary,
  Timestamps, UnmergedActivity,
};
use crate::util::{iso_in_tz, short_sha};

// --- Parameter Structs ---
// These remain unchanged as they define the public API.

#[derive(Debug)]
pub struct SimpleParams {
  pub repo: String,
  pub since: String,
  pub until: String,
  pub include_merges: bool,
  pub include_patch: bool,
  pub max_patch_bytes: usize,
  pub tz_local: bool,
  pub save_patches_dir: Option<String>,
  pub github_prs: bool,
}

#[derive(Debug)]
pub struct FullParams {
  pub repo: String,
  pub label: Option<String>,
  pub since: String,
  pub until: String,
  pub include_merges: bool,
  pub include_patch: bool,
  pub max_patch_bytes: usize,
  pub tz_local: bool,
  pub split_out: Option<String>,
  pub include_unmerged: bool,
  pub save_patches: bool,
  pub github_prs: bool,
}

// --- Internal Context for Processing ---
// An internal struct to pass common parameters to helper functions.

struct ProcessContext<'a> {
  repo: &'a str,
  tz_local: bool,
  github_prs: bool,
  include_patch: bool,
  max_patch_bytes: usize,
}

// --- Reusable Helper Functions ---

/// Builds a vector of `FileEntry` structs for a given commit.
fn build_file_entries(repo: &str, sha: &str) -> Result<Vec<FileEntry>> {
  let (num_list, num_map) = gitio::commit_numstat(repo, sha)?;
  let name_status_list = gitio::commit_name_status(repo, sha)?;

  if !name_status_list.is_empty() {
    // Prefer name-status for more detail (e.g., renames)
    Ok(
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
        .collect(),
    )
  } else {
    // Fallback to numstat for a simpler list of files
    Ok(
      num_list
        .into_iter()
        .map(|(path, additions, deletions)| FileEntry {
          file: path,
          status: "M".to_string(),
          old_path: None,
          additions,
          deletions,
        })
        .collect(),
    )
  }
}

/// Clips a patch text string to a maximum number of bytes, ensuring it doesn't split a UTF-8 character.
fn clip_patch(patch_text: String, max_bytes: usize) -> (Option<String>, Option<bool>) {
  if max_bytes == 0 {
    return (Some(patch_text), Some(false));
  }

  let bytes = patch_text.as_bytes();
  if bytes.len() <= max_bytes {
    return (Some(patch_text), Some(false));
  }

  let mut end = max_bytes;
  while end > 0 && (bytes[end] & 0xC0) == 0x80 {
    // Find the start of a UTF-8 character
    end -= 1;
  }

  (Some(String::from_utf8_lossy(&bytes[..end]).to_string()), Some(true))
}

/// Enriches a commit with its associated GitHub Pull Request info.
fn enrich_with_github_prs(commit: &mut Commit, repo: &str) {
  if let Ok(prs) = enrich::try_fetch_prs(repo, &commit.sha) {
    if let Some(first_pr) = prs.first() {
      commit.patch_ref.github_diff_url = first_pr.diff_url.clone();
      commit.patch_ref.github_patch_url = first_pr.patch_url.clone();
    }
    commit.github_prs = Some(prs);
  }
}
/// Saves a commit patch to a specified directory.
fn save_patch_to_disk(commit: &mut Commit, repo: &str, dir: &str) -> Result<()> {
  std::fs::create_dir_all(dir)?;
  let path_str = format!("{}/{}.patch", dir, commit.short_sha);
  let patch_content = gitio::commit_patch(repo, &commit.sha)?;
  std::fs::write(&path_str, patch_content)?;
  commit.patch_ref.local_patch_file = Some(path_str);
  Ok(())
}

fn build_commit_object(sha: &str, context: &ProcessContext) -> Result<Commit> {
  // 1. Build the core commit data from git metadata
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
  };

  Ok(commit)
}

/// Processes a single git commit SHA and returns a fully populated `Commit` struct.
fn process_commit(sha: &str, context: &ProcessContext) -> Result<Commit> {
  let mut commit = build_commit_object(sha, context)?;

  // 2. Act on the built data by enriching it further
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

  Ok(commit)
}

// --- Report Generation Logic ---

/// Generates a `SimpleReport` containing all commit data in memory.
pub fn run_simple(p: &SimpleParams) -> Result<SimpleReport> {
  let shas = gitio::rev_list(&p.repo, &p.since, &p.until, p.include_merges)?;
  let context = ProcessContext {
    repo: &p.repo,
    tz_local: p.tz_local,
    github_prs: p.github_prs,
    include_patch: p.include_patch,
    max_patch_bytes: p.max_patch_bytes,
  };

  let mut commits: Vec<Commit> = Vec::with_capacity(shas.len());
  let mut authors: BTreeMap<String, i64> = BTreeMap::new();
  let mut summary = Summary {
    additions: 0,
    deletions: 0,
    files_touched: 0,
  };
  let mut files_touched: HashSet<String> = HashSet::new();

  for sha in shas.iter() {
    let mut commit = process_commit(sha, &context)?;

    if let Some(dir) = &p.save_patches_dir {
      save_patch_to_disk(&mut commit, &p.repo, dir)?;
    }

    // Accumulate summary stats
    let author_key = format!("{} <{}>", commit.author.name, commit.author.email);
    *authors.entry(author_key).or_insert(0) += 1;

    for f in &commit.files {
      summary.additions += f.additions.unwrap_or(0);
      summary.deletions += f.deletions.unwrap_or(0);
      files_touched.insert(f.file.clone());
    }

    commits.push(commit);
  }

  summary.files_touched = files_touched.len();

  let report = SimpleReport {
    repo: p.repo.clone(),
    mode: "simple".into(),
    range: Range {
      since: p.since.clone(),
      until: p.until.clone(),
    },
    include_merges: p.include_merges,
    include_patch: p.include_patch,
    count: commits.len(),
    authors,
    summary,
    commits,
  };

  Ok(report)
}

/// Generates a `RangeManifest` and saves individual commit "shards" to disk.
pub fn run_full(p: &FullParams) -> Result<serde_json::Value> {
  let label = p.label.clone().unwrap_or_else(|| "window".to_string());
  let base_dir = p
    .split_out
    .clone()
    .unwrap_or_else(|| format!("activity-{}", Local::now().format("%Y%m%d-%H%M%S")));
  let subdir = Path::new(&base_dir).join(&label);
  std::fs::create_dir_all(&subdir)?;

  // Process the primary commit range
  let (items, summary, authors) = process_commit_range(p, &subdir, &label)?;

  // Optionally process unmerged branches
  let unmerged_activity = if p.include_unmerged {
    Some(process_unmerged_branches(p, &subdir, &label)?)
  } else {
    None
  };

  // Build and write the final manifest
  let manifest = RangeManifest {
    label: Some(label.clone()),
    range: Range {
      since: p.since.clone(),
      until: p.until.clone(),
    },
    repo: p.repo.clone(),
    include_merges: p.include_merges,
    include_patch: p.include_patch,
    mode: "full".into(),
    count: items.len(),
    authors,
    summary,
    items,
    unmerged_activity,
  };

  let manifest_path = Path::new(&base_dir).join(format!("manifest-{}.json", label));
  std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

  Ok(serde_json::json!({ "dir": base_dir, "manifest": format!("manifest-{}.json", label) }))
}

// --- `run_full` Sub-logic ---

/// Helper for `run_full` to process the main list of commits.
fn process_commit_range(
  p: &FullParams,
  subdir: &Path,
  label: &str,
) -> Result<(Vec<ManifestItem>, Summary, BTreeMap<String, i64>)> {
  let shas = gitio::rev_list(&p.repo, &p.since, &p.until, p.include_merges)?;
  let context = ProcessContext {
    repo: &p.repo,
    tz_local: p.tz_local,
    github_prs: p.github_prs,
    include_patch: p.include_patch,
    max_patch_bytes: p.max_patch_bytes,
  };

  let mut items = Vec::with_capacity(shas.len());
  let mut authors: BTreeMap<String, i64> = BTreeMap::new();
  let mut summary = Summary {
    additions: 0,
    deletions: 0,
    files_touched: 0,
  };
  let mut files_touched: HashSet<String> = HashSet::new();

  for sha in shas.iter() {
    let mut commit = process_commit(sha, &context)?;

    if p.save_patches {
      let patch_dir = subdir.join("patches");
      save_patch_to_disk(&mut commit, &p.repo, patch_dir.to_str().unwrap())?;
    }

    // Write commit shard to disk
    let fname = format_shard_name(commit.timestamps.commit, &commit.short_sha, p.tz_local);
    let shard_path = subdir.join(&fname);
    std::fs::write(&shard_path, serde_json::to_vec_pretty(&commit)?)?;

    // Accumulate manifest data
    items.push(ManifestItem {
      sha: commit.sha.clone(),
      file: Path::new(label).join(&fname).to_str().unwrap().to_string(),
      subject: commit.subject.clone(),
    });
    let author_key = format!("{} <{}>", commit.author.name, commit.author.email);
    *authors.entry(author_key).or_insert(0) += 1;
    for f in &commit.files {
      summary.additions += f.additions.unwrap_or(0);
      summary.deletions += f.deletions.unwrap_or(0);
      files_touched.insert(f.file.clone());
    }
  }

  summary.files_touched = files_touched.len();
  Ok((items, summary, authors))
}

/// Helper for `run_full` to process unmerged branches.
fn process_unmerged_branches(p: &FullParams, subdir: &Path, label: &str) -> Result<UnmergedActivity> {
  let current_branch = gitio::current_branch(&p.repo)?;
  let branches: Vec<String> = gitio::list_local_branches(&p.repo)?
    .into_iter()
    .filter(|b| Some(b.as_str()) != current_branch.as_deref())
    .collect();

  let context = ProcessContext {
    repo: &p.repo,
    tz_local: p.tz_local,
    github_prs: p.github_prs,
    include_patch: p.include_patch,
    max_patch_bytes: p.max_patch_bytes,
  };

  let mut ua = UnmergedActivity {
    branches_scanned: branches.len(),
    total_unmerged_commits: 0,
    branches: Vec::new(),
  };

  for br in branches {
    let unmerged_shas = gitio::unmerged_commits_in_range(&p.repo, &br, &p.since, &p.until, p.include_merges)?;
    if unmerged_shas.is_empty() {
      continue;
    }

    let br_dir_name = br.replace('/', "__");
    let br_dir = subdir.join("unmerged").join(&br_dir_name);

    let mut br_items = Vec::with_capacity(unmerged_shas.len());

    for sha in unmerged_shas.iter() {
      let mut commit = process_commit(sha, &context)?;

      if p.save_patches {
        let patch_dir = br_dir.join("patches");
        save_patch_to_disk(&mut commit, &p.repo, patch_dir.to_str().unwrap())?;
      }

      let fname = format_shard_name(commit.timestamps.commit, &commit.short_sha, p.tz_local);
      let shard_path = br_dir.join(&fname);
      std::fs::create_dir_all(shard_path.parent().unwrap())?;
      std::fs::write(&shard_path, serde_json::to_vec_pretty(&commit)?)?;

      br_items.push(ManifestItem {
        sha: commit.sha.clone(),
        file: Path::new(label)
          .join("unmerged")
          .join(&br_dir_name)
          .join(fname)
          .to_str()
          .unwrap()
          .to_string(),
        subject: commit.subject.clone(),
      });
    }

    let (behind, ahead) = gitio::branch_ahead_behind(&p.repo, &br)?;
    ua.total_unmerged_commits += br_items.len();
    ua.branches.push(BranchItems {
      name: br.clone(),
      merged_into_head: gitio::branch_merged_into_head(&p.repo, &br)?,
      ahead_of_head: ahead,
      behind_head: behind,
      items: br_items,
    });
  }

  Ok(ua)
}

/// Formats a file name for a commit shard based on its timestamp and SHA.
fn format_shard_name(epoch: i64, short_sha: &str, tz_local: bool) -> String {
  let (date, time) = if tz_local {
    let dt = Local.timestamp_opt(epoch, 0).single().unwrap();
    (dt.format("%Y.%m.%d").to_string(), dt.format("%H.%M").to_string())
  } else {
    let dt = Utc.timestamp_opt(epoch, 0).single().unwrap();
    (dt.format("%Y.%m.%d").to_string(), dt.format("%H.%M").to_string())
  };
  format!("{}-{}-{}.json", date, time, short_sha)
}

// --- Tests ---

#[cfg(test)]
mod tests {
  #[test]
  fn shard_name_utc_has_expected_pattern() {
    let name = super::format_shard_name(1_726_161_400, "abcdef123456", false); // 2024-09-12...
    assert!(name.ends_with("-abcdef123456.json"));
    assert_eq!(name.len(), "YYYY.MM.DD-HH.MM-abcdef123456.json".len());
  }
}

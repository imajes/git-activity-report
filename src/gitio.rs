// --- Git I/O Helpers ---
// Thin wrappers around `git` commands with small parsing utilities.

use crate::util::run_git;
use anyhow::Result;
use std::collections::HashMap;

type FileStat = (String, Option<i64>, Option<i64>);
type NumStatMap = HashMap<String, (Option<i64>, Option<i64>)>;
type NumStats = (Vec<FileStat>, NumStatMap);

/// Returns commit SHAs in the given window, earliest→latest (date order + reverse).
pub fn rev_list(repo: &str, since: &str, until: &str, include_merges: bool) -> Result<Vec<String>> {
  let mut args: Vec<String> = vec![
    "-c".into(),
    "log.showSignature=false".into(),
    "rev-list".into(),
    format!("--since={}", since),
    format!("--until={}", until),
    "--date-order".into(),
    "--reverse".into(),
    "HEAD".into(),
  ];

  if !include_merges {
    args.insert(4, "--no-merges".into());
  }

  let out = run_git(repo, &args)?;

  Ok(
    out
      .lines()
      .filter_map(|l| {
        let s = l.trim();

        if s.is_empty() { None } else { Some(s.to_string()) }
      })
      .collect(),
  )
}

/// Parsed metadata for a commit.
pub struct Meta {
  pub sha: String,
  pub parents: Vec<String>,
  pub author_name: String,
  pub author_email: String,
  pub author_date: String,
  pub committer_name: String,
  pub committer_email: String,
  pub committer_date: String,
  pub at: i64,
  pub ct: i64,
  pub subject: String,
  pub body: String,
}

// Mapping for the NUL (\0) separated pretty-format used in `commit_meta`.
//
// fmt = "%H%x00%P%x00%an%x00%ae%x00%ad%x00%cN%x00%cE%x00%cD%x00%at%x00%ct%x00%s%x00%b"
//
// Indices:
//   0 -> %H   full commit SHA (40 hex chars)
//   1 -> %P   parent SHAs (space-separated; may be empty)
//   2 -> %an  author name
//   3 -> %ae  author email
//   4 -> %ad  author date (formatted per --date)
//   5 -> %cN  committer name
//   6 -> %cE  committer email
//   7 -> %cD  committer date (RFC2822 when --date=iso-strict for %ad only)
//   8 -> %at  author timestamp (epoch seconds, UTC)
//   9 -> %ct  committer timestamp (epoch seconds, UTC)
//  10 -> %s   subject (first line / first sentence of commit message)
//  11 -> %b   body (rest of message, can be multi-line; may be empty)
const IDX_H: usize = 0;
const IDX_P: usize = 1;
const IDX_AN: usize = 2;
const IDX_AE: usize = 3;
const IDX_AD: usize = 4;
const IDX_CN: usize = 5;
const IDX_CE: usize = 6;
const IDX_CD: usize = 7;
const IDX_AT: usize = 8;
const IDX_CT: usize = 9;
const IDX_S: usize = 10;
const IDX_B: usize = 11;

/// Show commit metadata via `git show --no-patch` using a NUL-separated format.
pub fn commit_meta(repo: &str, sha: &str) -> Result<Meta> {
  let fmt = "%H%x00%P%x00%an%x00%ae%x00%ad%x00%cN%x00%cE%x00%cD%x00%at%x00%ct%x00%s%x00%b";
  let args: Vec<String> = vec![
    "show".into(),
    "--no-patch".into(),
    "--date=iso-strict".into(),
    format!("--pretty=format:{}", fmt),
    sha.into(),
  ];

  let out = run_git(repo, &args)?;

  let parts: Vec<&str> = out.split('\u{0}').collect();
  let get = |i: usize| -> String { parts.get(i).unwrap_or(&"").to_string() };
  // See index mapping above for details on each field.
  let at: i64 = get(IDX_AT).parse().unwrap_or(0);
  let ct: i64 = get(IDX_CT).parse().unwrap_or(0);

  Ok(Meta {
    sha: get(IDX_H),
    parents: if get(IDX_P).is_empty() {
      vec![]
    } else {
      get(IDX_P).split_whitespace().map(|s| s.to_string()).collect()
    },
    author_name: get(IDX_AN),
    author_email: get(IDX_AE),
    author_date: get(IDX_AD),
    committer_name: get(IDX_CN),
    committer_email: get(IDX_CE),
    committer_date: get(IDX_CD),
    at,
    ct,
    subject: get(IDX_S),
    body: get(IDX_B),
  })
}

/// Show per-file additions/deletions with `--numstat` (path, additions, deletions).
pub fn commit_numstat(repo: &str, sha: &str) -> Result<NumStats> {
  let args: Vec<String> = vec![
    "show".into(),
    "--numstat".into(),
    "--format=".into(),
    "--no-color".into(),
    sha.into(),
  ];

  let out = run_git(repo, &args)?;

  let mut files = Vec::new();
  let mut map: NumStatMap = HashMap::new();

  for line in out.lines() {
    let parts: Vec<&str> = line.split('\t').collect();

    if parts.len() != 3 {
      continue;
    }
    let to_int = |s: &str| -> Option<i64> { s.parse::<i64>().ok() };
    let a = to_int(parts[0]);
    let d = to_int(parts[1]);
    let path = parts[2].to_string();

    map.insert(path.clone(), (a, d));
    files.push((path, a, d));
  }
  Ok((files, map))
}

/// Show name-status with `--name-status -z` and parse into a vec of maps (status/file/old_path).
pub fn commit_name_status(repo: &str, sha: &str) -> Result<Vec<std::collections::HashMap<String, String>>> {
  // Use -z to split by NUL
  let args: Vec<String> = vec![
    "show".into(),
    "--name-status".into(),
    "-z".into(),
    "--format=".into(),
    "--no-color".into(),
    sha.into(),
  ];

  let out = run_git(repo, &args)?;

  let parts: Vec<&str> = out.split('\u{0}').collect();
  let mut res: Vec<std::collections::HashMap<String, String>> = Vec::new();
  let mut index = 0;

  while index < parts.len() && !parts[index].is_empty() {
    let code = parts[index];

    index += 1;
    if code.starts_with('R') || code.starts_with('C') {
      if index + 1 >= parts.len() {
        break;
      }
      let old_path_component = parts[index];
      let new_path_component = parts[index + 1];

      index += 2;
      let mut m = std::collections::HashMap::new();
      m.insert("status".to_string(), code.to_string());
      m.insert("old_path".to_string(), old_path_component.to_string());
      m.insert("file".to_string(), new_path_component.to_string());

      res.push(m);
    } else {
      if index >= parts.len() {
        break;
      }
      let path_component = parts[index];

      index += 1;
      if path_component.is_empty() {
        continue;
      }
      let mut m = std::collections::HashMap::new();
      m.insert("status".to_string(), code.to_string());
      m.insert("file".to_string(), path_component.to_string());

      res.push(m);
    }
  }
  Ok(res)
}

/// Show shortstat and return the trailing summary line.
pub fn commit_shortstat(repo: &str, sha: &str) -> Result<String> {
  let args: Vec<String> = vec![
    "show".into(),
    "--shortstat".into(),
    "--format=".into(),
    "--no-color".into(),
    sha.into(),
  ];

  let out = run_git(repo, &args)?;

  let s = out.lines().last().unwrap_or("").trim().to_string();
  Ok(s)
}

/// Show full patch as a unified diff text.
pub fn commit_patch(repo: &str, sha: &str) -> Result<String> {
  let args: Vec<String> = vec![
    "show".into(),
    "--patch".into(),
    "--format=".into(),
    "--no-color".into(),
    sha.into(),
  ];

  run_git(repo, &args)
}

/// Current branch name or None when HEAD detached.
pub fn current_branch(repo: &str) -> Result<Option<String>> {
  let out = run_git(repo, &["rev-parse".into(), "--abbrev-ref".into(), "HEAD".into()])?;
  let name = out.trim();

  if name == "HEAD" {
    Ok(None)
  } else {
    Ok(Some(name.to_string()))
  }
}

/// List local branches as short names.
pub fn list_local_branches(repo: &str) -> Result<Vec<String>> {
  let out = run_git(
    repo,
    &[
      "for-each-ref".into(),
      "refs/heads".into(),
      "--format=%(refname:short)".into(),
    ],
  )?;

  Ok(
    out
      .lines()
      .map(|l| l.trim())
      .filter(|s| !s.is_empty())
      .map(|s| s.to_string())
      .collect(),
  )
}

/// Ahead/behind counts comparing HEAD to `branch` (`--left-right --count`).
pub fn branch_ahead_behind(repo: &str, branch: &str) -> Result<(Option<i64>, Option<i64>)> {
  let out = run_git(
    repo,
    &[
      "rev-list".into(),
      "--left-right".into(),
      "--count".into(),
      format!("HEAD...{}", branch),
    ],
  )?;

  let parts: Vec<&str> = out.split_whitespace().collect();

  if parts.len() == 2 {
    Ok((parts[0].parse::<i64>().ok(), parts[1].parse::<i64>().ok()))
  } else {
    Ok((None, None))
  }
}

/// Whether `branch` is merged into HEAD (exit code of `merge-base --is-ancestor`).
pub fn branch_merged_into_head(repo: &str, branch: &str) -> Result<Option<bool>> {
  // Use merge-base --is-ancestor (exit code indicates result)
  let args: Vec<String> = vec![
    "merge-base".into(),
    "--is-ancestor".into(),
    branch.into(),
    "HEAD".into(),
  ];

  let res = std::process::Command::new("git").args(&args).current_dir(repo).status();

  match res {
    Ok(st) => Ok(Some(st.success())),
    Err(_) => Ok(None),
  }
}

/// Commits in branch but not in HEAD across a window (earliest→latest).
pub fn unmerged_commits_in_range(
  repo: &str,
  branch: &str,
  since: &str,
  until: &str,
  include_merges: bool,
) -> Result<Vec<String>> {
  let mut args: Vec<String> = vec![
    "-c".into(),
    "log.showSignature=false".into(),
    "rev-list".into(),
    branch.into(),
    "^HEAD".into(),
    format!("--since={}", since),
    format!("--until={}", until),
    "--date-order".into(),
    "--reverse".into(),
  ];

  if !include_merges {
    args.insert(6, "--no-merges".into());
  }

  let out = run_git(repo, &args)?;
  Ok(
    out
      .lines()
      .map(|l| l.trim())
      .filter(|s| !s.is_empty())
      .map(|s| s.to_string())
      .collect(),
  )
}

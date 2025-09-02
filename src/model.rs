// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Define the JSON model (commits, ranges, manifests, GitHub PRs) shared by rendering and enrichment
// role: model/types
// outputs: Serializable structs with stable field names and optional enrichment fields
// invariants: JSON field shapes match Python schema v2; additive fields only; timestamps shape unchanged
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person {
  pub name: String,
  pub email: String,
  pub date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Timestamps {
  pub author: i64,
  pub commit: i64,
  pub author_local: String,
  pub commit_local: String,
  pub timezone: String, // "local" | "utc"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
  pub file: String,
  pub status: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub old_path: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub additions: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub deletions: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchRef {
  pub embed: bool,
  pub git_show_cmd: String,
  pub local_patch_file: Option<String>,
  pub github_diff_url: Option<String>,
  pub github_patch_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Commit {
  pub sha: String,
  pub short_sha: String,
  pub parents: Vec<String>,
  pub author: Person,
  pub committer: Person,
  pub timestamps: Timestamps,
  pub subject: String,
  pub body: String,
  pub files: Vec<FileEntry>,
  pub diffstat_text: String,
  pub patch_ref: PatchRef,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub patch: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub patch_clipped: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub github_prs: Option<Vec<GithubPullRequest>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub body_lines: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
  pub additions: i64,
  pub deletions: i64,
  pub files_touched: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleReport {
  pub repo: String,
  pub range: Range,
  pub include_merges: bool,
  pub include_patch: bool,
  pub count: usize,
  pub authors: std::collections::BTreeMap<String, i64>,
  pub summary: Summary,
  pub commits: Vec<Commit>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_requests: Option<Vec<GithubPullRequest>>, // aggregated PRs across commits
  #[serde(skip_serializing_if = "Option::is_none")]
  pub items: Option<Vec<ManifestItem>>, // present when split-apart
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Range {
  pub since: String,
  pub until: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubUser {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub login: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubPullRequest {
  pub number: i64,
  pub title: String,
  pub state: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub body: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub created_at: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub merged_at: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub closed_at: Option<String>,
  pub html_url: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub diff_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub patch_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub user: Option<GithubUser>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub submitter: Option<GithubUser>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub approver: Option<GithubUser>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub head: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub base: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commits: Option<Vec<PullRequestCommit>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequestCommit {
  pub sha: String,
  pub short_sha: String,
  pub subject: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManifestItem {
  pub sha: String,
  pub file: String,
  pub subject: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BranchItems {
  pub name: String,
  pub merged_into_head: Option<bool>,
  pub ahead_of_head: Option<i64>,
  pub behind_head: Option<i64>,
  pub items: Vec<ManifestItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnmergedActivity {
  pub branches_scanned: usize,
  pub total_unmerged_commits: usize,
  pub branches: Vec<BranchItems>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RangeManifest {
  pub label: Option<String>,
  pub range: Range,
  pub repo: String,
  pub include_merges: bool,
  pub include_patch: bool,
  pub count: usize,
  pub authors: std::collections::BTreeMap<String, i64>,
  pub summary: Summary,
  pub items: Vec<ManifestItem>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unmerged_activity: Option<UnmergedActivity>,
}

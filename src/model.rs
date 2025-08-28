use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person { pub name: String, pub email: String, pub date: String }

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
    #[serde(skip_serializing_if = "Option::is_none")] pub old_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub additions: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub deletions: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchRef {
    pub embed: bool,
    pub git_show_cmd: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub local_patch_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub github_diff_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub github_patch_url: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")] pub patch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub patch_clipped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub github_prs: Option<Vec<GithubPr>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Summary { pub additions: i64, pub deletions: i64, pub files_touched: usize }

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleReport {
    pub repo: String,
    pub mode: String, // "simple"
    pub range: Range,
    pub include_merges: bool,
    pub include_patch: bool,
    pub count: usize,
    pub authors: std::collections::BTreeMap<String, i64>,
    pub summary: Summary,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Range { pub since: String, pub until: String }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubUser { #[serde(skip_serializing_if = "Option::is_none")] pub login: Option<String> }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubPr {
    pub number: i64,
    pub title: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub merged_at: Option<String>,
    pub html_url: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub diff_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub patch_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub user: Option<GithubUser>,
    #[serde(skip_serializing_if = "Option::is_none")] pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub base: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManifestItem { pub sha: String, pub file: String, pub subject: String }

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
    pub mode: String, // "full"
    pub count: usize,
    pub authors: std::collections::BTreeMap<String, i64>,
    pub summary: Summary,
    pub items: Vec<ManifestItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmerged_activity: Option<UnmergedActivity>,
}

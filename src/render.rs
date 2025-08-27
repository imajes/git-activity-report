use anyhow::Result;
use std::collections::{BTreeMap, HashMap, HashSet};
use crate::gitio;
use crate::model::{Commit, FileEntry, PatchRef, Person, Range, SimpleReport, Summary, Timestamps, RangeManifest, ManifestItem, UnmergedActivity, BranchItems};
use chrono::TimeZone;

#[derive(Debug)]
pub struct SimpleParams {
    pub repo: String,
    pub since: String,
    pub until: String,
    pub include_merges: bool,
    pub include_patch: bool,
    pub max_patch_bytes: usize,
    pub tz_local: bool,
}

fn short_sha(full: &str) -> String { full.chars().take(12).collect() }

fn iso_in_tz(epoch: i64, tz_local: bool) -> String {
    if tz_local {
        let dt = chrono::Local.timestamp_opt(epoch, 0).single().unwrap();
        dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    } else {
        let dt = chrono::Utc.timestamp_opt(epoch, 0).single().unwrap();
        dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }
}

pub fn run_simple(p: &SimpleParams) -> Result<SimpleReport> {
    let repo = p.repo.clone();
    let shas = gitio::rev_list(&repo, &p.since, &p.until, p.include_merges)?;

    let mut commits: Vec<Commit> = Vec::new();
    let mut authors: BTreeMap<String, i64> = BTreeMap::new();
    let mut additions: i64 = 0;
    let mut deletions: i64 = 0;
    let mut files_touched: HashSet<String> = HashSet::new();

    for sha in shas.iter() {
        let meta = gitio::commit_meta(&repo, sha)?;
        let (num_list, num_map) = gitio::commit_numstat(&repo, sha)?;
        let ns = gitio::commit_name_status(&repo, sha)?;
        let shortstat = gitio::commit_shortstat(&repo, sha)?;

        let mut files: Vec<FileEntry> = Vec::new();
        if !ns.is_empty() {
            for entry in ns {
                let path = entry.get("file").cloned().unwrap_or_default();
                let adds_dels = num_map.get(&path).cloned().unwrap_or((None, None));
                let mut fe = FileEntry{
                    file: path.clone(),
                    status: entry.get("status").cloned().unwrap_or_else(|| "M".to_string()),
                    old_path: entry.get("old_path").cloned(),
                    additions: adds_dels.0,
                    deletions: adds_dels.1,
                };
                files.push(fe);
            }
        } else {
            for (path, a, d) in num_list {
                files.push(FileEntry{ file: path.clone(), status: "M".to_string(), old_path: None, additions: a, deletions: d });
            }
        }

        // Accumulate summary
        for f in &files {
            if let Some(a) = f.additions { additions += a; }
            if let Some(d) = f.deletions { deletions += d; }
            files_touched.insert(f.file.clone());
        }
        let author_key = format!("{} <{}>", meta.author_name, meta.author_email);
        *authors.entry(author_key).or_insert(0) += 1;

        let tz_label = if p.tz_local { "local" } else { "utc" };
        let timestamps = Timestamps{
            author: meta.at,
            commit: meta.ct,
            author_local: iso_in_tz(meta.at, p.tz_local),
            commit_local: iso_in_tz(meta.ct, p.tz_local),
            timezone: tz_label.to_string(),
        };
        let patch_ref = PatchRef{
            embed: p.include_patch,
            git_show_cmd: vec!["git".into(), "show".into(), "--patch".into(), "--format=".into(), "--no-color".into(), meta.sha.clone()],
            local_patch_file: None,
            github_diff_url: None,
            github_patch_url: None,
        };
        let mut commit = Commit{
            sha: meta.sha.clone(),
            short_sha: short_sha(&meta.sha),
            parents: meta.parents.clone(),
            author: Person{ name: meta.author_name, email: meta.author_email, date: meta.author_date },
            committer: Person{ name: meta.committer_name, email: meta.committer_email, date: meta.committer_date },
            timestamps,
            subject: meta.subject,
            body: meta.body,
            files,
            diffstat_text: shortstat,
            patch_ref,
            patch: None,
            patch_clipped: None,
        };
        if p.include_patch {
            let txt = gitio::commit_patch(&repo, sha)?;
            if p.max_patch_bytes == 0 { commit.patch = Some(txt); commit.patch_clipped = Some(false); }
            else {
                let bytes = txt.as_bytes();
                if bytes.len() <= p.max_patch_bytes { commit.patch = Some(txt); commit.patch_clipped = Some(false); }
                else {
                    // Clip at UTF-8 boundary
                    let mut end = p.max_patch_bytes;
                    while end > 0 && (bytes[end-1] & 0b1100_0000) == 0b1000_0000 { end -= 1; }
                    commit.patch = Some(String::from_utf8_lossy(&bytes[..end]).to_string());
                    commit.patch_clipped = Some(true);
                }
            }
        }

        commits.push(commit);
    }

    let report = SimpleReport{
        repo,
        mode: "simple".into(),
        range: Range{ since: p.since.clone(), until: p.until.clone() },
        include_merges: p.include_merges,
        include_patch: p.include_patch,
        count: commits.len(),
        authors,
        summary: Summary{ additions, deletions, files_touched: files_touched.len() },
        commits,
    };
    Ok(report)
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
}

fn format_shard_name(epoch: i64, short_sha: &str, tz_local: bool) -> String {
    let (date, time) = if tz_local {
        let dt = chrono::Local.timestamp_opt(epoch, 0).single().unwrap();
        (dt.format("%Y.%m.%d").to_string(), dt.format("%H.%M").to_string())
    } else {
        let dt = chrono::Utc.timestamp_opt(epoch, 0).single().unwrap();
        (dt.format("%Y.%m.%d").to_string(), dt.format("%H.%M").to_string())
    };
    format!("{}-{}-{}.json", date, time, short_sha)
}

fn label_for_window(label_opt: Option<String>) -> String { label_opt.unwrap_or_else(|| "window".to_string()) }

pub fn run_full(p: &FullParams) -> Result<serde_json::Value> {
    let label = label_for_window(p.label.clone());
    let base = if let Some(dir) = &p.split_out { dir.clone() } else {
        let now = chrono::Local::now();
        format!("activity-{}", now.format("%Y%m%d-%H%M%S"))
    };
    let subdir = format!("{}/{}", base, label);
    std::fs::create_dir_all(&subdir)?;

    let shas = gitio::rev_list(&p.repo, &p.since, &p.until, p.include_merges)?;
    let mut authors: BTreeMap<String, i64> = BTreeMap::new();
    let mut adds: i64 = 0; let mut dels: i64 = 0; let mut files_touched: HashSet<String> = HashSet::new();
    let mut items: Vec<ManifestItem> = Vec::new();

    for sha in shas.iter() {
        let meta = gitio::commit_meta(&p.repo, sha)?;
        let (num_list, num_map) = gitio::commit_numstat(&p.repo, sha)?;
        let ns = gitio::commit_name_status(&p.repo, sha)?;
        let shortstat = gitio::commit_shortstat(&p.repo, sha)?;
        let mut files: Vec<FileEntry> = Vec::new();
        if !ns.is_empty() {
            for entry in ns {
                let path = entry.get("file").cloned().unwrap_or_default();
                let adds_dels = num_map.get(&path).cloned().unwrap_or((None, None));
                files.push(FileEntry{ file: path.clone(), status: entry.get("status").cloned().unwrap_or_else(|| "M".to_string()), old_path: entry.get("old_path").cloned(), additions: adds_dels.0, deletions: adds_dels.1 });
            }
        } else {
            for (path, a, d) in num_list { files.push(FileEntry{ file: path.clone(), status: "M".to_string(), old_path: None, additions: a, deletions: d }); }
        }
        for f in &files { if let Some(a)=f.additions{adds+=a}; if let Some(d)=f.deletions{dels+=d}; files_touched.insert(f.file.clone()); }
        let author_key = format!("{} <{}>", meta.author_name, meta.author_email);
        *authors.entry(author_key).or_insert(0) += 1;
        let timestamps = Timestamps{ author: meta.at, commit: meta.ct, author_local: iso_in_tz(meta.at, p.tz_local), commit_local: iso_in_tz(meta.ct, p.tz_local), timezone: if p.tz_local {"local".into()} else {"utc".into()} };
        let patch_ref = PatchRef{ embed: p.include_patch, git_show_cmd: vec!["git".into(), "show".into(), "--patch".into(), "--format=".into(), "--no-color".into(), meta.sha.clone()], local_patch_file: None, github_diff_url: None, github_patch_url: None };
        let mut commit = Commit{ sha: meta.sha.clone(), short_sha: short_sha(&meta.sha), parents: meta.parents.clone(), author: Person{ name: meta.author_name, email: meta.author_email, date: meta.author_date }, committer: Person{ name: meta.committer_name, email: meta.committer_email, date: meta.committer_date }, timestamps, subject: meta.subject, body: meta.body, files, diffstat_text: shortstat, patch_ref, patch: None, patch_clipped: None };
        if p.include_patch {
            let txt = gitio::commit_patch(&p.repo, sha)?;
            if p.max_patch_bytes == 0 { commit.patch = Some(txt); commit.patch_clipped = Some(false); }
            else { let bytes = txt.as_bytes(); if bytes.len() <= p.max_patch_bytes { commit.patch = Some(txt); commit.patch_clipped = Some(false); } else { let mut end = p.max_patch_bytes; while end>0 && (bytes[end-1]&0b1100_0000)==0b1000_0000 { end-=1; } commit.patch = Some(String::from_utf8_lossy(&bytes[..end]).to_string()); commit.patch_clipped = Some(true); } }
        }
        let fname = format_shard_name(commit.timestamps.commit, &commit.short_sha, p.tz_local);
        let shard_path = format!("{}/{}", subdir, fname);
        std::fs::write(&shard_path, serde_json::to_vec_pretty(&commit)?)?;
        items.push(ManifestItem{ sha: commit.sha.clone(), file: format!("{}/{}", label, fname), subject: commit.subject.clone() });
    }

    let mut unmerged: Option<UnmergedActivity> = None;
    if p.include_unmerged {
        let cur = gitio::current_branch(&p.repo)?;
        let branches: Vec<String> = gitio::list_local_branches(&p.repo)?.into_iter().filter(|b| Some(b.clone()) != cur).collect();
        let mut ua = UnmergedActivity{ branches_scanned: branches.len(), total_unmerged_commits: 0, branches: Vec::new() };
        for br in branches {
            let uniq = gitio::unmerged_commits_in_range(&p.repo, &br, &p.since, &p.until, p.include_merges)?;
            if uniq.is_empty() { continue; }
            let merged = gitio::branch_merged_into_head(&p.repo, &br)?;
            let (behind, ahead) = gitio::branch_ahead_behind(&p.repo, &br)?;
            let br_dir = format!("{}/unmerged/{}", subdir, br.replace('/', "__"));
            std::fs::create_dir_all(&br_dir)?;
            let mut br_items: Vec<ManifestItem> = Vec::new();
            for sha in uniq.iter() {
                let meta = gitio::commit_meta(&p.repo, sha)?;
                let (num_list, num_map) = gitio::commit_numstat(&p.repo, sha)?;
                let ns = gitio::commit_name_status(&p.repo, sha)?;
                let shortstat = gitio::commit_shortstat(&p.repo, sha)?;
                let mut files: Vec<FileEntry> = Vec::new();
                if !ns.is_empty() {
                    for entry in ns { let path = entry.get("file").cloned().unwrap_or_default(); let adds_dels = num_map.get(&path).cloned().unwrap_or((None,None)); files.push(FileEntry{ file: path.clone(), status: entry.get("status").cloned().unwrap_or_else(||"M".into()), old_path: entry.get("old_path").cloned(), additions: adds_dels.0, deletions: adds_dels.1 }); }
                } else { for (path,a,d) in num_list { files.push(FileEntry{ file: path.clone(), status: "M".into(), old_path: None, additions: a, deletions: d }); } }
                let timestamps = Timestamps{ author: meta.at, commit: meta.ct, author_local: iso_in_tz(meta.at, p.tz_local), commit_local: iso_in_tz(meta.ct, p.tz_local), timezone: if p.tz_local {"local".into()} else {"utc".into()} };
                let patch_ref = PatchRef{ embed: p.include_patch, git_show_cmd: vec!["git".into(), "show".into(), "--patch".into(), "--format=".into(), "--no-color".into(), meta.sha.clone()], local_patch_file: None, github_diff_url: None, github_patch_url: None };
                let commit = Commit{ sha: meta.sha.clone(), short_sha: short_sha(&meta.sha), parents: meta.parents.clone(), author: Person{ name: meta.author_name, email: meta.author_email, date: meta.author_date }, committer: Person{ name: meta.committer_name, email: meta.committer_email, date: meta.committer_date }, timestamps, subject: meta.subject, body: meta.body, files, diffstat_text: shortstat, patch_ref, patch: None, patch_clipped: None };
                let fname = format_shard_name(commit.timestamps.commit, &commit.short_sha, p.tz_local);
                let shard_path = format!("{}/{}", br_dir, fname);
                std::fs::write(&shard_path, serde_json::to_vec_pretty(&commit)?)?;
                br_items.push(ManifestItem{ sha: commit.sha.clone(), file: format!("{}/unmerged/{}/{}", label, br.replace('/', "__"), fname), subject: commit.subject.clone() });
            }
            ua.total_unmerged_commits += br_items.len();
            ua.branches.push(BranchItems{ name: br, merged_into_head: merged, ahead_of_head: ahead, behind_head: behind, items: br_items });
        }
        unmerged = Some(ua);
    }

    let manifest = RangeManifest{
        label: Some(label.clone()),
        range: Range{ since: p.since.clone(), until: p.until.clone() },
        repo: p.repo.clone(),
        include_merges: p.include_merges,
        include_patch: p.include_patch,
        mode: "full".into(),
        count: items.len(),
        authors,
        summary: Summary{ additions: adds, deletions: dels, files_touched: files_touched.len() },
        items,
        unmerged_activity: unmerged,
    };
    let manifest_path = format!("{}/manifest-{}.json", base, label);
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(serde_json::json!({"dir": base, "manifest": format!("manifest-{}.json", label)}))
}

use anyhow::Result;
use crate::util::run_git;

pub fn rev_list(repo: &str, since: &str, until: &str, include_merges: bool) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![
        "-c".into(), "log.showSignature=false".into(),
        "rev-list".into(),
        format!("--since={}", since),
        format!("--until={}", until),
        "--date-order".into(),
        "--reverse".into(),
        "HEAD".into(),
    ];
    if !include_merges { args.insert(4, "--no-merges".into()); }
    let out = run_git(repo, &args)?;
    Ok(out.lines().filter_map(|l| { let s = l.trim(); if s.is_empty() {None} else {Some(s.to_string())} }).collect())
}

pub struct Meta { pub sha: String, pub parents: Vec<String>, pub author_name: String, pub author_email: String, pub author_date: String, pub committer_name: String, pub committer_email: String, pub committer_date: String, pub at: i64, pub ct: i64, pub subject: String, pub body: String }

pub fn commit_meta(repo: &str, sha: &str) -> Result<Meta> {
    let fmt = "%H%x00%P%x00%an%x00%ae%x00%ad%x00%cN%x00%cE%x00%cD%x00%at%x00%ct%x00%s%x00%b";
    let args: Vec<String> = vec![
        "show".into(), "--no-patch".into(), "--date=iso-strict".into(), format!("--pretty=format:{}", fmt), sha.into()
    ];
    let out = run_git(repo, &args)?;
    let parts: Vec<&str> = out.split('\u{0}').collect();
    let get = |i: usize| -> String { parts.get(i).unwrap_or(&"").to_string() };
    let at: i64 = get(9).parse().unwrap_or(0);
    let ct: i64 = get(10).parse().unwrap_or(0);
    Ok(Meta{ sha: get(0), parents: if get(1).is_empty(){vec![]} else {get(1).split_whitespace().map(|s| s.to_string()).collect()}, author_name: get(2), author_email: get(3), author_date: get(4), committer_name: get(5), committer_email: get(6), committer_date: get(7), at, ct, subject: get(11), body: get(12) })
}

pub fn commit_numstat(repo: &str, sha: &str) -> Result<(Vec<(String, Option<i64>, Option<i64>)>, std::collections::HashMap<String, (Option<i64>, Option<i64>)>)> {
    let args: Vec<String> = vec!["show".into(), "--numstat".into(), "--format=".into(), "--no-color".into(), sha.into()];
    let out = run_git(repo, &args)?;
    let mut files = Vec::new();
    let mut map = std::collections::HashMap::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 3 { continue; }
        let to_int = |s: &str| -> Option<i64> { s.parse::<i64>().ok() };
        let a = to_int(parts[0]);
        let d = to_int(parts[1]);
        let path = parts[2].to_string();
        map.insert(path.clone(), (a, d));
        files.push((path, a, d));
    }
    Ok((files, map))
}

pub fn commit_name_status(repo: &str, sha: &str) -> Result<Vec<std::collections::HashMap<String, String>>> {
    // Use -z to split by NUL
    let args: Vec<String> = vec!["show".into(), "--name-status".into(), "-z".into(), "--format=".into(), "--no-color".into(), sha.into()];
    let out = run_git(repo, &args)?;
    let parts: Vec<&str> = out.split('\u{0}').collect();
    let mut res: Vec<std::collections::HashMap<String,String>> = Vec::new();
    let mut i = 0;
    while i < parts.len() && !parts[i].is_empty() {
        let code = parts[i];
        i += 1;
        if code.starts_with('R') || code.starts_with('C') {
            if i + 1 >= parts.len() { break; }
            let oldp = parts[i];
            let newp = parts[i+1];
            i += 2;
            let mut m = std::collections::HashMap::new();
            m.insert("status".to_string(), code.to_string());
            m.insert("old_path".to_string(), oldp.to_string());
            m.insert("file".to_string(), newp.to_string());
            res.push(m);
        } else {
            if i >= parts.len() { break; }
            let p = parts[i];
            i += 1;
            if p.is_empty() { continue; }
            let mut m = std::collections::HashMap::new();
            m.insert("status".to_string(), code.to_string());
            m.insert("file".to_string(), p.to_string());
            res.push(m);
        }
    }
    Ok(res)
}

pub fn commit_shortstat(repo: &str, sha: &str) -> Result<String> {
    let args: Vec<String> = vec!["show".into(), "--shortstat".into(), "--format=".into(), "--no-color".into(), sha.into()];
    let out = run_git(repo, &args)?;
    let s = out.lines().last().unwrap_or("").trim().to_string();
    Ok(s)
}

pub fn commit_patch(repo: &str, sha: &str) -> Result<String> {
    let args: Vec<String> = vec!["show".into(), "--patch".into(), "--format=".into(), "--no-color".into(), sha.into()];
    run_git(repo, &args)
}

pub fn current_branch(repo: &str) -> Result<Option<String>> {
    let out = run_git(repo, &vec!["rev-parse".into(), "--abbrev-ref".into(), "HEAD".into()])?;
    let name = out.trim();
    if name == "HEAD" { Ok(None) } else { Ok(Some(name.to_string())) }
}

pub fn list_local_branches(repo: &str) -> Result<Vec<String>> {
    let out = run_git(repo, &vec!["for-each-ref".into(), "refs/heads".into(), "--format=%(refname:short)".into()])?;
    Ok(out.lines().map(|l| l.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect())
}

pub fn branch_ahead_behind(repo: &str, branch: &str) -> Result<(Option<i64>, Option<i64>)> {
    let out = run_git(repo, &vec!["rev-list".into(), "--left-right".into(), "--count".into(), format!("HEAD...{}", branch)])?;
    let parts: Vec<&str> = out.trim().split_whitespace().collect();
    if parts.len() == 2 {
        Ok((parts[0].parse::<i64>().ok(), parts[1].parse::<i64>().ok()))
    } else { Ok((None, None)) }
}

pub fn branch_merged_into_head(repo: &str, branch: &str) -> Result<Option<bool>> {
    // Use merge-base --is-ancestor (exit code indicates result)
    let args: Vec<String> = vec!["merge-base".into(), "--is-ancestor".into(), branch.into(), "HEAD".into()];
    let res = std::process::Command::new("git").args(&args).current_dir(repo).status();
    match res { Ok(st) => Ok(Some(st.success())), Err(_) => Ok(None) }
}

pub fn unmerged_commits_in_range(repo: &str, branch: &str, since: &str, until: &str, include_merges: bool) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![
        "-c".into(), "log.showSignature=false".into(),
        "rev-list".into(),
        branch.into(), "^HEAD".into(),
        format!("--since={}", since), format!("--until={}", until),
        "--date-order".into(), "--reverse".into(),
    ];
    if !include_merges { args.insert(6, "--no-merges".into()); }
    let out = run_git(repo, &args)?;
    Ok(out.lines().map(|l| l.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect())
}

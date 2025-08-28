use anyhow::Result;
use crate::util::run_git;
use crate::model::GithubPr;

fn parse_origin_github(repo: &str) -> Option<(String, String)> {
    if let Ok(url) = run_git(repo, &vec!["config".into(), "--get".into(), "remote.origin.url".into()]) {
        let u = url.trim();
        let re1 = regex::Regex::new(r"^(?:git@github\.com:|https?://github\.com/)([^/]+)/([^/]+?)(?:\.git)?$").unwrap();
        if let Some(c) = re1.captures(u) {
            return Some((c.get(1).unwrap().as_str().to_string(), c.get(2).unwrap().as_str().to_string()));
        }
    }
    None
}

pub fn try_fetch_prs(repo: &str, sha: &str) -> Result<Vec<GithubPr>> {
    let mut out: Vec<GithubPr> = Vec::new();
    let Some((owner, name)) = parse_origin_github(repo) else { return Ok(out) };
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        // Use ureq with token
        let url = format!("https://api.github.com/repos/{}/{}/commits/{}/pulls", owner, name, sha);
        let agent = ureq::AgentBuilder::new().build();
        let res = agent
            .get(&url)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "git-activity-report")
            .set("Authorization", &format!("Bearer {}", token))
            .call();
        if let Ok(resp) = res { if let Ok(v) = resp.into_json::<serde_json::Value>() {
            if let Some(arr) = v.as_array() {
                for pr in arr {
                    let html = pr.get("html_url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    out.push(GithubPr{
                        number: pr.get("number").and_then(|x| x.as_i64()).unwrap_or(0),
                        title: pr.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        state: pr.get("state").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        created_at: pr.get("created_at").and_then(|x| x.as_str()).map(|s| s.to_string()),
                        merged_at: pr.get("merged_at").and_then(|x| x.as_str()).map(|s| s.to_string()),
                        html_url: html.clone(),
                        diff_url: if html.is_empty(){None}else{Some(format!("{}.diff", html))},
                        patch_url: if html.is_empty(){None}else{Some(format!("{}.patch", html))},
                        user: pr.get("user").and_then(|u| u.get("login")).and_then(|l| l.as_str()).map(|login| crate::model::GithubUser{ login: Some(login.to_string()) }),
                        head: pr.get("head").and_then(|x| x.get("ref")).and_then(|s| s.as_str()).map(|s| s.to_string()),
                        base: pr.get("base").and_then(|x| x.get("ref")).and_then(|s| s.as_str()).map(|s| s.to_string()),
                    });
                }
            }
        }}
    }
    Ok(out)
}


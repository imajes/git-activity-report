use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn canonicalize_lossy<P: AsRef<Path>>(p: P) -> String {
  let p = p.as_ref();
  let pb: PathBuf = match std::fs::canonicalize(p) {
    Ok(x) => x,
    Err(_) => match std::env::current_dir() {
      Ok(cwd) => cwd.join(p),
      Err(_) => PathBuf::from(p),
    },
  };
  pb.to_string_lossy().to_string()
}

pub fn run_git(repo: &str, args: &[String]) -> Result<String> {
  let out = Command::new("git")
    .args(args)
    .current_dir(repo)
    .output()
    .with_context(|| format!("spawning git {:?}", args))?;
  if out.status.success() {
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
  } else {
    let stderr = String::from_utf8_lossy(&out.stderr);
    anyhow::bail!("git {:?} failed: {}", args, stderr)
  }
}

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use chrono::{Local, SecondsFormat, TimeZone, Utc};

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

/// Generates a short 12-character SHA from a full one.
pub fn short_sha(full: &str) -> String {
  full.chars().take(12).collect()
}

/// Formats a Unix epoch timestamp into an RFC3339 string in the specified timezone.
pub fn iso_in_tz(epoch: i64, tz_local: bool) -> String {
  if tz_local {
    let dt = Local.timestamp_opt(epoch, 0).single().unwrap();
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
  } else {
    let dt = Utc.timestamp_opt(epoch, 0).single().unwrap();
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
  }
}

// JSON extension helpers are in `crate::ext::serde_json`.

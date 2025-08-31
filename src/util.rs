use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, SecondsFormat, TimeZone, Utc};
use clap::CommandFactory;

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

/// Returns the effective "now" given an optional override.
///
/// When `override_now` is `Some`, that instant is returned; otherwise
/// the current local time is used. Centralizes our handling of test
/// determinism without sprinkling `Local::now()` throughout the code.
pub fn effective_now(override_now: Option<DateTime<Local>>) -> DateTime<Local> {
  override_now.unwrap_or_else(Local::now)
}

/// Render a section-1 man page for a clap `CommandFactory` implementor.
/// Returns the troff content as a UTF-8 string.
pub fn render_man_page<T: CommandFactory>() -> anyhow::Result<String> {
  let cmd = T::command();
  let man = clap_mangen::Man::new(cmd);
  let mut buf: Vec<u8> = Vec::new();
  man.render(&mut buf)?;
  Ok(String::from_utf8_lossy(&buf).to_string())
}

// JSON extension helpers are in `crate::ext::serde_json`.

#[cfg(test)]
mod tests {
  use super::*;
  use clap::Parser;

  #[test]
  fn short_sha_truncates() {
    assert_eq!(short_sha("abcdef1234567890"), "abcdef123456");
    assert_eq!(short_sha("abc"), "abc");
  }

  #[test]
  fn iso_formats_utc_and_local() {
    // 2024-09-12T00:30:00Z (epoch 1726101000)
    let iso_utc = iso_in_tz(1_726_101_000, false);
    assert!(iso_utc.ends_with('Z'));

    let iso_local = iso_in_tz(1_726_101_000, true);
    assert!(iso_local.ends_with('Z') || iso_local.contains('+') || iso_local.contains('-'));
  }

  #[test]
  fn canonicalize_returns_abs_path() {
    let abs = canonicalize_lossy(".");
    assert!(abs.starts_with('/'));
  }

  #[test]
  fn run_git_failure_is_error() {
    let err = run_git(".", &["definitely-not-a-real-subcommand".into()]).unwrap_err();
    let msg = format!("{:#}", err);
    assert!(msg.contains("git"));
  }

  #[derive(Parser, Debug)]
  #[command(name = "dummy", version, about = "Dummy CLI", long_about = None)]
  struct DummyCli;

  #[test]
  fn render_man_page_produces_troff_text() {
    let page = render_man_page::<DummyCli>().expect("render manpage");
    assert!(page.contains(".TH"));
    assert!(page.to_lowercase().contains("dummy"));
  }
}

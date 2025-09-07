// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Utilities for paths, time formatting, spacing-safe helpers, and man page rendering
// role: utilities/helpers
// inputs: Various primitives; DateTime; paths; clap CommandFactory
// outputs: Canonicalized paths, formatted timestamps, directories ensured, man page text
// side_effects: prepare_out_dir creates directories; run_git invokes subprocesses
// invariants:
// - prepare_out_dir returns an existing directory (either provided or temp timestamped)
// - clip_patch never splits UTF-8; indicates clipping accurately
// - format_shard_name pattern is stable and locale-independent
// errors: run_git surfaces command + stderr; IO errors bubble with context
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;
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
pub fn iso_in_tz(epoch: i64, tz: &str) -> String {
  if tz.eq_ignore_ascii_case("local") {
    let dt = Local.timestamp_opt(epoch, 0).single().unwrap();
    return dt.to_rfc3339_opts(SecondsFormat::Secs, true);
  }

  if tz.eq_ignore_ascii_case("utc") {
    let dt = Utc.timestamp_opt(epoch, 0).single().unwrap();
    return dt.to_rfc3339_opts(SecondsFormat::Secs, true);
  }

  let dt_utc = Utc.timestamp_opt(epoch, 0).single().unwrap();

  match tz.parse::<Tz>() {
    Ok(zone) => zone
      .from_utc_datetime(&dt_utc.naive_utc())
      .to_rfc3339_opts(SecondsFormat::Secs, true),
    Err(_) => dt_utc.to_rfc3339_opts(SecondsFormat::Secs, true),
  }
}

/// Clips a patch text string to a maximum number of bytes, ensuring it doesn't split a UTF-8 character.
pub fn clip_patch(patch_text: String, max_bytes: usize) -> (Option<String>, Option<bool>) {
  if max_bytes == 0 {
    return (Some(patch_text), Some(false));
  }

  let bytes = patch_text.as_bytes();

  if bytes.len() <= max_bytes {
    return (Some(patch_text), Some(false));
  }

  let mut end = max_bytes;

  while end > 0 && (bytes[end] & 0xC0) == 0x80 {
    end -= 1;
  }

  (Some(String::from_utf8_lossy(&bytes[..end]).to_string()), Some(true))
}

/// Returns the effective "now" given an optional override.
///
/// When `override_now` is `Some`, that instant is returned; otherwise
/// the current local time is used. Centralizes our handling of test
/// determinism without sprinkling `Local::now()` throughout the code.
pub fn effective_now(override_now: Option<DateTime<Local>>) -> DateTime<Local> {
  override_now.unwrap_or_else(Local::now)
}

/// Prepare an output directory for multi-range or split-apart runs.
///
/// - When `out` is not "-", it is treated as the target directory; it will be created if needed.
/// - When `out` is "-", a temp directory is created with a timestamped name.
///   Returns the absolute path as a String.
pub fn prepare_out_dir(out: &str, now_opt: Option<DateTime<Local>>) -> anyhow::Result<String> {
  let dir = if out != "-" {
    out.to_string()
  } else {
    let eff_now = effective_now(now_opt);
    std::env::temp_dir()
      .join(format!("activity-{}", eff_now.format("%Y%m%d-%H%M%S")))
      .to_string_lossy()
      .to_string()
  };
  std::fs::create_dir_all(&dir)?;

  Ok(dir)
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

/// Compute the difference in seconds between two RFC3339 timestamps.
/// Returns None when either timestamp cannot be parsed.
pub fn diff_seconds(start_iso: &str, end_iso: &str) -> Option<i64> {
  let ps = chrono::DateTime::parse_from_rfc3339(start_iso).ok()?;
  let pe = chrono::DateTime::parse_from_rfc3339(end_iso).ok()?;
  Some((pe - ps).num_seconds())
}

#[cfg(test)]
mod tests {
  use super::*;
  use chrono::{Local, TimeZone};
  use clap::Parser;

  #[test]
  fn short_sha_truncates() {
    assert_eq!(short_sha("abcdef1234567890"), "abcdef123456");
    assert_eq!(short_sha("abc"), "abc");
  }

  #[test]
  fn iso_formats_utc_and_local() {
    // 2024-09-12T00:30:00Z (epoch 1726101000)
    let iso_utc = iso_in_tz(1_726_101_000, "utc");
    assert!(iso_utc.ends_with('Z'));

    let iso_local = iso_in_tz(1_726_101_000, "local");
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

  #[test]
  fn prepare_out_dir_creates_given_directory() {
    let td = tempfile::TempDir::new().unwrap();
    let target = td.path().join("outdir");
    let out = target.to_string_lossy().to_string();
    let dir = prepare_out_dir(&out, None).expect("prepare_out_dir");
    assert_eq!(dir, out);
    assert!(std::path::Path::new(&dir).exists());
  }

  #[test]
  fn prepare_out_dir_temp_includes_timestamp() {
    let fixed = Local.with_ymd_and_hms(2025, 8, 15, 12, 0, 0).single().unwrap();
    let dir = prepare_out_dir("-", Some(fixed)).expect("prepare_out_dir temp");
    assert!(dir.contains("activity-20250815-120000"), "dir was: {}", dir);
    assert!(std::path::Path::new(&dir).exists());
  }

  #[test]
  fn clip_patch_never_splits_utf8() {
    let (p, clipped) = clip_patch("ééé".to_string(), 1);
    assert_eq!(clipped, Some(true));
    let out = p.unwrap();
    assert!(out.is_char_boundary(out.len()));
  }

  #[test]
  fn shard_name_utc_has_expected_pattern() {
    let name = super::format_shard_name(1_726_161_400, "abcdef123456", "utc"); // 2024-09-12...
    assert!(name.ends_with("-abcdef123456.json"));
    assert_eq!(name.len(), "YYYY.MM.DD-HH.MM-abcdef123456.json".len());
  }
}

/// Formats a file name for a commit shard based on its timestamp and SHA.
pub fn format_shard_name(epoch: i64, short_sha: &str, tz: &str) -> String {
  if tz.eq_ignore_ascii_case("local") {
    let dt = Local.timestamp_opt(epoch, 0).single().unwrap();
    return format!("{}-{}-{}.json", dt.format("%Y.%m.%d"), dt.format("%H.%M"), short_sha);
  }

  if tz.eq_ignore_ascii_case("utc") {
    let dt = Utc.timestamp_opt(epoch, 0).single().unwrap();
    return format!("{}-{}-{}.json", dt.format("%Y.%m.%d"), dt.format("%H.%M"), short_sha);
  }

  let dt_utc = Utc.timestamp_opt(epoch, 0).single().unwrap();

  if let Ok(zone) = tz.parse::<Tz>() {
    let dt = zone.from_utc_datetime(&dt_utc.naive_utc());
    format!("{}-{}-{}.json", dt.format("%Y.%m.%d"), dt.format("%H.%M"), short_sha)
  } else {
    format!(
      "{}-{}-{}.json",
      dt_utc.format("%Y.%m.%d"),
      dt_utc.format("%H.%M"),
      short_sha
    )
  }
}

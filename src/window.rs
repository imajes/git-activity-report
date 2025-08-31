use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

// Windowing-related types live here to keep main focused.

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Tz {
  Local,
  Utc,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum WindowSpec {
  Month { ym: String },
  ForPhrase { phrase: String },
  SinceUntil { since: String, until: String },
}

pub fn month_bounds(year_month: &str) -> Result<(String, String)> {
  let parts: Vec<&str> = year_month.split('-').collect();

  if parts.len() != 2 {
    bail!("invalid --month, expected YYYY-MM");
  }
  let y: i32 = parts[0].parse().context("parsing year in --month")?;
  let m: i32 = parts[1].parse().context("parsing month in --month")?;

  if !(1..=12).contains(&m) {
    bail!("invalid month in --month");
  }
  let next_y = if m == 12 { y + 1 } else { y };
  let next_m = if m == 12 { 1 } else { m + 1 };

  Ok((
    format!("{y:04}-{m:02}-01T00:00:00"),
    format!("{next_y:04}-{next_m:02}-01T00:00:00"),
  ))
}

pub fn compute_window_strings(window: &WindowSpec) -> Result<(String, String)> {
  match window {
    WindowSpec::SinceUntil { since, until } => Ok((since.clone(), until.clone())),
    WindowSpec::Month { ym } => month_bounds(ym),
    WindowSpec::ForPhrase { .. } => bail!(
      "--for phrase windows not implemented in Rust port yet; use --month or --since/--until"
    ),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn month_bounds_basic() {
    let (s, u) = month_bounds("2025-08").unwrap();
    assert_eq!(s, "2025-08-01T00:00:00");
    assert_eq!(u, "2025-09-01T00:00:00");
  }

  #[test]
  fn compute_window_since_until_passthrough() {
    let win = WindowSpec::SinceUntil { since: "2025-08-01".into(), until: "2025-09-01".into() };
    let (s, u) = compute_window_strings(&win).unwrap();
    assert_eq!(s, "2025-08-01");
    assert_eq!(u, "2025-09-01");
  }

  #[test]
  fn compute_window_for_phrase_not_supported() {
    let win = WindowSpec::ForPhrase { phrase: "last week".into() };
    assert!(compute_window_strings(&win).is_err());
  }
}

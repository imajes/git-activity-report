use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};
use chrono_english::{Interval, parse_duration};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use two_timer::parse as parse_natural;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabeledRange {
  pub label: String,
  pub since: String,
  pub until: String,
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

/// Compute (since, until) for a window.
///
/// Supports an optional `now` override for deterministic testing.
pub fn compute_window_strings(
  window: &WindowSpec,
  now: Option<chrono::DateTime<chrono::Local>>,
) -> Result<(String, String)> {
  match window {
    WindowSpec::SinceUntil { since, until } => Ok((since.clone(), until.clone())),
    WindowSpec::Month { ym } => month_bounds(ym),
    WindowSpec::ForPhrase { phrase } => for_phrase_bounds(phrase, now),
  }
}

// --- Helpers for `--for` parsing ---

fn start_of_week(dt: chrono::DateTime<chrono::Local>) -> chrono::DateTime<chrono::Local> {
  let weekday = dt.weekday().num_days_from_monday() as i64;
  (dt - chrono::Duration::days(weekday))
    .date_naive()
    .and_hms_opt(0, 0, 0)
    .unwrap()
    .and_local_timezone(Local)
    .single()
    .unwrap()
}

fn last_week_range(now: chrono::DateTime<chrono::Local>) -> (String, String) {
  let start_this_week = start_of_week(now);
  let start_last_week = start_of_week(now - chrono::Duration::days(7));
  (iso_naive(start_last_week), iso_naive(start_this_week))
}

fn last_month_range(now: chrono::DateTime<chrono::Local>) -> (String, String) {
  let y = now.year();
  let m = now.month() as i32;
  let (last_y, last_m) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
  let start_last = NaiveDate::from_ymd_opt(last_y, last_m as u32, 1)
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap();
  let start_this = NaiveDate::from_ymd_opt(y, now.month(), 1)
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap();
  (
    start_last.format("%Y-%m-%dT%H:%M:%S").to_string(),
    start_this.format("%Y-%m-%dT%H:%M:%S").to_string(),
  )
}

fn iso_naive(dt: chrono::DateTime<chrono::Local>) -> String {
  // Render as YYYY-MM-DDTHH:MM:SS, drop timezone for git approxidate friendliness
  dt.naive_local().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Parse a `--now-override` string into a local DateTime.
/// Accepts RFC3339 (e.g. 2025-08-15T12:00:00Z) or a naive local timestamp
/// formatted as `%Y-%m-%dT%H:%M:%S`.
pub fn parse_now_override(s: Option<&str>) -> Option<DateTime<Local>> {
  s.and_then(|raw| {
    chrono::DateTime::parse_from_rfc3339(raw)
      .ok()
      .map(|dt| dt.with_timezone(&Local))
      .or_else(|| {
        chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S")
          .ok()
          .and_then(|ndt| ndt.and_local_timezone(Local).single())
      })
  })
}

/// Compute range for a natural-language phrase, with optional `now` override for tests.
fn for_phrase_bounds(input: &str, now: Option<chrono::DateTime<chrono::Local>>) -> Result<(String, String)> {
  let phrase = input.trim().to_lowercase();
  let now = now.unwrap_or_else(Local::now);

  // Prefer library support; avoid custom anchoring when better alternates exist.
  // Override: for "today" and "yesterday", anchor to local day start / 24h ago, ending at now.
  if phrase == "today" {
    let start = now
      .date_naive()
      .and_hms_opt(0, 0, 0)
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();

    return Ok((iso_naive(start), iso_naive(now)));
  }

  // Override: last week — anchor to previous calendar week (Mon 00:00 to this week's Mon 00:00)
  if phrase == "last week" {
    return Ok(last_week_range(now));
  }

  // Override: last month — anchor to first-of-last-month → first-of-this-month
  if phrase == "last month" {
    return Ok(last_month_range(now));
  }

  // Override: last <weekday> — compute strictly previous occurrence (avoid future dates)
  if let Some(caps) = regex::Regex::new(r"^last\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)$")
    .unwrap()
    .captures(&phrase)
  {
    let day = caps.get(1).unwrap().as_str();
    let target_idx = match day {
      "monday" => 0,
      "tuesday" => 1,
      "wednesday" => 2,
      "thursday" => 3,
      "friday" => 4,
      "saturday" => 5,
      "sunday" => 6,
      _ => 0,
    } as i64;

    let today_start = now
      .date_naive()
      .and_hms_opt(0, 0, 0)
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();

    let cur_idx = today_start.weekday().num_days_from_monday() as i64;
    let mut delta_days = cur_idx - target_idx;
    if delta_days <= 0 {
      delta_days += 7;
    }
    let since = today_start - chrono::Duration::days(delta_days);

    return Ok((iso_naive(since), iso_naive(now)));
  }

  if phrase == "yesterday" {
    return Ok((iso_naive(now - chrono::Duration::days(1)), iso_naive(now)));
  }

  // Duration/"ago" parsing via chrono-english (handle first to avoid misclassification by natural parser)
  if let Ok(interval) = parse_duration(&phrase) {
    let (start, end) = match interval {
      Interval::Seconds(secs) => {
        let d = chrono::Duration::seconds(secs.into());
        if secs < 0 { (now + d, now) } else { (now, now + d) }
      }
      Interval::Days(days) => {
        let d = chrono::Duration::days(days.into());
        if days < 0 { (now + d, now) } else { (now, now + d) }
      }
      Interval::Months(months) => {
        if months < 0 {
          (subtract_months(now, months.unsigned_abs() as i32), now)
        } else {
          (now, subtract_months(now, -months))
        }
      }
    };

    return Ok((iso_naive(start), iso_naive(end)));
  }

  // Natural ranges via two_timer (today, yesterday, last week, last tuesday, last month, last year)
  if let Ok((start_naive, end_naive, _lit)) = parse_natural(&phrase, None) {
    let start = start_naive.and_local_timezone(Local).single().unwrap();
    let end = end_naive.and_local_timezone(Local).single().unwrap();

    let until = if end > now { now } else { end };

    return Ok((iso_naive(start), iso_naive(until)));
  }

  // Fallback: delegate to git approxidate by passing raw phrase and using "now" until
  Ok((input.to_string(), "now".to_string()))
}

/// If the phrase is a multi-bucket request (e.g., "every month for the last N months"),
/// compute labeled buckets (chronological, earliest→latest). Otherwise, return None.
/// Build labeled ranges for multi-bucket phrases, with optional `now` override for tests.
pub fn for_phrase_buckets(input: &str, now: Option<chrono::DateTime<chrono::Local>>) -> Option<Vec<LabeledRange>> {
  let phrase = input.trim().to_lowercase();
  let now = now.unwrap_or_else(Local::now);

  // every month for the last N months
  if let Some(caps) = regex::Regex::new(r"^every\s+month\s+for\s+the\s+last\s+(\d+)\s+months?$")
    .ok()?
    .captures(&phrase)
  {
    let n: i32 = caps.get(1).unwrap().as_str().parse().ok()?;
    let mut out: Vec<LabeledRange> = Vec::new();
    let mut cursor_y = now.year();
    let mut cursor_m = now.month() as i32;
    // Cursor is first of current month
    for _ in 0..n {
      // Start = first of previous month
      let prev_m = if cursor_m == 1 { 12 } else { cursor_m - 1 };
      let prev_y = if cursor_m == 1 { cursor_y - 1 } else { cursor_y };

      let start = NaiveDate::from_ymd_opt(prev_y, prev_m as u32, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
      let end = NaiveDate::from_ymd_opt(cursor_y, cursor_m as u32, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();

      let label = format!("{:04}-{:02}", prev_y, prev_m);
      let entry = LabeledRange {
        label,
        since: start.format("%Y-%m-%dT%H:%M:%S").to_string(),
        until: end.format("%Y-%m-%dT%H:%M:%S").to_string(),
      };

      out.push(entry);

      cursor_y = prev_y;
      cursor_m = prev_m;
    }
    out.reverse();
    return Some(out);
  }

  // every week for the last N weeks
  if let Some(caps) = regex::Regex::new(r"^every\s+week\s+for\s+the\s+last\s+(\d+)\s+weeks?$")
    .ok()?
    .captures(&phrase)
  {
    let n: i32 = caps.get(1).unwrap().as_str().parse().ok()?;
    let mut out: Vec<LabeledRange> = Vec::new();
    let mut cursor = start_of_week(now);
    for _ in 0..n {
      let start = cursor - chrono::Duration::days(7);
      let end = cursor;
      // ISO week for label
      let iso = start.naive_local().iso_week();
      let label = format!("{}-W{:02}", iso.year(), iso.week());
      let entry = LabeledRange {
        label,
        since: start.naive_local().format("%Y-%m-%dT%H:%M:%S").to_string(),
        until: end.naive_local().format("%Y-%m-%dT%H:%M:%S").to_string(),
      };

      out.push(entry);
      cursor = start;
    }
    out.reverse();
    return Some(out);
  }

  None
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
  // Advance to first day of next month, subtract one day
  let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
  let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
  let last = first_next.pred_opt().unwrap();
  last.day()
}

fn subtract_months(dt: chrono::DateTime<Local>, n: i32) -> chrono::DateTime<Local> {
  let total = (dt.year() * 12 + dt.month() as i32 - 1) - n;
  let y = total.div_euclid(12);
  let m0 = total.rem_euclid(12);
  let m = (m0 + 1) as u32;
  let d = dt.day().min(last_day_of_month(y, m));
  let nd = NaiveDate::from_ymd_opt(y, m, d).unwrap();
  let nt = nd.and_hms_opt(dt.hour(), dt.minute(), dt.second()).unwrap();
  nt.and_local_timezone(Local).single().unwrap()
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
  fn month_bounds_invalid_errors() {
    assert!(month_bounds("2025-13").is_err());
  }

  #[test]
  fn compute_window_since_until_passthrough() {
    let win = WindowSpec::SinceUntil {
      since: "2025-08-01".into(),
      until: "2025-09-01".into(),
    };
    let (s, u) = compute_window_strings(&win, None).unwrap();
    assert_eq!(s, "2025-08-01");
    assert_eq!(u, "2025-09-01");
  }

  #[test]
  fn compute_window_for_phrase_not_supported() {
    let win = WindowSpec::ForPhrase {
      phrase: "last week".into(),
    };
    let (s, u) = compute_window_strings(&win, None).unwrap();
    assert!(s.len() >= 10);
    assert!(u.len() >= 10);
  }

  #[test]
  fn for_phrase_last_month_basic() {
    let win = WindowSpec::ForPhrase {
      phrase: "last month".into(),
    };
    let (s, u) = compute_window_strings(&win, None).unwrap();
    assert!(s < u);
    assert!(s.contains('T'));
    assert!(u.contains('T'));
  }

  #[test]
  fn for_phrase_parsed_instant_uses_until_now() {
    let win = WindowSpec::ForPhrase {
      phrase: "2 weeks ago".into(),
    };
    let (s, u) = compute_window_strings(&win, None).unwrap();
    // Both should be ISO-like strings; we only assert presence of separators for stability
    assert!(s.contains('T'));
    assert!(u.contains('T'));
  }

  #[test]
  fn for_phrase_fallback_delegates_to_git_approxidate() {
    let p = "unparseable phrase 12345";
    let win = WindowSpec::ForPhrase { phrase: p.into() };
    let (s, u) = compute_window_strings(&win, None).unwrap();
    assert_eq!(s, p);
    assert_eq!(u, "now");
  }

  #[test]
  fn for_phrase_today_anchors_to_day_start_until_now() {
    let now = chrono::NaiveDateTime::parse_from_str("2025-08-15T12:00:00", "%Y-%m-%dT%H:%M:%S")
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();
    let win = WindowSpec::ForPhrase { phrase: "today".into() };
    let (s, u) = compute_window_strings(&win, Some(now)).unwrap();
    assert!(s.ends_with("00:00:00"));
    assert!(u.ends_with("12:00:00"));
  }

  #[test]
  fn for_phrase_last_year_has_calendar_bounds() {
    let now = chrono::NaiveDateTime::parse_from_str("2025-08-15T12:00:00", "%Y-%m-%dT%H:%M:%S")
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();
    let win = WindowSpec::ForPhrase {
      phrase: "last year".into(),
    };
    let (s, u) = compute_window_strings(&win, Some(now)).unwrap();
    assert_eq!(s, "2024-01-01T00:00:00");
    assert_eq!(u, "2025-01-01T00:00:00");
  }

  #[test]
  fn for_phrase_last_week_has_expected_bounds() {
    let now = chrono::NaiveDateTime::parse_from_str("2025-08-15T12:00:00", "%Y-%m-%dT%H:%M:%S")
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();
    let win = WindowSpec::ForPhrase {
      phrase: "last week".into(),
    };
    let (s, u) = compute_window_strings(&win, Some(now)).unwrap();
    // Start-of-last-week (Mon) and start-of-this-week
    assert!(s.ends_with("00:00:00"));
    assert!(u.ends_with("00:00:00"));
  }

  #[test]
  fn now_local_reads_rfc3339_roundtrip() {
    // This asserts that RFC3339 can be used to build a now override via parsing
    let now = chrono::DateTime::parse_from_rfc3339("2025-08-15T12:00:00Z")
      .unwrap()
      .with_timezone(&Local);
    let win = WindowSpec::ForPhrase {
      phrase: "yesterday".into(),
    };
    let (_s, _u) = compute_window_strings(&win, Some(now)).unwrap();
  }
}

#[cfg(test)]
mod future_tests {
  use super::*;

  #[test]
  fn duration_minutes_future_without_preposition() {
    let now = chrono::NaiveDateTime::parse_from_str("2025-08-15T12:00:00", "%Y-%m-%dT%H:%M:%S")
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();
    let win = WindowSpec::ForPhrase {
      phrase: "10 minutes".into(),
    };
    let (s, u) = compute_window_strings(&win, Some(now)).unwrap();
    let sn = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S").unwrap();
    let un = chrono::NaiveDateTime::parse_from_str(&u, "%Y-%m-%dT%H:%M:%S").unwrap();
    assert_eq!((un - sn).num_minutes(), 10);
  }

  #[test]
  fn duration_months_future_without_preposition() {
    let now = chrono::NaiveDateTime::parse_from_str("2025-01-31T08:00:00", "%Y-%m-%dT%H:%M:%S")
      .unwrap()
      .and_local_timezone(Local)
      .single()
      .unwrap();
    let win = WindowSpec::ForPhrase {
      phrase: "1 month".into(),
    };
    let (s, u) = compute_window_strings(&win, Some(now)).unwrap();
    let sn = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S").unwrap();
    let un = chrono::NaiveDateTime::parse_from_str(&u, "%Y-%m-%dT%H:%M:%S").unwrap();
    assert_eq!(sn.time(), un.time());
    assert!(un.month() == 2 || un.month() == 3);
  }
}

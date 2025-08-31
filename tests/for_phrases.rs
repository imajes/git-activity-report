mod common;
use assert_cmd::Command;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, Timelike, Weekday};
use regex::Regex;

fn iso(dt: chrono::DateTime<Local>) -> String {
  dt.naive_local().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn start_of_week(dt: chrono::DateTime<Local>) -> chrono::DateTime<Local> {
  let weekday = dt.weekday().num_days_from_monday() as i64;
  (dt - chrono::Duration::days(weekday))
    .date_naive()
    .and_hms_opt(0, 0, 0)
    .unwrap()
    .and_local_timezone(Local)
    .single()
    .unwrap()
}

fn last_month_bounds(now: chrono::DateTime<Local>) -> (String, String) {
  let y = now.year();
  let m = now.month() as i32;
  let (last_y, last_m) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
  let start_last = NaiveDate::from_ymd_opt(last_y, last_m as u32, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
  let start_this = NaiveDate::from_ymd_opt(y, now.month(), 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
  (
    start_last.format("%Y-%m-%dT%H:%M:%S").to_string(),
    start_this.format("%Y-%m-%dT%H:%M:%S").to_string(),
  )
}

#[test]
fn many_for_phrases_should_match_expected_ranges() {
  let repo = common::fixture_repo();
  let repo_path = repo.to_str().unwrap();

  // Freeze "now" for deterministic expectations in both CLI and our math
  let fixed_now_str = "2025-08-15T12:00:00";
  // Pin TZ to UTC to avoid local variance
  std::env::set_var("TZ", "UTC");
  let fixed_now = NaiveDateTime::parse_from_str(fixed_now_str, "%Y-%m-%dT%H:%M:%S")
    .unwrap()
    .and_local_timezone(Local)
    .single()
    .unwrap();

  let phrases = vec![
    "yesterday",
    "today",
    "1 hour ago",
    "12 hours ago",
    "90 minutes ago",
    "2 days ago",
    "3 days ago",
    "10 days ago",
    "last week",
    "1 week ago",
    "2 weeks ago",
    "4 weeks ago",
    "last month",
    "1 month ago",
    "2 months ago",
    "3 months ago",
    "last tuesday",
    "last friday",
    "last sunday",
    "last year",
  ];

  for p in phrases {
    let out = Command::cargo_bin("git-activity-report")
      .unwrap()
      .args(["--simple", "--for", p, "--repo", repo_path, "--tz", "utc", "--now-override", fixed_now_str])
      .output()
      .unwrap();

    assert!(out.status.success(), "phrase failed: {}", p);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let since = v["range"]["since"].as_str().unwrap().to_string();
    let until = v["range"]["until"].as_str().unwrap().to_string();

    // Compute expected
    let (exp_since, exp_until) = if p == "last week" {
      let sow = start_of_week(fixed_now);
      (iso(sow - chrono::Duration::days(7)), iso(sow))
    } else if p == "last month" {
      last_month_bounds(fixed_now)
    } else if p == "last year" {
      let y = fixed_now.year();
      (
        format!("{:04}-01-01T00:00:00", y - 1),
        format!("{:04}-01-01T00:00:00", y),
      )
    } else if p == "today" {
      let start = fixed_now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .unwrap();
      (iso(start), iso(fixed_now))
    } else if p.starts_with("last ") {
      let wd = match p.split_whitespace().nth(1).unwrap() {
        "monday" => Weekday::Mon,
        "tuesday" => Weekday::Tue,
        "wednesday" => Weekday::Wed,
        "thursday" => Weekday::Thu,
        "friday" => Weekday::Fri,
        "saturday" => Weekday::Sat,
        "sunday" => Weekday::Sun,
        _ => Weekday::Mon,
      };
      let today_start = fixed_now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .unwrap();
      let cur_idx = today_start.weekday().num_days_from_monday() as i64;
      let target_idx = wd.num_days_from_monday() as i64;
      let mut delta_days = cur_idx - target_idx;
      if delta_days <= 0 { delta_days += 7; }
      let since = today_start - chrono::Duration::days(delta_days);
      (iso(since), iso(fixed_now))
    } else {
      // Parse N units ago for minutes/hours/days/weeks/months; treat "yesterday" as 1 day ago
      let s = p.to_lowercase();
      let re = Regex::new(r"^(\d+)\s+(minutes?|hours?|days?|weeks?)\s+ago$").unwrap();
      if s == "yesterday" {
        (iso(fixed_now - chrono::Duration::days(1)), iso(fixed_now))
      } else if let Some(c) = Regex::new(r"^(\d+)\s+months?\s+ago$").unwrap().captures(&s) {
        let n: i32 = c.get(1).unwrap().as_str().parse().unwrap();
        let since = subtract_months(fixed_now, n);
        (iso(since), iso(fixed_now))
      } else if let Some(c) = re.captures(&s) {
        let n: i64 = c.get(1).unwrap().as_str().parse().unwrap();
        let unit = c.get(2).unwrap().as_str();
        let dur = match unit {
          "minute" | "minutes" => chrono::Duration::minutes(n),
          "hour" | "hours" => chrono::Duration::hours(n),
          "day" | "days" => chrono::Duration::days(n),
          "week" | "weeks" => chrono::Duration::weeks(n as i64),
          _ => unreachable!(),
        };
        (iso(fixed_now - dur), iso(fixed_now))
      } else {
        panic!("Unhandled phrase in test expectations: {}", p)
      }
    };

    assert_eq!(since, exp_since, "phrase: {}", p);
    assert_eq!(until, exp_until, "phrase: {}", p);
  }
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
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

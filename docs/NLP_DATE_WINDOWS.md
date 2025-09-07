## QUESTION

ok, think hard about this -- do you know of a rust crate/library that does efficient natural language parsing, that could turn something like "every week for 3 months" into buckets of time?

## ANSWER

Short answer: there isn’t a single “do-it-all” Rust crate that cleanly turns “every week for 3 months” into a ready-made list of time buckets. The practical pattern is parser + recurrence engine:

1. Parse the natural-language bits (start, duration, cadence).
   Good candidates:
   • rustling-ontology — industrial NLU that extracts Time, Duration, TimeInterval, numbers, etc. Heavy compile, fast at runtime. CLI examples include parsing “tomorrow morning.” ￼ ￼
   • timewarp — English/German NL date parser built with pest; lighter footprint than rustling. ￼ ￼
   • date_time_parser_multi_language — aims for phrases like “tomorrow” / “in three weeks” across multiple languages. (You’ll likely still add your own “every N” grammar.) ￼
   • chrono-english — rock-solid for “next friday 8pm” and relative durations, but explicitly not full NL parsing (won’t understand recurrence by itself). ￼
   • dateparser / natural-date-parser — parse many date formats and some relative phrases; think “get a DateTime,” not “model a schedule.” ￼ ￼

2. Expand the recurrence with an RFC 5545 engine.
   Use rrule (Rust implementation of iCalendar RRULE). You can build from a string (it implements FromStr) and then expand occurrences or query “between” two dates. It returns an iterator of instances, which you can turn into buckets. ￼ Spec reference: RRULE in RFC 5545. ￼

Concrete approach for “every week for 3 months”
• Parse an anchor (default to now or a given start) and a window (3 months). A light grammar can map “every week” → FREQ=WEEKLY;INTERVAL=1.
• Build an RRULE string like:
DTSTART:20250906T150000Z\nRRULE:FREQ=WEEKLY;UNTIL=20251206T150000Z
Then feed it to rrule and expand occurrences; pair each date with + 1 week (or with the next occurrence) to form your buckets. The crate exposes all() and range filters like after(...).before(...) for efficient expansion. ￼

Why the split?
Natural-language understanding and standards-compliant recurrence are different beasts. Trying to teach a date parser about all of RFC 5545 (BYDAY, BYSETPOS, DST edge cases, etc.) is a yak farm; rrule already handles that. Let the parser extract (start, frequency, interval, horizon) and pass a clean RRULE to the engine.

Tips that save pain
• Use chrono-tz time zones and keep DTSTART localized; rrule returns dates in the same zone, which avoids DST gotchas. ￼
• Distinguish “for 3 months” (calendar horizon → UNTIL) vs “3 months” as a count of occurrences (compute COUNT instead).
• If your inputs are mostly “every {unit} for {duration},” a tiny nom/pest grammar (~200 LOC) may outperform general NLU while keeping dependencies lean.
• Need multi-lingual? Prefer rustling-ontology or timewarp, then normalize to RRULE.

If you want, I can sketch a minimal crate layout (parser trait + RRULE builder + bucket iterator) that plugs into your timezone handling and test harness.

⸻

Truthfulness audit
• Facts: chrono-english supports limited patterns and not full NL parsing; it parses durations and relative dates. rrule implements RFC 5545, supports FromStr, RRuleSet::all, after/before filters, and returns occurrences in the same timezone as DTSTART. rustling-ontology parses Time/Duration and has a CLI; it compiles slowly. Confidence: High. ￼ ￼ ￼
• Inferences: Combining an NL parser with rrule is the most maintainable way to get “every week for 3 months” into buckets, and a small custom grammar may suffice for constrained phrases. Confidence: Medium–High.
• Speculation: Newer crates like natural-date-parser and date_time_parser_multi_language may evolve quickly; feature coverage for recurrence phrases could improve. Confidence: Medium.

---

# Consolidated Notes and Plan (vNext)

This section captures implementation notes and an incremental plan for robust natural‑language window bucketing. It complements the Q/A above with concrete architecture, library options, edge cases, and test ideas.

## Architecture: Parse → RRULE → Expand

- Parse intent (start anchor, cadence, interval, horizon) from phrases like:
  - "each|every month for the last N months"
  - "each|every week for the last N weeks"
  - Base phrases: "last week", "last month", "yesterday", "N units ago"
- Build an RFC 5545 RRULE + DTSTART representing cadence and horizon (e.g., `FREQ=WEEKLY;INTERVAL=1;UNTIL=...`).
- Expand occurrences with the `rrule` crate; convert each occurrence to a bucket [start, end). For weeks: next occurrence is the end. For months: next occurrence (or calendar next‑month) is the end.
- Keep all times in a real timezone (`chrono-tz`) to avoid DST footguns.

Why this works: NL parsing and recurrence expansion are separable concerns; RRULE already encodes complex cadence rules. We only need to extract cadence + limits reliably.

## Candidate Libraries (parsing layer)

- rustling-ontology: rich NLU for Time/Duration/Interval/Numbers; heavy compile, fast runtime. Good for multi‑language or flexible phrasing.
- timewarp: pest‑based English/German date parsing; lighter dependency footprint.
- chrono-english: solid for durations and relative instants; not a recurrence parser (we already use it for some phrases).
- date_time_parser_multi_language / natural-date-parser: can parse many date formats and relative expressions; recurrence intent often still needs custom grammar.

Recommendation: for constrained patterns (our current need), a tiny pest/nom grammar is sufficient and keeps dependencies light. If we expand to multi‑lingual or broad NL, evaluate rustling‑ontology.

## Phrase Coverage and Grammar Sketch

- Buckets:
  - Pattern: `(every|each) <unit:week|month> for the last <N> <unit>s?`
  - N can be either digits or small spelled numbers (one..twelve). We currently support 1–12 spelled; digits arbitrary.
  - Anchor: now (or `--now-override` in tests). Expand occurrences backwards to produce N buckets, then reverse to earliest→latest.
- Base windows (single range):
  - "last week" → [start_of_prev_week, start_of_this_week)
  - "last month" → [1st_of_last_month, 1st_of_this_month)
  - "last <weekday>" → [previous weekday 00:00, now)
  - "yesterday" → [now-1d, now)
  - "N <units> ago" → [now-N, now)

## RRULE Construction Notes

- DTSTART: localized now truncated to the appropriate boundary (e.g., start_of_week or start_of_month as needed).
- FREQ/INTERVAL: from cadence (WEEKLY/MONTHLY, interval=1 unless specified).
- Horizon: choose `UNTIL` when phrase implies a wall‑clock window ("for the last N months"), or `COUNT` when the phrase implies a number of occurrences without explicit end.
- Expansion: emit occurrences up to `now` (cap future occurrences).

## Timezone and DST

- Use `chrono-tz::Tz` for the chosen zone (IANA, local, or UTC).
- Keep DTSTART in the chosen zone; RRULE expansion remains in the same zone.
- Bucket boundaries: compute naive local start times, then render ISO without timezone for git approxidate friendliness in argv; keep stored timestamps consistent with our existing ISO strategy.

## Spelled Numbers

- Current bridge: map one..twelve → 1..12 (we implemented this for week/month buckets).
- Next: consider using a number‑word crate if we extend beyond 12, or rustling‑ontology’s numeric extraction.

## Edge Cases and Decisions

- Month ends (e.g., Jan 31 → Feb): RRULE expansion handles calendar length; our month bucket boundaries use the 1st of each month at 00:00.
- Week start: use ISO week starting Monday for labels; label example: `YYYY-Www`.
- Future end caps: when the natural parser yields a range ending after now, clamp `end = now`.
- Performance: RRULE expansion is cheap for our scales (<= ~200 occurrences). Avoid O(N^2) loops and clone only when necessary.

## Tests (suggested)

- Deterministic with `--now-override` in UTC to avoid local variance.
- Buckets:
  - each/every + digits + spelled numbers (six/twelve) for weeks/months.
  - Earliest→latest order; labels match month or ISO week.
- Base phrases:
  - last week/month/year; last <weekday>; N units ago (minutes/hours/days/weeks/months).
- DST transitions: ensure bucket sizes remain consistent around DST changes in a fixed IANA zone.

## Migration Plan

1) Keep current lightweight regex approach for the two constrained patterns (weeks/months) and spelled numbers 1–12.
2) Prototype a small grammar or integrate rustling‑ontology; normalize to RRULE builder.
3) Swap bucket computation to RRULE expansion while keeping external JSON contracts unchanged.

Open Questions

- How far to expand spelled number coverage (beyond twelve)?
- Do we need multi‑lingual inputs? If yes, rustling‑ontology becomes more attractive.

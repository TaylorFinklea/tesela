//! Registry-driven inline-NLP-lift detection (`tesela-ug7`).
//!
//! Port of the web `date-parser.ts` (NL date/time/recurrence parsing) +
//! `task-tokens.ts` (`detectTokens`/`detectTaskTokens` — config-driven token
//! detection over a block's first line: select triggers, `<number> <trigger>`
//! pairs, `<trigger> <date>` pairs, and a bare trailing/line-start/
//! intent-gated default date) into Rust, so both clients can eventually share
//! one implementation via FFI (`crates/tesela-sync-ffi::detect_nlp_lifts`,
//! consumed by iOS's `InlineNLP.detectLifts`) instead of hand-duplicating it.
//! Recurrence recognition delegates to `crate::recurrence::recognize` (the
//! same source of truth `parse_recurrence`/`format_recurrence` already
//! expose, tesela-pfix.2) rather than reimplementing it here.
//!
//! Conformance is pinned by `tests/fixtures/nlp-lift-conformance.json`
//! (tesela-pfix.3) — see `tests/nlp_lift_conformance.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use chrono::{Datelike, Duration, Months, NaiveDate};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::recurrence;

// ---------------------------------------------------------------------
// Registry spec (caller-provided — mirrors web's `DetectSpec`/iOS's
// resolved `PropertyDef` list, JSON-shaped so the FFI boundary stays a
// plain string in / string out, like `parse_recurrence`).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySpec {
    pub key: String,
    pub value_type: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub default_date_property: String,
    pub properties: Vec<PropertySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiftedProp {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResult {
    pub stripped: String,
    pub props: Vec<LiftedProp>,
}

// ---------------------------------------------------------------------
// Date/time/recurrence phrase parsing (date-parser.ts port).
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParsedDateTimeRecurrence {
    pub date: String,
    pub time: Option<String>,
    pub recurrence: Option<String>,
    pub field: Option<String>,
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn weekday_from_str(s: &str) -> Option<u32> {
    // 0-based, Sun=0 .. Sat=6 (matches `Weekday::num_days_from_sunday`).
    Some(match s {
        "sun" | "sunday" => 0,
        "mon" | "monday" => 1,
        "tue" | "tues" | "tuesday" => 2,
        "wed" | "weds" | "wednesday" => 3,
        "thu" | "thur" | "thurs" | "thursday" => 4,
        "fri" | "friday" => 5,
        "sat" | "saturday" => 6,
        _ => return None,
    })
}

fn month_from_str(s: &str) -> Option<u32> {
    // 1-based (matches `NaiveDate::from_ymd_opt`).
    Some(match s {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    })
}

/// Next occurrence of `target` weekday (1-7 days ahead — never today).
fn next_weekday(base: NaiveDate, target: u32) -> NaiveDate {
    let cur = base.weekday().num_days_from_sunday();
    let delta = (target + 7 - cur) % 7;
    let delta = if delta == 0 { 7 } else { delta };
    base + Duration::days(delta as i64)
}

/// Soonest occurrence of `target` weekday (today if today matches, else upcoming).
fn soonest_weekday(base: NaiveDate, target: u32) -> NaiveDate {
    let cur = base.weekday().num_days_from_sunday();
    let delta = (target + 7 - cur) % 7;
    base + Duration::days(delta as i64)
}

static ISO_DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4})-(\d{1,2})-(\d{1,2})$").unwrap());
static SLASH_DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{1,2})/(\d{1,2})(?:/(\d{2}|\d{4}))?$").unwrap());
static MONTH_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(\d{1,2})\s+)?([a-z]+)(?:\s+(\d{1,2}))?(?:[,\s]+(\d{2}|\d{4}))?$").unwrap()
});
static NEXT_WEEKDAY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^next\s+([a-z]+)$").unwrap());
static IN_N_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^in\s+(\d+)\s+(day|days|week|weeks|month|months|d|w|mo)$").unwrap()
});
static SHORT_N_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)(d|w)$").unwrap());

fn year_from_group(s: &str) -> i32 {
    let y: i32 = s.parse().unwrap_or(0);
    if y < 100 {
        2000 + y
    } else {
        y
    }
}

/// Resolve an unspecified year for a month/day phrase: this year, unless
/// that date has already passed relative to `today`, in which case next
/// year (mirrors web's `date-parser.ts` / iOS `DateParser.swift`).
fn resolve_year(month: u32, day: u32, today: NaiveDate) -> i32 {
    let mut year = today.year();
    if let Some(candidate) = NaiveDate::from_ymd_opt(year, month, day) {
        if candidate < today {
            year += 1;
        }
    }
    year
}

fn parse_date_part(s: &str, today: NaiveDate) -> Option<String> {
    match s {
        "today" | "tod" => return Some(fmt_date(today)),
        "tomorrow" | "tom" | "tmrw" => return Some(fmt_date(today + Duration::days(1))),
        "yesterday" | "yes" | "yest" => return Some(fmt_date(today - Duration::days(1))),
        _ => {}
    }

    if let Some(caps) = ISO_DATE_RE.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let mo: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return NaiveDate::from_ymd_opt(y, mo, d).map(fmt_date);
    }

    if let Some(caps) = SLASH_DATE_RE.captures(s) {
        let mo: u32 = caps[1].parse().ok()?;
        let d: u32 = caps[2].parse().ok()?;
        let year = match caps.get(3) {
            Some(y) => year_from_group(y.as_str()),
            None => resolve_year(mo, d, today),
        };
        return NaiveDate::from_ymd_opt(year, mo, d).map(fmt_date);
    }

    // "apr 15" / "april 15" / "15 apr" — optional year.
    if let Some(caps) = MONTH_NAME_RE.captures(s) {
        if let Some(mo) = month_from_str(&caps[2]) {
            let day_str = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());
            if let Some(day_str) = day_str {
                let d: u32 = day_str.parse().ok()?;
                let year = match caps.get(4) {
                    Some(y) => year_from_group(y.as_str()),
                    None => resolve_year(mo, d, today),
                };
                if let Some(date) = NaiveDate::from_ymd_opt(year, mo, d) {
                    return Some(fmt_date(date));
                }
            }
        }
    }

    if s == "next week" || s == "nw" {
        return Some(fmt_date(today + Duration::days(7)));
    }

    // "next <weekday>" — strictly future (skips today).
    if let Some(caps) = NEXT_WEEKDAY_RE.captures(s) {
        if let Some(wd) = weekday_from_str(&caps[1]) {
            return Some(fmt_date(next_weekday(today, wd)));
        }
    }

    if let Some(wd) = weekday_from_str(s) {
        return Some(fmt_date(soonest_weekday(today, wd)));
    }

    // "in N days/weeks/months".
    if let Some(caps) = IN_N_RE.captures(s) {
        let n: i64 = caps[1].parse().ok()?;
        let unit = &caps[2];
        if unit.starts_with('d') {
            return Some(fmt_date(today + Duration::days(n)));
        }
        if unit.starts_with('w') {
            return Some(fmt_date(today + Duration::days(n * 7)));
        }
        // months
        return today
            .checked_add_months(Months::new(n as u32))
            .map(fmt_date);
    }

    // "<N>d" / "<N>w" shorthand.
    if let Some(caps) = SHORT_N_RE.captures(s) {
        let n: i64 = caps[1].parse().ok()?;
        let unit = &caps[2];
        return Some(fmt_date(today + Duration::days(if unit == "w" { n * 7 } else { n })));
    }

    None
}

static TRAILING_TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|\s)(at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?$").unwrap()
});

/// Strip a trailing time phrase ("at 8", "10am", "noon", "today noon") off
/// `s`. Returns the 24-hour `HH:MM` (when present) and the remaining date
/// phrase — defaulting to `"today"` when the whole input was time-only.
fn extract_time(s: &str) -> (Option<String>, String) {
    if s == "noon" {
        return (Some("12:00".to_string()), "today".to_string());
    }
    if s == "midnight" {
        return (Some("00:00".to_string()), "today".to_string());
    }
    if let Some(rest) = s.strip_suffix(" noon") {
        let rest = rest.trim();
        return (
            Some("12:00".to_string()),
            if rest.is_empty() { "today".to_string() } else { rest.to_string() },
        );
    }
    if let Some(rest) = s.strip_suffix(" midnight") {
        let rest = rest.trim();
        return (
            Some("00:00".to_string()),
            if rest.is_empty() { "today".to_string() } else { rest.to_string() },
        );
    }

    let Some(caps) = TRAILING_TIME_RE.captures(s) else {
        return (None, s.to_string());
    };
    let has_at = caps.get(1).is_some();
    let hour_str = &caps[2];
    let min_str = caps.get(3).map(|m| m.as_str());
    let ampm = caps.get(4).map(|m| m.as_str().to_lowercase());
    let has_colon = min_str.is_some();
    let has_ampm = ampm.is_some();
    if !has_at && !has_colon && !has_ampm {
        return (None, s.to_string());
    }
    let Ok(mut h) = hour_str.parse::<i32>() else {
        return (None, s.to_string());
    };
    let mins: i32 = min_str.and_then(|m| m.parse().ok()).unwrap_or(0);
    if ampm.as_deref() == Some("pm") && h < 12 {
        h += 12;
    }
    if ampm.as_deref() == Some("am") && h == 12 {
        h = 0;
    }
    if !(0..=23).contains(&h) || !(0..=59).contains(&mins) {
        return (None, s.to_string());
    }
    let time = format!("{h:02}:{mins:02}");
    let m0 = caps.get(0).unwrap();
    let rest = s[..m0.start()].trim();
    (Some(time), if rest.is_empty() { "today".to_string() } else { rest.to_string() })
}

/// Parse a date+time phrase (no field/recurrence extraction). `None` on
/// failure; time-only input ("noon", "10am") defaults the date to `today`.
fn parse_date_input(input: &str, today: NaiveDate) -> Option<(String, Option<String>)> {
    let raw = input.trim().to_lowercase();
    if raw.is_empty() {
        return None;
    }
    let (time, rest) = extract_time(&raw);
    let date = parse_date_part(&rest, today)?;
    Some((date, time))
}

static FIELD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(deadline|scheduled|due)\s+(.+)$").unwrap());

/// Strip a leading `deadline`/`scheduled`/`due` keyword. `due` -> `deadline`.
fn extract_field(raw: &str) -> (Option<String>, String) {
    let Some(caps) = FIELD_RE.captures(raw) else {
        return (None, raw.to_string());
    };
    let keyword = &caps[1];
    let field = if keyword == "due" { "deadline" } else { keyword };
    (Some(field.to_string()), caps[2].to_string())
}

const BYDAY_TOKEN: &str =
    r"(?:mon(?:day)?|tues?(?:day)?|wed(?:nesday)?|thu(?:rs?(?:day)?)?|fri(?:day)?|sat(?:urday)?|sun(?:day)?)";

static TRAILING_RECUR_RE: LazyLock<Regex> = LazyLock::new(|| {
    let byday_set = format!(r"every\s+{BYDAY_TOKEN}(?:\s*,\s*{BYDAY_TOKEN})*");
    let end_clause = r"(?:\s+until\s+\d{4}-\d{2}-\d{2}|\s+count\s+\d+)?";
    let pattern = format!(
        r"(?i)\s+((?:daily|weekly|monthly|yearly|annually|biweekly|fortnightly|quarterly|weekdays|weekends|every\s+other\s+(?:days?|weeks?|months?|years?)|every\s+\d+\s+(?:days?|weeks?|months?|years?)|every\s+weekdays?|every\s+(?:day|week|month|year)|{byday_set}){end_clause})$"
    );
    Regex::new(&pattern).unwrap()
});

/// Strip a trailing recurrence phrase off `s` (requires leading whitespace —
/// won't match a bare recurrence-only phrase). Delegates recognition to
/// `crate::recurrence::recognize` — the shared canonical parser.
fn extract_recurrence(s: &str) -> (Option<String>, String) {
    if let Some(caps) = TRAILING_RECUR_RE.captures(s) {
        let m0 = caps.get(0).unwrap();
        let tail = caps.get(1).unwrap().as_str().to_lowercase();
        if let Some(rec) = recurrence::recognize(&tail) {
            let rest = s[..m0.start()].trim().to_string();
            return (Some(rec), rest);
        }
    }
    (None, s.to_string())
}

/// Parse a natural-language phrase that may contain date + time + recurrence
/// (+ a leading `deadline`/`scheduled`/`due` field keyword). Recurrence and
/// time are independent — either, both, or neither may be present. `None`
/// only when the date portion is unrecognized (mirrors `date-parser.ts`'s
/// `parseDateAndRecurrenceInput`).
pub fn parse_date_and_recurrence_input(
    input: &str,
    today: NaiveDate,
) -> Option<ParsedDateTimeRecurrence> {
    let raw = input.trim().to_lowercase();
    if raw.is_empty() {
        return None;
    }
    let (field, after_field) = extract_field(&raw);
    let (recurrence, rest) = extract_recurrence(&after_field);
    if let Some((date, time)) = parse_date_input(&rest, today) {
        return Some(ParsedDateTimeRecurrence { date, time, recurrence, field });
    }
    // The whole input may itself be a bare recurrence phrase with no date
    // ("every monday", "weekdays", "deadline every day"). Recurrence-only
    // input anchors to today so the engine has a date to bump from.
    let bare_rec = recurrence.or_else(|| recurrence::recognize(&after_field));
    if let Some(bare_rec) = bare_rec {
        return Some(ParsedDateTimeRecurrence {
            date: fmt_date(today),
            time: None,
            recurrence: Some(bare_rec),
            field,
        });
    }
    None
}

// ---------------------------------------------------------------------
// literalRanges (mirror of task-tokens.ts): spans no lift may cross.
// ---------------------------------------------------------------------

static WIKI_LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[[^\]]*\]\]").unwrap());
static MD_LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!?\[[^\]]*\]\([^)]*\)").unwrap());
static INLINE_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]*`").unwrap());
static BARE_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bhttps?://\S+").unwrap());
static WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+").unwrap());

/// Literal ranges (byte offsets into `line0`) that detection must never lift
/// a token out of: `[[wiki links]]`, markdown links/images, bare URLs, and
/// inline `` `code` `` spans.
fn literal_ranges(line0: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for re in [&*WIKI_LINK_RE, &*MD_LINK_RE, &*INLINE_CODE_RE, &*BARE_URL_RE] {
        for m in re.find_iter(line0) {
            ranges.push((m.start(), m.end()));
        }
    }
    ranges
}

fn overlaps(claimed: &[(usize, usize)], from: usize, to: usize) -> bool {
    claimed.iter().any(|&(a, b)| from < b && to > a)
}

// ---------------------------------------------------------------------
// detectTokens / detectTaskTokens port.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
struct WordSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
struct DateHit {
    from: usize,
    to: usize,
    end_word: usize,
    date: String,
    time: Option<String>,
    recurrence: Option<String>,
}

#[derive(Debug, Clone)]
struct DetectedToken {
    from: usize,
    to: usize,
    key: String,
    value: String,
    recurrence: Option<String>,
}

const MAX_DATE_WORDS: usize = 5;

fn date_value(date: &str, time: &Option<String>) -> String {
    match time {
        Some(t) => format!("{date} {t}"),
        None => date.to_string(),
    }
}

/// Longest valid date phrase starting at word `i` (skips claimed ranges).
fn longest_date_from(
    line0: &str,
    words: &[WordSpan],
    i: usize,
    claimed: &[(usize, usize)],
    today: NaiveDate,
) -> Option<DateHit> {
    let max_len = MAX_DATE_WORDS.min(words.len() - i);
    for len in (1..=max_len).rev() {
        let from = words[i].start;
        let to = words[i + len - 1].end;
        if overlaps(claimed, from, to) {
            continue;
        }
        if let Some(parsed) = parse_date_and_recurrence_input(&line0[from..to], today) {
            return Some(DateHit {
                from,
                to,
                end_word: i + len - 1,
                date: parsed.date,
                time: parsed.time,
                recurrence: parsed.recurrence,
            });
        }
    }
    None
}

/// Whether nothing but whitespace and/or already-claimed spans follows
/// `from` to the end of the line.
fn is_trailing_from(line0: &str, claimed: &[(usize, usize)], from: usize) -> bool {
    let mut j = from;
    while j < line0.len() {
        let ch = line0[j..].chars().next().unwrap();
        if ch.is_whitespace() {
            j += ch.len_utf8();
            continue;
        }
        match claimed.iter().find(|&&(a, b)| j >= a && j < b) {
            Some(&(_, b)) => j = b,
            None => return false,
        }
    }
    true
}

/// Detect tokens in the block's first line per the spec. `today` anchors
/// relative-date phrases ("tomorrow", "next tuesday"). Mirrors web's
/// `detectTokens`.
fn detect_tokens(line0: &str, spec: &Registry, today: NaiveDate) -> Vec<DetectedToken> {
    let mut tokens: Vec<DetectedToken> = Vec::new();
    let mut claimed: Vec<(usize, usize)> = literal_ranges(line0);
    let words: Vec<WordSpan> = WORD_RE
        .find_iter(line0)
        .map(|m| WordSpan { start: m.start(), end: m.end() })
        .collect();

    // 1. select props: each trigger IS the value token.
    for p in &spec.properties {
        if p.value_type != "select" || p.triggers.is_empty() {
            continue;
        }
        for trig in &p.triggers {
            let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(trig))) else { continue };
            for m in re.find_iter(line0) {
                let (from, to) = (m.start(), m.end());
                if overlaps(&claimed, from, to) {
                    continue;
                }
                claimed.push((from, to));
                tokens.push(DetectedToken { from, to, key: p.key.clone(), value: trig.clone(), recurrence: None });
            }
        }
    }

    // 2. number props: `<number> <trigger>`.
    for p in &spec.properties {
        if p.value_type != "number" || p.triggers.is_empty() {
            continue;
        }
        for trig in &p.triggers {
            let Ok(re) = Regex::new(&format!(r"(?i)\b(\d+(?:\.\d+)?)\s*{}\b", regex::escape(trig))) else { continue };
            for caps in re.captures_iter(line0) {
                let m0 = caps.get(0).unwrap();
                let (from, to) = (m0.start(), m0.end());
                if overlaps(&claimed, from, to) {
                    continue;
                }
                claimed.push((from, to));
                let value = caps.get(1).unwrap().as_str().to_string();
                tokens.push(DetectedToken { from, to, key: p.key.clone(), value, recurrence: None });
            }
        }
    }

    // 3. date props with triggers: `<trigger> <NL date+time>`.
    for p in &spec.properties {
        if p.value_type != "date" || p.triggers.is_empty() {
            continue;
        }
        for trig in &p.triggers {
            let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(trig))) else { continue };
            for m in re.find_iter(line0) {
                let trig_from = m.start();
                let after_trig = m.end();
                if overlaps(&claimed, trig_from, after_trig) {
                    continue;
                }
                let Some(start_word) = words.iter().position(|w| w.start >= after_trig) else { continue };
                if let Some(hit) = longest_date_from(line0, &words, start_word, &claimed, today) {
                    if !overlaps(&claimed, trig_from, hit.to) {
                        claimed.push((trig_from, hit.to));
                        let value = date_value(&hit.date, &hit.time);
                        tokens.push(DetectedToken {
                            from: trig_from,
                            to: hit.to,
                            key: p.key.clone(),
                            value,
                            recurrence: hit.recurrence.clone(),
                        });
                    }
                }
            }
        }
    }

    // 4. default date property: bare NL dates — ONLY at line-start, trailing
    // position, or right after a date-intent word (the locked
    // trailing-position rule: a bare date phrase mid-prose needs an intent
    // word, so "call her tomorrow about the launch" doesn't lift, while "buy
    // milk tomorrow" does).
    let default_key = spec.default_date_property.clone();
    let mut date_intent_words: HashSet<String> =
        ["on", "by", "at"].iter().map(|s| s.to_string()).collect();
    for p in &spec.properties {
        if p.value_type != "date" {
            continue;
        }
        for t in &p.triggers {
            date_intent_words.insert(t.to_lowercase());
        }
    }

    let mut i = 0;
    while i < words.len() {
        if overlaps(&claimed, words[i].start, words[i].end) {
            i += 1;
            continue;
        }
        if let Some(hit) = longest_date_from(line0, &words, i, &claimed, today) {
            let at_line_start = hit.from == 0;
            let prev_word = if i > 0 {
                Some(line0[words[i - 1].start..words[i - 1].end].to_lowercase())
            } else {
                None
            };
            let preceded_by_intent = prev_word.map(|w| date_intent_words.contains(&w)).unwrap_or(false);
            if at_line_start || preceded_by_intent || is_trailing_from(line0, &claimed, hit.to) {
                claimed.push((hit.from, hit.to));
                let value = date_value(&hit.date, &hit.time);
                tokens.push(DetectedToken {
                    from: hit.from,
                    to: hit.to,
                    key: default_key.clone(),
                    value,
                    recurrence: hit.recurrence.clone(),
                });
            }
            i = hit.end_word + 1;
        } else {
            i += 1;
        }
    }

    tokens.sort_by_key(|t| t.from);
    tokens
}

static COLLAPSE_SPACES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" {2,}").unwrap());

/// Detect + strip tokens from `text` (first line only). Returns the stripped
/// text + the structured props to set, or unchanged + `[]` when nothing is
/// detected. Per key, the LAST token wins (in line-position order);
/// recurrences ride along as `recurring`. Mirrors web's `detectTaskTokens`.
pub fn detect_task_tokens(text: &str, spec: &Registry, today: NaiveDate) -> DetectResult {
    let (line0, rest) = match text.find('\n') {
        Some(idx) => (&text[..idx], &text[idx..]),
        None => (text, ""),
    };

    let tokens = detect_tokens(line0, spec, today);
    if tokens.is_empty() {
        return DetectResult { stripped: text.to_string(), props: Vec::new() };
    }

    let mut order: Vec<String> = Vec::new();
    let mut values: HashMap<String, String> = HashMap::new();
    let mut recurrences: Vec<LiftedProp> = Vec::new();
    for t in &tokens {
        if !values.contains_key(&t.key) {
            order.push(t.key.clone());
        }
        values.insert(t.key.clone(), t.value.clone());
        if let Some(rec) = &t.recurrence {
            recurrences.push(LiftedProp { key: "recurring".to_string(), value: rec.clone() });
        }
    }
    let mut props: Vec<LiftedProp> = order
        .into_iter()
        .map(|k| {
            let v = values.get(&k).cloned().unwrap_or_default();
            LiftedProp { key: k, value: v }
        })
        .collect();
    props.extend(recurrences);

    let mut s = line0.to_string();
    let mut by_pos_desc = tokens.clone();
    by_pos_desc.sort_by(|a, b| b.from.cmp(&a.from));
    for t in &by_pos_desc {
        s.replace_range(t.from..t.to, "");
    }
    let collapsed = COLLAPSE_SPACES_RE.replace_all(&s, " ");
    let stripped = format!("{}{}", collapsed.trim(), rest);
    DetectResult { stripped, props }
}

// ---------------------------------------------------------------------
// FFI-facing entry point (see `tesela-sync-ffi::detect_nlp_lifts`).
// ---------------------------------------------------------------------

/// Parse `registry_json` + `anchor_date` ("YYYY-MM-DD") and run
/// `detect_task_tokens` against `text`, returning the `DetectResult` as
/// JSON. Never errors — malformed registry/anchor input degrades to "no
/// lift" (mirrors `format_recurrence`'s never-errors contract), so a caller
/// mistake shows unstripped text rather than crashing the editor.
pub fn detect_nlp_lifts_json(text: &str, registry_json: &str, anchor_date: &str) -> String {
    let fallback = || {
        serde_json::to_string(&DetectResult { stripped: text.to_string(), props: Vec::new() })
            .unwrap_or_else(|_| "{\"stripped\":\"\",\"props\":[]}".to_string())
    };
    let Ok(registry) = serde_json::from_str::<Registry>(registry_json) else {
        return fallback();
    };
    let Ok(today) = NaiveDate::parse_from_str(anchor_date, "%Y-%m-%d") else {
        return fallback();
    };
    let result = detect_task_tokens(text, &registry, today);
    serde_json::to_string(&result).unwrap_or_else(|_| fallback())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 22).unwrap()
    }

    fn fixture_registry() -> Registry {
        Registry {
            default_date_property: "deadline".to_string(),
            properties: vec![
                PropertySpec {
                    key: "priority".to_string(),
                    value_type: "select".to_string(),
                    choices: vec!["p1", "p2", "p3", "p4"].into_iter().map(str::to_string).collect(),
                    triggers: vec!["p1", "p2", "p3", "p4"].into_iter().map(str::to_string).collect(),
                },
                PropertySpec {
                    key: "deadline".to_string(),
                    value_type: "date".to_string(),
                    choices: vec![],
                    triggers: vec!["due", "deadline"].into_iter().map(str::to_string).collect(),
                },
            ],
        }
    }

    #[test]
    fn select_lift_basic() {
        let result = detect_task_tokens("fix the bug p1", &fixture_registry(), today());
        assert_eq!(result.stripped, "fix the bug");
        assert_eq!(result.props.len(), 1);
        assert_eq!(result.props[0].key, "priority");
        assert_eq!(result.props[0].value, "p1");
    }

    #[test]
    fn today_noon_lifts() {
        let result = detect_task_tokens("call mom today noon", &fixture_registry(), today());
        assert_eq!(result.stripped, "call mom");
        assert_eq!(result.props.len(), 1);
        assert_eq!(result.props[0].value, "2026-05-22 12:00");
    }

    #[test]
    fn url_embedded_priority_does_not_lift() {
        let result = detect_task_tokens("check https://x.com/p1/doc", &fixture_registry(), today());
        assert_eq!(result.stripped, "check https://x.com/p1/doc");
        assert!(result.props.is_empty());
    }

    #[test]
    fn midprose_bare_date_without_intent_does_not_lift() {
        let result =
            detect_task_tokens("call her tomorrow about the launch", &fixture_registry(), today());
        assert_eq!(result.stripped, "call her tomorrow about the launch");
        assert!(result.props.is_empty());
    }

    #[test]
    fn ffi_json_round_trip() {
        let registry_json = serde_json::to_string(&fixture_registry()).unwrap();
        let json = detect_nlp_lifts_json("fix the bug p1", &registry_json, "2026-05-22");
        let result: DetectResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.stripped, "fix the bug");
        assert_eq!(result.props[0].value, "p1");
    }

    #[test]
    fn ffi_json_malformed_input_falls_back_unstripped() {
        let json = detect_nlp_lifts_json("fix the bug p1", "not json", "2026-05-22");
        let result: DetectResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.stripped, "fix the bug p1");
        assert!(result.props.is_empty());
    }
}

//! Recurrence engine for Task blocks.
//!
//! Reads a small natural-language vocabulary stored in the `recurring::`
//! property and produces the next occurrence date relative to an anchor.
//! Pure module — no I/O, no allocation beyond the parser's tokenizer —
//! so the same routines can be called from server handlers, the CLI, or
//! a future Swift FFI bridge.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceEnd {
    /// Series runs through this date (inclusive).
    Until(NaiveDate),
    /// Total number of occurrences, including the first (rrule COUNT).
    Count(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recurrence {
    pub freq: Freq,
    /// >= 1. For `Daily` this is the "every N days" step.
    pub interval: u32,
    /// Empty = anchor on the date's own weekday / day-of-month.
    /// Non-empty = a BYDAY set (implies weekly cadence).
    pub by_weekday: Vec<Weekday>,
    pub end: Option<RecurrenceEnd>,
}

impl Recurrence {
    /// Constructor for a plain interval recurrence with no BYDAY / end.
    pub fn simple(freq: Freq, interval: u32) -> Self {
        Recurrence { freq, interval, by_weekday: Vec::new(), end: None }
    }
}

/// Parse a weekday token — three-letter or full name. Case-insensitive
/// (caller already lowercased).
fn parse_weekday(tok: &str) -> Option<Weekday> {
    Some(match tok {
        "mon" | "monday" => Weekday::Mon,
        "tue" | "tues" | "tuesday" => Weekday::Tue,
        "wed" | "wednesday" => Weekday::Wed,
        "thu" | "thur" | "thurs" | "thursday" => Weekday::Thu,
        "fri" | "friday" => Weekday::Fri,
        "sat" | "saturday" => Weekday::Sat,
        "sun" | "sunday" => Weekday::Sun,
        _ => return None,
    })
}

/// Sort a weekday set into Mon..Sun order and dedupe.
fn normalize_weekdays(mut days: Vec<Weekday>) -> Vec<Weekday> {
    days.sort_by_key(|w| w.num_days_from_monday());
    days.dedup();
    days
}

/// Parse a `recurring::` value. Lower-cases and collapses internal whitespace
/// before matching, so `"Every  2 Weeks"` is equivalent to `"every 2 weeks"`.
/// Returns `None` for unrecognized input — callers treat that as "no-op."
pub fn parse(input: &str) -> Option<Recurrence> {
    let s: String = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    match s.as_str() {
        "daily" | "every day" => return Some(Recurrence::simple(Freq::Daily, 1)),
        "weekly" | "every week" => return Some(Recurrence::simple(Freq::Weekly, 1)),
        "monthly" | "every month" => return Some(Recurrence::simple(Freq::Monthly, 1)),
        "yearly" | "annually" | "every year" => return Some(Recurrence::simple(Freq::Yearly, 1)),
        "weekdays" => {
            return Some(Recurrence {
                freq: Freq::Weekly,
                interval: 1,
                by_weekday: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri,
                ],
                end: None,
            })
        }
        "weekends" => {
            return Some(Recurrence {
                freq: Freq::Weekly,
                interval: 1,
                by_weekday: vec![Weekday::Sat, Weekday::Sun],
                end: None,
            })
        }
        _ => {}
    }

    // "every N <unit>" — `N` defaults to 1 if absent ("every day" already matched above).
    if let Some(rest) = s.strip_prefix("every ") {
        // BYDAY: "every mon, wed, fri" — all tokens must be weekdays.
        let day_tokens: Vec<&str> = rest.split(',').map(|t| t.trim()).collect();
        if !rest.is_empty() && day_tokens.iter().all(|t| parse_weekday(t).is_some()) {
            let days: Vec<Weekday> = day_tokens.iter().filter_map(|t| parse_weekday(t)).collect();
            return Some(Recurrence {
                freq: Freq::Weekly,
                interval: 1,
                by_weekday: normalize_weekdays(days),
                end: None,
            });
        }
        // "every N <unit>" handling.
        let (n_str, unit) = rest.split_once(' ')?;
        let n: u32 = n_str.parse().ok()?;
        if n == 0 {
            return None;
        }
        return match unit {
            "day" | "days" => Some(Recurrence::simple(Freq::Daily, n)),
            "week" | "weeks" => Some(Recurrence::simple(Freq::Weekly, n)),
            "month" | "months" => Some(Recurrence::simple(Freq::Monthly, n)),
            "year" | "years" => Some(Recurrence::simple(Freq::Yearly, n)),
            _ => None,
        };
    }

    None
}

/// Compute the next occurrence strictly after `anchor`.
///
/// - `Daily` / `Weekly` advance by a fixed day count scaled by `interval`.
/// - `Monthly` / `Yearly` clamp the day-of-month when the target month is
///   shorter (Jan 31 + 1 month → Feb 28/29).
/// - When `by_weekday` is non-empty, delegates to `next_by_weekday` (BYDAY
///   stepping, filled in Task 4).
pub fn next_after(rec: &Recurrence, anchor: NaiveDate) -> NaiveDate {
    if !rec.by_weekday.is_empty() {
        return next_by_weekday(rec, anchor);
    }
    match rec.freq {
        Freq::Daily => anchor + Duration::days(rec.interval as i64),
        Freq::Weekly => anchor + Duration::days(7 * rec.interval as i64),
        Freq::Monthly => add_months(anchor, rec.interval),
        Freq::Yearly => add_years(anchor, rec.interval),
    }
}

/// BYDAY stepping — filled in Task 4.
fn next_by_weekday(rec: &Recurrence, anchor: NaiveDate) -> NaiveDate {
    let _ = rec;
    anchor + Duration::days(1)
}

/// Add `n` calendar months, clamping day-of-month to the last valid day
/// of the target month (Jan 31 + 1 → Feb 28/29).
fn add_months(date: NaiveDate, n: u32) -> NaiveDate {
    let total_months = date.year() as i64 * 12 + (date.month0() as i64) + n as i64;
    let new_year = (total_months / 12) as i32;
    let new_month0 = (total_months % 12) as u32;
    let new_month = new_month0 + 1;
    let last_day = days_in_month(new_year, new_month);
    let day = date.day().min(last_day);
    NaiveDate::from_ymd_opt(new_year, new_month, day)
        .expect("clamped day is always valid for the target month")
}

/// Add `n` years, clamping Feb 29 → Feb 28 on non-leap years.
fn add_years(date: NaiveDate, n: u32) -> NaiveDate {
    let new_year = date.year() + n as i32;
    let last_day = days_in_month(new_year, date.month());
    let day = date.day().min(last_day);
    NaiveDate::from_ymd_opt(new_year, date.month(), day)
        .expect("clamped day is always valid for the target year/month")
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_y, next_m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_of_next = NaiveDate::from_ymd_opt(next_y, next_m, 1).expect("valid month");
    let last = first_of_next - Duration::days(1);
    last.day()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }

    #[test]
    fn parse_simple_phrases() {
        assert_eq!(parse("daily"), Some(Recurrence::simple(Freq::Daily, 1)));
        assert_eq!(parse(" Daily "), Some(Recurrence::simple(Freq::Daily, 1)));
        assert_eq!(parse("every day"), Some(Recurrence::simple(Freq::Daily, 1)));
        assert_eq!(parse("weekly"), Some(Recurrence::simple(Freq::Weekly, 1)));
        assert_eq!(parse("every week"), Some(Recurrence::simple(Freq::Weekly, 1)));
        assert_eq!(parse("monthly"), Some(Recurrence::simple(Freq::Monthly, 1)));
        assert_eq!(parse("yearly"), Some(Recurrence::simple(Freq::Yearly, 1)));
        assert_eq!(parse("annually"), Some(Recurrence::simple(Freq::Yearly, 1)));
        assert_eq!(
            parse("weekdays"),
            Some(Recurrence {
                freq: Freq::Weekly,
                interval: 1,
                by_weekday: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
                end: None,
            })
        );
    }

    #[test]
    fn parse_every_n() {
        assert_eq!(parse("every 2 weeks"), Some(Recurrence::simple(Freq::Weekly, 2)));
        assert_eq!(parse("every 3 days"), Some(Recurrence::simple(Freq::Daily, 3)));
        assert_eq!(parse("every 1 day"), Some(Recurrence::simple(Freq::Daily, 1)));
        assert_eq!(parse("every 6 months"), Some(Recurrence::simple(Freq::Monthly, 6)));
        assert_eq!(parse("every 2 years"), Some(Recurrence::simple(Freq::Yearly, 2)));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("blarg"), None);
        assert_eq!(parse("every"), None);
        assert_eq!(parse("every 0 days"), None);
        assert_eq!(parse("every 2 fortnights"), None);
        // "every monday" becomes valid in Task 2; removed from this test
    }

    #[test]
    fn next_after_daily_and_every_n() {
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Daily, 1), d(2026, 5, 7)),
            d(2026, 5, 8)
        );
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Daily, 3), d(2026, 5, 7)),
            d(2026, 5, 10)
        );
    }

    #[test]
    fn next_after_weekly() {
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Weekly, 1), d(2026, 5, 7)),
            d(2026, 5, 14)
        );
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Weekly, 2), d(2026, 5, 7)),
            d(2026, 5, 21)
        );
    }

    #[test]
    fn next_after_monthly_clamps_short_months() {
        // Jan 31 + 1 month → Feb 28 (2026 is not a leap year)
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Monthly, 1), d(2026, 1, 31)),
            d(2026, 2, 28)
        );
        // Mar 31 + 1 month → Apr 30
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Monthly, 1), d(2026, 3, 31)),
            d(2026, 4, 30)
        );
        // Dec → Jan rollover
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Monthly, 1), d(2026, 12, 15)),
            d(2027, 1, 15)
        );
    }

    #[test]
    fn next_after_yearly_handles_leap_day() {
        // Feb 29 2024 (leap) + 1 year → Feb 28 2025
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Yearly, 1), d(2024, 2, 29)),
            d(2025, 2, 28)
        );
        assert_eq!(
            next_after(&Recurrence::simple(Freq::Yearly, 4), d(2024, 2, 29)),
            d(2028, 2, 29)
        );
    }

    #[test]
    fn parse_weekends() {
        assert_eq!(
            parse("weekends"),
            Some(Recurrence {
                freq: Freq::Weekly,
                interval: 1,
                by_weekday: vec![Weekday::Sat, Weekday::Sun],
                end: None,
            })
        );
    }

    #[test]
    fn parse_byday_sets() {
        let mwf = parse("every mon, wed, fri").unwrap();
        assert_eq!(mwf.freq, Freq::Weekly);
        assert_eq!(mwf.by_weekday, vec![Weekday::Mon, Weekday::Wed, Weekday::Fri]);
        // Full names and a single day also parse.
        assert_eq!(parse("every monday").unwrap().by_weekday, vec![Weekday::Mon]);
        // Order is normalized Mon..Sun regardless of input order.
        assert_eq!(
            parse("every fri, mon").unwrap().by_weekday,
            vec![Weekday::Mon, Weekday::Fri]
        );
        // Unknown day token rejects the whole input.
        assert_eq!(parse("every mon, blarg"), None);
    }

    #[test]
    #[ignore = "BYDAY stepping lands in Task 4"]
    fn next_after_weekdays_skips_weekend() {
        let weekdays = Recurrence {
            freq: Freq::Weekly,
            interval: 1,
            by_weekday: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
            end: None,
        };
        // Fri 2026-05-08 → Mon 2026-05-11
        assert_eq!(next_after(&weekdays, d(2026, 5, 8)), d(2026, 5, 11));
        // Sat 2026-05-09 → Mon 2026-05-11
        assert_eq!(next_after(&weekdays, d(2026, 5, 9)), d(2026, 5, 11));
        // Sun 2026-05-10 → Mon 2026-05-11
        assert_eq!(next_after(&weekdays, d(2026, 5, 10)), d(2026, 5, 11));
        // Mon 2026-05-11 → Tue 2026-05-12
        assert_eq!(next_after(&weekdays, d(2026, 5, 11)), d(2026, 5, 12));
    }
}

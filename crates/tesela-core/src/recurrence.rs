//! Recurrence engine for Task blocks.
//!
//! Reads a small natural-language vocabulary stored in the `recurring::`
//! property and produces the next occurrence date relative to an anchor.
//! Pure module — no I/O, no allocation beyond the parser's tokenizer —
//! so the same routines can be called from server handlers, the CLI, or
//! a future Swift FFI bridge.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recurrence {
    Daily,
    Weekly { interval: u32 },
    Monthly { interval: u32 },
    Yearly { interval: u32 },
    Weekdays,
    EveryNDays(u32),
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
        "daily" | "every day" => return Some(Recurrence::Daily),
        "weekly" | "every week" => return Some(Recurrence::Weekly { interval: 1 }),
        "monthly" | "every month" => return Some(Recurrence::Monthly { interval: 1 }),
        "yearly" | "annually" | "every year" => return Some(Recurrence::Yearly { interval: 1 }),
        "weekdays" => return Some(Recurrence::Weekdays),
        _ => {}
    }

    // "every N <unit>" — `N` defaults to 1 if absent ("every day" already matched above).
    if let Some(rest) = s.strip_prefix("every ") {
        let mut parts = rest.splitn(2, ' ');
        let n_str = parts.next()?;
        let unit = parts.next()?;
        let n: u32 = n_str.parse().ok()?;
        if n == 0 {
            return None;
        }
        return match unit {
            "day" | "days" if n == 1 => Some(Recurrence::Daily),
            "day" | "days" => Some(Recurrence::EveryNDays(n)),
            "week" | "weeks" => Some(Recurrence::Weekly { interval: n }),
            "month" | "months" => Some(Recurrence::Monthly { interval: n }),
            "year" | "years" => Some(Recurrence::Yearly { interval: n }),
            _ => None,
        };
    }

    None
}

/// Compute the next occurrence strictly after `anchor`.
///
/// - `Daily` / `EveryNDays` / `Weekly` advance by a fixed day count.
/// - `Monthly` / `Yearly` clamp the day-of-month when the target month is
///   shorter (Jan 31 + 1 month → Feb 28/29).
/// - `Weekdays` advances one calendar day and skips Sat/Sun. From Friday
///   it lands on the following Monday.
pub fn next_after(rec: &Recurrence, anchor: NaiveDate) -> NaiveDate {
    match *rec {
        Recurrence::Daily => anchor + Duration::days(1),
        Recurrence::EveryNDays(n) => anchor + Duration::days(n as i64),
        Recurrence::Weekly { interval } => anchor + Duration::days(7 * interval as i64),
        Recurrence::Monthly { interval } => add_months(anchor, interval),
        Recurrence::Yearly { interval } => add_years(anchor, interval),
        Recurrence::Weekdays => {
            let mut d = anchor + Duration::days(1);
            while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
                d += Duration::days(1);
            }
            d
        }
    }
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
        assert_eq!(parse("daily"), Some(Recurrence::Daily));
        assert_eq!(parse(" Daily "), Some(Recurrence::Daily));
        assert_eq!(parse("every day"), Some(Recurrence::Daily));
        assert_eq!(parse("weekly"), Some(Recurrence::Weekly { interval: 1 }));
        assert_eq!(
            parse("every week"),
            Some(Recurrence::Weekly { interval: 1 })
        );
        assert_eq!(parse("monthly"), Some(Recurrence::Monthly { interval: 1 }));
        assert_eq!(parse("yearly"), Some(Recurrence::Yearly { interval: 1 }));
        assert_eq!(parse("annually"), Some(Recurrence::Yearly { interval: 1 }));
        assert_eq!(parse("weekdays"), Some(Recurrence::Weekdays));
    }

    #[test]
    fn parse_every_n() {
        assert_eq!(
            parse("every 2 weeks"),
            Some(Recurrence::Weekly { interval: 2 })
        );
        assert_eq!(parse("every 3 days"), Some(Recurrence::EveryNDays(3)));
        assert_eq!(parse("every 1 day"), Some(Recurrence::Daily));
        assert_eq!(
            parse("every 6 months"),
            Some(Recurrence::Monthly { interval: 6 })
        );
        assert_eq!(
            parse("every 2 years"),
            Some(Recurrence::Yearly { interval: 2 })
        );
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("blarg"), None);
        assert_eq!(parse("every"), None);
        assert_eq!(parse("every 0 days"), None);
        assert_eq!(parse("every 2 fortnights"), None);
        assert_eq!(parse("every monday"), None); // BYDAY deferred to v1.1+
    }

    #[test]
    fn next_after_daily_and_every_n() {
        assert_eq!(next_after(&Recurrence::Daily, d(2026, 5, 7)), d(2026, 5, 8));
        assert_eq!(
            next_after(&Recurrence::EveryNDays(3), d(2026, 5, 7)),
            d(2026, 5, 10)
        );
    }

    #[test]
    fn next_after_weekly() {
        assert_eq!(
            next_after(&Recurrence::Weekly { interval: 1 }, d(2026, 5, 7)),
            d(2026, 5, 14)
        );
        assert_eq!(
            next_after(&Recurrence::Weekly { interval: 2 }, d(2026, 5, 7)),
            d(2026, 5, 21)
        );
    }

    #[test]
    fn next_after_monthly_clamps_short_months() {
        // Jan 31 + 1 month → Feb 28 (2026 is not a leap year)
        assert_eq!(
            next_after(&Recurrence::Monthly { interval: 1 }, d(2026, 1, 31)),
            d(2026, 2, 28)
        );
        // Mar 31 + 1 month → Apr 30
        assert_eq!(
            next_after(&Recurrence::Monthly { interval: 1 }, d(2026, 3, 31)),
            d(2026, 4, 30)
        );
        // Dec → Jan rollover
        assert_eq!(
            next_after(&Recurrence::Monthly { interval: 1 }, d(2026, 12, 15)),
            d(2027, 1, 15)
        );
    }

    #[test]
    fn next_after_yearly_handles_leap_day() {
        // Feb 29 2024 (leap) + 1 year → Feb 28 2025
        assert_eq!(
            next_after(&Recurrence::Yearly { interval: 1 }, d(2024, 2, 29)),
            d(2025, 2, 28)
        );
        assert_eq!(
            next_after(&Recurrence::Yearly { interval: 4 }, d(2024, 2, 29)),
            d(2028, 2, 29)
        );
    }

    #[test]
    fn next_after_weekdays_skips_weekend() {
        // Fri 2026-05-08 → Mon 2026-05-11
        assert_eq!(
            next_after(&Recurrence::Weekdays, d(2026, 5, 8)),
            d(2026, 5, 11)
        );
        // Sat 2026-05-09 → Mon 2026-05-11
        assert_eq!(
            next_after(&Recurrence::Weekdays, d(2026, 5, 9)),
            d(2026, 5, 11)
        );
        // Sun 2026-05-10 → Mon 2026-05-11
        assert_eq!(
            next_after(&Recurrence::Weekdays, d(2026, 5, 10)),
            d(2026, 5, 11)
        );
        // Mon 2026-05-11 → Tue 2026-05-12
        assert_eq!(
            next_after(&Recurrence::Weekdays, d(2026, 5, 11)),
            d(2026, 5, 12)
        );
    }
}

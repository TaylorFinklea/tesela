# Recurrence Completeness (Engine) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Tesela's recurrence engine with BYDAY day-sets, `until`/`count` end conditions, a `weekends` keyword, skip-occurrence, and multi-field (`deadline::` + `scheduled::`) anchoring — all round-tripping to Apple Reminders.

**Architecture:** Refactor the flat `Copy` `Recurrence` enum in `tesela-core/src/recurrence.rs` into an rrule-shaped struct (`freq` / `interval` / `by_weekday` / `end`) that maps 1:1 onto `EKRecurrenceRule`. The pure `recurrence` module owns parsing + date math + series-end logic; `tesela-server` owns the `recurrence_done::` companion counter and the skip/complete bump path; `darwin.rs` owns the EventKit translation.

**Tech Stack:** Rust, `chrono` (`NaiveDate`, `Weekday`), `objc2-event-kit` (EventKit FFI). TDD with `cargo test -p <crate>`.

**Scope:** Engine only. Web `DatePicker` / `date-parser.ts` and iOS recurrence UI are a separate follow-up plan — the `recurring::` text grammar from this plan is fully functional without them.

**Reference spec:** `.docs/ai/phases/2026-05-21-recurrence-completeness-design.md`

---

## File Structure

- `crates/tesela-core/src/recurrence.rs` — **modify.** New `Recurrence` struct, `Freq`, `RecurrenceEnd`; extended `parse`; `next_after` with BYDAY; new `series_spent` / `advance` helpers.
- `crates/tesela-server/src/reminders/darwin.rs` — **modify.** `build_recurrence_rule` (push) + the pull-side `EKRecurrenceRule` → `Recurrence` mapping.
- `crates/tesela-server/src/**` — **modify.** The `apply_post_save_bumps` recurrence path (grep to locate) — multi-field anchor, `recurrence_done::` maintenance, series-end; the `recur-bump` endpoint gains a `mode`.

---

## Task 1: Refactor `Recurrence` to an rrule-shaped struct (behavior-preserving)

**Files:**
- Modify: `crates/tesela-core/src/recurrence.rs`

This task changes the *type* only — every input that parses today must still parse to an equivalent value and `next_after` must return identical dates. New syntax comes in Tasks 2-4.

- [ ] **Step 1: Replace the type definitions**

Replace the `Recurrence` enum (lines ~12-20) with:

```rust
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
```

- [ ] **Step 2: Rewrite `parse` to produce the struct (existing forms only)**

Keep the same recognized vocabulary; only the return type changes. `weekdays` becomes `Weekly` + the Mon-Fri set:

```rust
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
        _ => {}
    }

    if let Some(rest) = s.strip_prefix("every ") {
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
```

Note: `every 1 day` now yields `simple(Daily, 1)` — identical date math to the old `Daily`, so no behavior change.

- [ ] **Step 3: Rewrite `next_after` to take the struct**

`by_weekday` is empty for all Task-1 inputs, so the BYDAY branch is a stub that Task 4 fills:

```rust
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
```

`add_months`, `add_years`, `days_in_month` are unchanged.

- [ ] **Step 4: Update the existing tests to the struct shape**

In the `#[cfg(test)] mod tests`, rewrite assertions. `parse_simple_phrases`, `parse_every_n`, the `next_after_*` tests all change from enum variants to `Recurrence::simple(...)` / explicit structs. Example replacements:

```rust
assert_eq!(parse("daily"), Some(Recurrence::simple(Freq::Daily, 1)));
assert_eq!(parse("weekly"), Some(Recurrence::simple(Freq::Weekly, 1)));
assert_eq!(parse("every 2 weeks"), Some(Recurrence::simple(Freq::Weekly, 2)));
assert_eq!(parse("every 3 days"), Some(Recurrence::simple(Freq::Daily, 3)));
assert_eq!(
    parse("weekdays"),
    Some(Recurrence {
        freq: Freq::Weekly,
        interval: 1,
        by_weekday: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
        end: None,
    })
);

assert_eq!(
    next_after(&Recurrence::simple(Freq::Daily, 1), d(2026, 5, 7)),
    d(2026, 5, 8)
);
assert_eq!(
    next_after(&Recurrence::simple(Freq::Weekly, 2), d(2026, 5, 7)),
    d(2026, 5, 21)
);
```

The `next_after_weekdays_skips_weekend` test asserts the BYDAY-driven Mon-Fri behavior — leave it asserting the same dates; it will pass once Task 4 lands. For Task 1, mark it `#[ignore = "BYDAY stepping lands in Task 4"]` so the suite is green.

Keep `parse_rejects_garbage` but remove the `parse("every monday")` line — that becomes valid in Task 2; move it to a Task 2 test.

- [ ] **Step 5: Fix `darwin.rs` compile breakage**

`crates/tesela-server/src/reminders/darwin.rs` matches on the old enum in `build_recurrence_rule` (~line 622) and stores `recurrence: Option<Recurrence>`. The struct is no longer `Copy` — adjust call sites to borrow (`build_recurrence_rule(&Recurrence)`), and rewrite the `match` over the old variants into a `match rec.freq` (Task 7 generalizes it properly; for now produce the same `EKRecurrenceRule`s the old code did, ignoring `by_weekday`/`end`). This step is purely "make it compile with identical behavior."

- [ ] **Step 6: Run tests**

Run: `cargo test -p tesela-core --lib recurrence`
Expected: PASS (the one `#[ignore]`d test reported as ignored).
Run: `cargo build -p tesela-server`
Expected: compiles clean.

- [ ] **Step 7: Commit**

```bash
git add crates/tesela-core/src/recurrence.rs crates/tesela-server/src/reminders/darwin.rs
git commit -m "refactor(core): rrule-shaped Recurrence struct (behavior-preserving)"
```

---

## Task 2: Parse `weekends` + BYDAY day-sets

**Files:**
- Modify: `crates/tesela-core/src/recurrence.rs`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-core --lib recurrence::tests::parse_byday_sets`
Expected: FAIL (`every mon` currently returns `None`).

- [ ] **Step 3: Add a weekday-token parser + wire it into `parse`**

Add a helper and two new match arms. Put the helper above `parse`:

```rust
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
```

In `parse`, add `"weekends"` to the literal match arm:

```rust
"weekends" => {
    return Some(Recurrence {
        freq: Freq::Weekly,
        interval: 1,
        by_weekday: vec![Weekday::Sat, Weekday::Sun],
        end: None,
    })
}
```

And, in the `every ` branch, before the `(n_str, unit)` split, try a BYDAY parse: if every comma-separated token after `every ` is a weekday, build a weekly BYDAY recurrence:

```rust
if let Some(rest) = s.strip_prefix("every ") {
    // BYDAY: "every mon, wed, fri" — all tokens must be weekdays.
    let day_tokens: Vec<&str> = rest.split(',').map(|t| t.trim()).collect();
    if day_tokens.iter().all(|t| parse_weekday(t).is_some()) && !rest.is_empty() {
        let days: Vec<Weekday> = day_tokens.iter().filter_map(|t| parse_weekday(t)).collect();
        return Some(Recurrence {
            freq: Freq::Weekly,
            interval: 1,
            by_weekday: normalize_weekdays(days),
            end: None,
        });
    }
    // ... existing "every N <unit>" handling unchanged ...
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p tesela-core --lib recurrence`
Expected: PASS (`parse_weekends`, `parse_byday_sets`, plus all prior).

- [ ] **Step 5: Commit**

```bash
git add crates/tesela-core/src/recurrence.rs
git commit -m "feat(core): parse weekends + BYDAY day-sets in recurring::"
```

---

## Task 3: Parse `until` / `count` end conditions

**Files:**
- Modify: `crates/tesela-core/src/recurrence.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn parse_end_conditions() {
    let u = parse("weekly until 2026-12-31").unwrap();
    assert_eq!(u.freq, Freq::Weekly);
    assert_eq!(u.end, Some(RecurrenceEnd::Until(d(2026, 12, 31))));

    let c = parse("every mon, fri count 12").unwrap();
    assert_eq!(c.by_weekday, vec![Weekday::Mon, Weekday::Fri]);
    assert_eq!(c.end, Some(RecurrenceEnd::Count(12)));

    assert_eq!(
        parse("every 2 weeks until 2027-01-01").unwrap().end,
        Some(RecurrenceEnd::Until(d(2027, 1, 1)))
    );
    // count 0 and a bad date reject.
    assert_eq!(parse("daily count 0"), None);
    assert_eq!(parse("daily until not-a-date"), None);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-core --lib recurrence::tests::parse_end_conditions`
Expected: FAIL.

- [ ] **Step 3: Strip the end clause before the frequency parse**

At the very top of `parse`, after the whitespace-normalized `s` is built, split a trailing ` until <date>` / ` count <n>` clause off, parse the base, then attach `end`:

```rust
// Split off a trailing end clause: " until YYYY-MM-DD" or " count N".
let (base, end): (&str, Option<RecurrenceEnd>) = {
    if let Some(idx) = s.rfind(" until ") {
        let date_str = s[idx + 7..].trim();
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
        (&s[..idx], Some(RecurrenceEnd::Until(date)))
    } else if let Some(idx) = s.rfind(" count ") {
        let n: u32 = s[idx + 7..].trim().parse().ok()?;
        if n == 0 {
            return None;
        }
        (&s[..idx], Some(RecurrenceEnd::Count(n)))
    } else {
        (s.as_str(), None)
    }
};
```

Then run the *existing* frequency/BYDAY logic against `base` instead of `s`, and set `.end = end` on the result. The cleanest structure: rename the current body into `fn parse_freq(base: &str) -> Option<Recurrence>` (returns `end: None`), and have `parse` call it then overwrite `end`:

```rust
pub fn parse(input: &str) -> Option<Recurrence> {
    let s = /* normalized as today */;
    let (base, end) = /* split as above */;
    let mut rec = parse_freq(base)?;
    rec.end = end;
    Some(rec)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p tesela-core --lib recurrence`
Expected: PASS (all, including prior tasks).

- [ ] **Step 5: Commit**

```bash
git add crates/tesela-core/src/recurrence.rs
git commit -m "feat(core): parse until/count end conditions in recurring::"
```

---

## Task 4: `next_after` BYDAY stepping

**Files:**
- Modify: `crates/tesela-core/src/recurrence.rs`

- [ ] **Step 1: Write the failing tests + un-ignore the weekdays test**

Remove the `#[ignore]` from `next_after_weekdays_skips_weekend`. Add:

```rust
#[test]
fn next_after_byday_set() {
    // Mon/Wed/Fri. d(2026,5,11) is a Monday.
    let mwf = parse("every mon, wed, fri").unwrap();
    assert_eq!(next_after(&mwf, d(2026, 5, 11)), d(2026, 5, 13)); // Mon -> Wed
    assert_eq!(next_after(&mwf, d(2026, 5, 13)), d(2026, 5, 15)); // Wed -> Fri
    assert_eq!(next_after(&mwf, d(2026, 5, 15)), d(2026, 5, 18)); // Fri -> next Mon
    // From a day not in the set: Tue -> Wed.
    assert_eq!(next_after(&mwf, d(2026, 5, 12)), d(2026, 5, 13));
}

#[test]
fn next_after_weekends() {
    let we = parse("weekends").unwrap();
    assert_eq!(next_after(&we, d(2026, 5, 9)), d(2026, 5, 10)); // Sat -> Sun
    assert_eq!(next_after(&we, d(2026, 5, 10)), d(2026, 5, 16)); // Sun -> next Sat
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-core --lib recurrence::tests::next_after_byday_set`
Expected: FAIL (stub returns `anchor + 1 day`).

- [ ] **Step 3: Implement `next_by_weekday`**

Scan forward day-by-day from `anchor + 1` for the first date whose weekday is in the (non-empty) set. `interval` is treated as 1 for BYDAY in v1 — every eligible week (an `interval > 1` BYDAY is out of scope; the parser only ever produces `interval: 1` for BYDAY forms):

```rust
fn next_by_weekday(rec: &Recurrence, anchor: NaiveDate) -> NaiveDate {
    debug_assert!(!rec.by_weekday.is_empty());
    let mut d = anchor + Duration::days(1);
    // At most 7 steps reaches the next eligible weekday.
    for _ in 0..7 {
        if rec.by_weekday.contains(&d.weekday()) {
            return d;
        }
        d += Duration::days(1);
    }
    d // unreachable for a non-empty set
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p tesela-core --lib recurrence`
Expected: PASS — `next_after_byday_set`, `next_after_weekends`, and the un-ignored `next_after_weekdays_skips_weekend`.

- [ ] **Step 5: Commit**

```bash
git add crates/tesela-core/src/recurrence.rs
git commit -m "feat(core): next_after BYDAY stepping"
```

---

## Task 5: Series-end + advance/skip helpers

**Files:**
- Modify: `crates/tesela-core/src/recurrence.rs`

The engine needs a single place that answers "given the current occurrence and how many have completed, what's the next date — or is the series spent?".

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn advance_respects_until() {
    let r = parse("weekly until 2026-05-20").unwrap();
    // 2026-05-14 -> 2026-05-21 would be past `until` -> spent.
    assert_eq!(advance(&r, d(2026, 5, 14), 1), None);
    // A step that lands ON `until` is allowed.
    let r2 = parse("weekly until 2026-05-21").unwrap();
    assert_eq!(advance(&r2, d(2026, 5, 14), 1), Some(d(2026, 5, 21)));
}

#[test]
fn advance_respects_count() {
    let r = parse("daily count 3").unwrap();
    // count 3 = occurrences #1,#2,#3. done_so_far is the count already completed.
    assert_eq!(advance(&r, d(2026, 5, 7), 0), Some(d(2026, 5, 8))); // completing #1 -> #2
    assert_eq!(advance(&r, d(2026, 5, 8), 1), Some(d(2026, 5, 9))); // completing #2 -> #3
    assert_eq!(advance(&r, d(2026, 5, 9), 2), None);                // completing #3 -> spent
}

#[test]
fn advance_unbounded_never_spent() {
    let r = parse("daily").unwrap();
    assert_eq!(advance(&r, d(2026, 5, 7), 999), Some(d(2026, 5, 8)));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-core --lib recurrence::tests::advance_respects_count`
Expected: FAIL (`advance` not defined).

- [ ] **Step 3: Implement `advance`**

```rust
/// Compute the next occurrence after `current`, or `None` if completing
/// `current` exhausts the series.
///
/// `done_so_far` is the number of occurrences already completed *before*
/// this one — i.e. the engine-maintained `recurrence_done::` counter.
pub fn advance(rec: &Recurrence, current: NaiveDate, done_so_far: u32) -> Option<NaiveDate> {
    match rec.end {
        Some(RecurrenceEnd::Count(total)) => {
            // Completing `current` makes (done_so_far + 1) occurrences.
            // If that reaches the total, there is no next occurrence.
            if done_so_far + 1 >= total {
                return None;
            }
        }
        Some(RecurrenceEnd::Until(_)) | None => {}
    }
    let next = next_after(rec, current);
    if let Some(RecurrenceEnd::Until(until)) = rec.end {
        if next > until {
            return None;
        }
    }
    Some(next)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p tesela-core --lib recurrence`
Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
git add crates/tesela-core/src/recurrence.rs
git commit -m "feat(core): advance() with until/count series-end"
```

---

## Task 6: Server — multi-field anchor, `recurrence_done::`, skip mode

**Files:**
- Modify: the recurrence bump path in `tesela-server` (locate with `rg "apply_post_save_bumps" crates/tesela-server`) and the `recur-bump` route handler (`rg "recur-bump" crates/tesela-server`).

- [ ] **Step 1: Locate the current bump logic**

Run: `rg -n "apply_post_save_bumps|recur-bump|recurring" crates/tesela-server/src`
Read the function that, on a `status:: done` flip of a `recurring::` block, rewrites `deadline::` via `recurrence::next_after`. Note how it reads/writes block properties.

- [ ] **Step 2: Write the failing test**

In the server crate's recurrence test module (same file as the bump logic, or `tests/`), add a test that PUTs a note whose block is `recurring:: daily count 2` with `deadline:: 2026-05-07` `scheduled:: 2026-05-06` `status:: todo`, flips `status:: done`, and asserts: after the first done-flip both `deadline::` and `scheduled::` advanced by one day, `recurrence_done:: 1` was stamped, `status::` reset to `todo`; after the second done-flip the series is spent — no date change, `status::` stays `done`, `recurrence_done:: 2`.

Mirror the existing recurrence server test (`rg -n "recur" crates/tesela-server/src` for the closest example) for the harness shape.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p tesela-server recurrence`
Expected: FAIL.

- [ ] **Step 4: Implement the bump changes**

In the bump function:
1. Read `recurrence_done::` (default `0`) for the block.
2. Call `recurrence::advance(&rec, current, done_so_far)` instead of `next_after` directly, where `current` is the block's `deadline::` (or `scheduled::` if no deadline).
3. If `advance` returns `Some(next)`: advance **every** present date field — for each of `deadline::` and `scheduled::` that exists, set it to `recurrence::next_after(&rec, <that field's value>)` (each field steps independently, preserving offsets). Stamp `recurrence_done:: done_so_far + 1`. Reset `status:: todo`, stamp `last_completed::` with the prior date (unchanged behavior).
4. If `advance` returns `None`: the series is spent — leave `status:: done`, leave dates, set `recurrence_done::` to the `Count` total (or just `done_so_far + 1`). Do **not** strip `recurring::`.

- [ ] **Step 5: Add `mode` to the `recur-bump` endpoint**

The `POST /api/blocks/recur-bump` handler gains an optional body field `mode: "complete" | "skip"` (default `"complete"`). For `"skip"`: run the same `advance` + multi-field date step + `recurrence_done` increment, but do **not** flip `status::` to/from done and do **not** stamp `last_completed::`. Extract the shared date-stepping into one helper both modes call.

- [ ] **Step 6: Run tests**

Run: `cargo test -p tesela-server recurrence`
Expected: PASS.
Run: `cargo test -p tesela-core --lib recurrence`
Expected: PASS (unchanged).

- [ ] **Step 7: Commit**

```bash
git add crates/tesela-server/src
git commit -m "feat(server): multi-field recurrence anchor, recurrence_done counter, skip mode"
```

---

## Task 7: EventKit round-trip — BYDAY + end conditions

**Files:**
- Modify: `crates/tesela-server/src/reminders/darwin.rs`

- [ ] **Step 1: Write the failing test**

`darwin.rs` has an EK round-trip test (`rg -n "recurrence|EKRecurrence" crates/tesela-server/src/reminders/darwin.rs` near `#[cfg(test)]`). Add cases: a `Recurrence` with `by_weekday = [Mon, Wed, Fri]` and one with `end = Some(Count(10))` each survive `build_recurrence_rule` → (parse back) with equal value. If darwin tests are gated to macOS + a live store, instead add a pure unit test for the `Recurrence` → `EKRecurrenceRule` field mapping that doesn't require EventKit access.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-server --lib reminders`
Expected: FAIL.

- [ ] **Step 3: Generalize `build_recurrence_rule`**

Replace the `match` over the old enum (~line 622) with field-driven construction:
- `freq` → `EKRecurrenceFrequency` (`Daily`/`Weekly`/`Monthly`/`Yearly`).
- `interval` → the rule interval.
- `by_weekday` non-empty → an `EKRecurrenceDayOfWeek` array (generalize the existing `weekdays_rule` helper, which already builds that array for Mon-Fri — make it take any `&[Weekday]`).
- `end` → `EKRecurrenceEnd`: `Until(date)` → `EKRecurrenceEnd::endWithEndDate:` (convert `NaiveDate` → `NSDate`); `Count(n)` → `EKRecurrenceEnd::endWithOccurrenceCount:`.
Use the full `initRecurrenceWithFrequency_interval_daysOfTheWeek_..._end:` initializer when `by_weekday` is non-empty, the simple initializer otherwise.

- [ ] **Step 4: Extend the pull-side mapping**

Find where pull builds a `recurring::` string from an `EKRecurrenceRule` (`rg -n "recurring" darwin.rs` on the pull path). Map `daysOfTheWeek` → BYDAY tokens, `recurrenceEnd` → ` until `/` count `. Emit the same surface grammar `parse` accepts so the parsed-value diff stays stable.

- [ ] **Step 5: Run tests**

Run: `cargo test -p tesela-server`
Expected: PASS.
Run: `cargo build --release -p tesela-server`
Expected: compiles clean.

- [ ] **Step 6: Commit**

```bash
git add crates/tesela-server/src/reminders/darwin.rs
git commit -m "feat(server): EventKit round-trip for BYDAY + until/count recurrence"
```

---

## Self-Review

**Spec coverage:** BYDAY (Tasks 2,4,7) · `until`/`count` (Tasks 3,5,7) · skip (Task 6) · `weekends` (Tasks 2,4) · `scheduled::` recurrence / multi-field anchor (Task 6) · series-end "stays done" (Tasks 5,6) · model refactor (Task 1) · EK round-trip (Task 7). Web/iOS UI is intentionally deferred to the follow-up clients plan.

**Type consistency:** `Recurrence` / `Freq` / `RecurrenceEnd` defined in Task 1 and used unchanged through Task 7. `advance(&Recurrence, NaiveDate, u32) -> Option<NaiveDate>` (Task 5) is the single entry point Task 6 consumes. `recurrence_done::` is the property name used in both Tasks 5-tests and Task 6.

**Placeholder scan:** Task 6 uses `rg` to locate server internals rather than guessed line numbers — intentional, the executor confirms the exact site. All code-bearing steps carry real code.

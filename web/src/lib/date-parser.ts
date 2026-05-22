/**
 * Tiny natural-language date parser, Todoist-flavored.
 *
 * Returns an ISO date string (`YYYY-MM-DD`) or `null` if the input doesn't
 * match any known phrase. The matcher is intentionally narrow — the goal is
 * "the things you'd actually type", not full English date parsing.
 */

const WEEKDAYS: Record<string, number> = {
  sun: 0, sunday: 0,
  mon: 1, monday: 1,
  tue: 2, tues: 2, tuesday: 2,
  wed: 3, weds: 3, wednesday: 3,
  thu: 4, thur: 4, thurs: 4, thursday: 4,
  fri: 5, friday: 5,
  sat: 6, saturday: 6,
};

const MONTHS: Record<string, number> = {
  jan: 0, january: 0,
  feb: 1, february: 1,
  mar: 2, march: 2,
  apr: 3, april: 3,
  may: 4,
  jun: 5, june: 5,
  jul: 6, july: 6,
  aug: 7, august: 7,
  sep: 8, sept: 8, september: 8,
  oct: 9, october: 9,
  nov: 10, november: 10,
  dec: 11, december: 11,
};

function fmt(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${dd}`;
}

function addDays(base: Date, days: number): Date {
  const d = new Date(base);
  d.setDate(d.getDate() + days);
  return d;
}

function makeDate(y: number, m: number, d: number): Date | null {
  const dt = new Date(y, m, d);
  if (dt.getFullYear() !== y || dt.getMonth() !== m || dt.getDate() !== d) return null;
  return dt;
}

/** Next occurrence of the given weekday (1–7 days ahead — never today). */
function nextWeekday(base: Date, target: number): Date {
  const cur = base.getDay();
  const delta = ((target - cur + 7) % 7) || 7;
  return addDays(base, delta);
}

/** Soonest occurrence of the given weekday (today if today matches, else upcoming). */
function soonestWeekday(base: Date, target: number): Date {
  const cur = base.getDay();
  const delta = (target - cur + 7) % 7;
  return addDays(base, delta);
}

export type ParsedDateTime = { date: string; time: string | null };
export type ParsedDateTimeRecurrence = ParsedDateTime & { recurrence: string | null };

/**
 * Parse a recurrence phrase. Returns the canonical string we store in
 * `recurring::` (e.g. `"monthly"`, `"every 2 weeks"`, `"weekdays"`,
 * `"every mon, wed, fri"`, `"weekly until 2026-12-31"`) or `null` if
 * unrecognized. The Rust side (`tesela-core::recurrence`) is the source
 * of truth — this mirror is only used so the picker can show "valid"
 * feedback before round-tripping through the server.
 */

const WEEKDAY_TOKENS: Record<string, string> = {
  mon: "mon", monday: "mon",
  tue: "tue", tues: "tue", tuesday: "tue",
  wed: "wed", wednesday: "wed",
  thu: "thu", thur: "thu", thurs: "thu", thursday: "thu",
  fri: "fri", friday: "fri",
  sat: "sat", saturday: "sat",
  sun: "sun", sunday: "sun",
};
const WEEKDAY_ORDER = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

export function parseRecurrenceInput(input: string): string | null {
  const s = input.trim().toLowerCase().replace(/\s+/g, " ");
  if (!s) return null;

  // Split off a trailing end clause: " until YYYY-MM-DD" or " count N".
  let base = s;
  let endClause = "";
  const untilIdx = s.lastIndexOf(" until ");
  const countIdx = s.lastIndexOf(" count ");
  if (untilIdx !== -1) {
    const dateStr = s.slice(untilIdx + 7).trim();
    if (!/^\d{4}-\d{2}-\d{2}$/.test(dateStr)) return null;
    // `Date.parse` rolls overflow dates forward (2026-02-30 → Mar 2), so it
    // would accept dates the Rust `chrono` parser rejects. Round-trip the
    // components instead to catch a genuinely invalid calendar date.
    const [y, mo, d] = dateStr.split("-").map(Number);
    const dt = new Date(y, mo - 1, d);
    if (dt.getFullYear() !== y || dt.getMonth() !== mo - 1 || dt.getDate() !== d) return null;
    base = s.slice(0, untilIdx);
    endClause = ` until ${dateStr}`;
  } else if (countIdx !== -1) {
    const n = Number(s.slice(countIdx + 7).trim());
    if (!Number.isInteger(n) || n < 1) return null;
    base = s.slice(0, countIdx);
    endClause = ` count ${n}`;
  }

  const freq = parseRecurrenceFreq(base);
  return freq === null ? null : freq + endClause;
}

function parseRecurrenceFreq(base: string): string | null {
  switch (base) {
    case "daily":   case "every day":   return "daily";
    case "weekly":  case "every week":  return "weekly";
    case "monthly": case "every month": return "monthly";
    case "yearly":  case "annually":    case "every year": return "yearly";
    case "weekdays": return "weekdays";
    case "weekends": return "weekends";
  }

  if (base.startsWith("every ")) {
    const rest = base.slice(6);

    // BYDAY: "every mon, wed, fri" — all comma-separated tokens must be weekdays.
    const tokens = rest.split(",").map((t) => t.trim());
    if (rest && tokens.every((t) => WEEKDAY_TOKENS[t] !== undefined)) {
      const days = [...new Set(tokens.map((t) => WEEKDAY_TOKENS[t]))]
        .sort((a, b) => WEEKDAY_ORDER.indexOf(a) - WEEKDAY_ORDER.indexOf(b));
      return `every ${days.join(", ")}`;
    }

    // "every N <unit>" — only when the rest contains a space (not matched as BYDAY above).
    const m = rest.match(/^(\d+) (day|days|week|weeks|month|months|year|years)$/);
    if (m) {
      const n = Number(m[1]);
      if (!Number.isFinite(n) || n < 1) return null;
      const unit = m[2].endsWith("s") ? m[2] : `${m[2]}s`;
      if (n === 1) {
        if (unit === "days") return "daily";
        if (unit === "weeks") return "weekly";
        if (unit === "months") return "monthly";
        if (unit === "years") return "yearly";
      }
      return `every ${n} ${unit}`;
    }
  }

  return null;
}

/**
 * Trailing recurrence matcher for the DatePicker NL input. Allows phrases
 * like `"fri weekly"` or `"may 1 every 2 weeks"` — strips the recurrence
 * tail off, leaving the rest for `parseDateInput`. Returns the canonical
 * recurrence string and the remainder (or both nulls if no tail matched).
 */
// Matches a trailing recurrence phrase — group 1 captures the entire phrase
// including any optional end clause (" until YYYY-MM-DD" or " count N"), so
// extractRecurrence can pass it directly to parseRecurrenceInput.
//
// Supported base forms:
//   - simple keywords: daily, weekly, monthly, yearly, annually, weekdays, weekends
//   - "every N <unit>": every 2 weeks, every 3 days, etc.
//   - "every <day|week|month|year>": aliases for interval-1 forms
//   - BYDAY day-sets: "every mon, wed, fri" (one or more comma-separated weekday tokens)
const _BYDAY_TOKEN = "(?:mon(?:day)?|tues?(?:day)?|wed(?:nesday)?|thu(?:rs?(?:day)?)?|fri(?:day)?|sat(?:urday)?|sun(?:day)?)";
const _BYDAY_SET = `every\\s+${_BYDAY_TOKEN}(?:\\s*,\\s*${_BYDAY_TOKEN})*`;
const _END_CLAUSE = "(?:\\s+until\\s+\\d{4}-\\d{2}-\\d{2}|\\s+count\\s+\\d+)?";
const TRAILING_RECUR_RE = new RegExp(
  `\\s+((?:daily|weekly|monthly|yearly|annually|weekdays|weekends|every\\s+\\d+\\s+(?:days?|weeks?|months?|years?)|every\\s+(?:day|week|month|year)|${_BYDAY_SET})${_END_CLAUSE})$`,
  "i",
);
function extractRecurrence(s: string): { recurrence: string | null; rest: string } {
  const m = s.match(TRAILING_RECUR_RE);
  if (!m) return { recurrence: null, rest: s };
  const tail = m[1].toLowerCase();
  const rec = parseRecurrenceInput(tail);
  if (!rec) return { recurrence: null, rest: s };
  return { recurrence: rec, rest: s.slice(0, m.index!).trim() };
}

/**
 * Parse a natural-language phrase that may contain date + time + recurrence.
 * Recurrence and time are independent — either, both, or neither may be
 * present. Returns null only when the date portion is unrecognized.
 */
export function parseDateAndRecurrenceInput(
  input: string,
  today: Date = new Date(),
): ParsedDateTimeRecurrence | null {
  const raw = input.trim().toLowerCase();
  if (!raw) return null;
  const recExtracted = extractRecurrence(raw);
  const parsed = parseDateInput(recExtracted.rest, today);
  if (!parsed) return null;
  return { ...parsed, recurrence: recExtracted.recurrence };
}

// Trailing time matcher: optional "at " prefix, hours, optional :minutes,
// optional am/pm. Anchored to end-of-string. We only TREAT a tail as time
// when it has either a colon, an am/pm marker, OR was preceded by "at" —
// otherwise "fri 10" is ambiguous and we leave it for the date parser.
const TRAILING_TIME_RE = /(?:^|\s)(at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?$/i;

function extractTime(s: string): { time: string | null; rest: string } {
  if (s === "noon") return { time: "12:00", rest: "today" };
  if (s === "midnight") return { time: "00:00", rest: "today" };

  const m = s.match(TRAILING_TIME_RE);
  if (!m) return { time: null, rest: s };

  const hasAt = !!m[1];
  const hasColon = m[3] !== undefined;
  const hasAmPm = !!m[4];
  if (!hasAt && !hasColon && !hasAmPm) return { time: null, rest: s };

  let h = Number(m[2]);
  const mins = m[3] ? Number(m[3]) : 0;
  const ampm = m[4]?.toLowerCase();
  if (ampm === "pm" && h < 12) h += 12;
  if (ampm === "am" && h === 12) h = 0;
  if (h < 0 || h > 23 || mins < 0 || mins > 59) return { time: null, rest: s };

  const time = `${String(h).padStart(2, "0")}:${String(mins).padStart(2, "0")}`;
  const rest = s.slice(0, m.index! + (m[0].startsWith(" ") ? 0 : 0)).trim();
  return { time, rest: rest || "today" };
}

/**
 * Parse a natural-language date+time phrase. Returns date and optional
 * 24-hour time, or null on failure. Time-only input ("noon", "10am")
 * defaults the date to today.
 */
export function parseDateInput(input: string, today: Date = new Date()): ParsedDateTime | null {
  const raw = input.trim().toLowerCase();
  if (!raw) return null;

  const { time, rest } = extractTime(raw);
  const s = rest;

  const datePart = parseDatePart(s, today);
  if (datePart === null) return null;
  return { date: datePart, time };
}

function parseDatePart(s: string, today: Date): string | null {
  if (s === "today" || s === "tod") return fmt(today);
  if (s === "tomorrow" || s === "tom" || s === "tmrw") return fmt(addDays(today, 1));
  if (s === "yesterday" || s === "yes" || s === "yest") return fmt(addDays(today, -1));

  const iso = s.match(/^(\d{4})-(\d{1,2})-(\d{1,2})$/);
  if (iso) {
    const d = makeDate(Number(iso[1]), Number(iso[2]) - 1, Number(iso[3]));
    return d ? fmt(d) : null;
  }

  const slash = s.match(/^(\d{1,2})\/(\d{1,2})(?:\/(\d{2}|\d{4}))?$/);
  if (slash) {
    const m = Number(slash[1]) - 1;
    const day = Number(slash[2]);
    let year: number;
    if (slash[3]) {
      const y = Number(slash[3]);
      year = y < 100 ? 2000 + y : y;
    } else {
      year = today.getFullYear();
      const candidate = makeDate(year, m, day);
      if (candidate && candidate < new Date(today.getFullYear(), today.getMonth(), today.getDate())) {
        year += 1;
      }
    }
    const d = makeDate(year, m, day);
    return d ? fmt(d) : null;
  }

  // "apr 15" / "april 15" / "15 apr" — optional year
  const monthName = s.match(/^(?:(\d{1,2})\s+)?([a-z]+)(?:\s+(\d{1,2}))?(?:[,\s]+(\d{2}|\d{4}))?$/);
  if (monthName) {
    const m = MONTHS[monthName[2]];
    if (m !== undefined) {
      const dayStr = monthName[1] ?? monthName[3];
      if (dayStr) {
        const day = Number(dayStr);
        let year: number;
        if (monthName[4]) {
          const y = Number(monthName[4]);
          year = y < 100 ? 2000 + y : y;
        } else {
          year = today.getFullYear();
          const candidate = makeDate(year, m, day);
          if (candidate && candidate < new Date(today.getFullYear(), today.getMonth(), today.getDate())) {
            year += 1;
          }
        }
        const d = makeDate(year, m, day);
        if (d) return fmt(d);
      }
    }
  }

  if (s === "next week" || s === "nw") return fmt(addDays(today, 7));

  // "next <weekday>" — strictly future (skips today)
  const nextWd = s.match(/^next\s+([a-z]+)$/);
  if (nextWd) {
    const wd = WEEKDAYS[nextWd[1]];
    if (wd !== undefined) return fmt(nextWeekday(today, wd));
  }

  if (WEEKDAYS[s] !== undefined) return fmt(soonestWeekday(today, WEEKDAYS[s]));

  // "in N days/weeks/months"
  const inN = s.match(/^in\s+(\d+)\s+(day|days|week|weeks|month|months|d|w|mo)$/);
  if (inN) {
    const n = Number(inN[1]);
    const unit = inN[2];
    if (unit.startsWith("d")) return fmt(addDays(today, n));
    if (unit.startsWith("w")) return fmt(addDays(today, n * 7));
    const d = new Date(today);
    d.setMonth(d.getMonth() + n);
    return fmt(d);
  }

  // "<N>d" / "<N>w" shorthand
  const shortN = s.match(/^(\d+)(d|w)$/);
  if (shortN) {
    const n = Number(shortN[1]);
    return fmt(addDays(today, shortN[2] === "w" ? n * 7 : n));
  }

  return null;
}

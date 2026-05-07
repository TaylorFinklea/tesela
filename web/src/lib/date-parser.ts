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

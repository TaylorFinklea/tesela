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

export function parseDateInput(input: string, today: Date = new Date()): string | null {
  const s = input.trim().toLowerCase();
  if (!s) return null;

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

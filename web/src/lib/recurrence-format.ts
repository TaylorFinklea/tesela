const DAY_LABEL: Record<string, string> = {
  mon: "Mon", tue: "Tue", wed: "Wed", thu: "Thu", fri: "Fri", sat: "Sat", sun: "Sun",
};

/** Human-readable rendering of a `recurring::` value. Unrecognized input
 *  is returned unchanged (never throws). */
export function formatRecurrence(value: string): string {
  const s = value.trim().toLowerCase().replace(/\s+/g, " ");
  if (!s) return value;

  let base = s;
  let endText = "";
  const untilIdx = s.lastIndexOf(" until ");
  const countIdx = s.lastIndexOf(" count ");
  if (untilIdx !== -1) {
    base = s.slice(0, untilIdx);
    const date = new Date(s.slice(untilIdx + 7).trim() + "T00:00:00");
    endText = Number.isNaN(date.getTime())
      ? ""
      : ` until ${date.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}`;
  } else if (countIdx !== -1) {
    base = s.slice(0, countIdx);
    endText = `, ${s.slice(countIdx + 7).trim()}×`;
  }

  const freq = formatFreq(base);
  return freq === null ? value : freq + endText;
}

const OTHER_UNIT_LABEL: Record<string, string> = {
  day: "day", days: "day",
  week: "week", weeks: "week",
  month: "month", months: "month",
  year: "year", years: "year",
};

function formatFreq(base: string): string | null {
  switch (base) {
    case "daily": return "Daily";
    case "weekly": return "Weekly";
    case "monthly": return "Monthly";
    case "yearly": return "Yearly";
    // Single-word cadences (Rust recurrence.rs, added 2026-06-20).
    case "biweekly": return "Biweekly";
    case "fortnightly": return "Fortnightly";
    case "quarterly": return "Quarterly";
    case "weekdays": return "Weekdays";
    case "every weekday": case "every weekdays": return "Weekdays";
    case "weekends": return "Weekends";
  }
  if (base.startsWith("every ")) {
    const rest = base.slice(6);
    const tokens = rest.split(",").map((t) => t.trim());
    if (rest && tokens.every((t) => DAY_LABEL[t] !== undefined)) {
      return tokens.map((t) => DAY_LABEL[t]).join(", ");
    }
    // "every other <unit>" → interval 2 (added 2026-06-20).
    if (rest.startsWith("other ")) {
      const unit = rest.slice(6);
      const label = OTHER_UNIT_LABEL[unit];
      return label ? `Every other ${label}` : null;
    }
    const m = rest.match(/^(\d+) (days?|weeks?|months?|years?)$/);
    if (m) return `Every ${m[1]} ${m[2]}`;
  }
  return null;
}

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
      : ` until ${date.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })}`;
  } else if (countIdx !== -1) {
    base = s.slice(0, countIdx);
    endText = `, ${s.slice(countIdx + 7).trim()}×`;
  }

  const freq = formatFreq(base);
  return freq === null ? value : freq + endText;
}

function formatFreq(base: string): string | null {
  switch (base) {
    case "daily": return "Daily";
    case "weekly": return "Weekly";
    case "monthly": return "Monthly";
    case "yearly": return "Yearly";
    case "weekdays": return "Weekdays";
    case "weekends": return "Weekends";
  }
  if (base.startsWith("every ")) {
    const rest = base.slice(6);
    const tokens = rest.split(",").map((t) => t.trim());
    if (rest && tokens.every((t) => DAY_LABEL[t] !== undefined)) {
      return tokens.map((t) => DAY_LABEL[t]).join(", ");
    }
    const m = rest.match(/^(\d+) (days?|weeks?|months?|years?)$/);
    if (m) return `Every ${m[1]} ${m[2]}`;
  }
  return null;
}

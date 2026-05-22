/** Human-readable rendering of a date property value. Accepts a bare
 *  `YYYY-MM-DD` (optionally ` HH:mm`) or a `[[YYYY-MM-DD]]`-wrapped value.
 *  Unrecognized input is returned trimmed-but-unchanged. */
export function formatDateMonthDay(v: string): string {
  // Accept `[[YYYY-MM-DD]]` (optionally followed by ` HH:mm`) and bare ISO.
  const m = v.trim().match(/^\[\[(\d{4})-(\d{2})-(\d{2})\]\](?:\s+(\d{2}):(\d{2}))?$/) ||
            v.trim().match(/^(\d{4})-(\d{2})-(\d{2})(?:\s+(\d{2}):(\d{2}))?$/);
  if (!m) return v.trim();
  const [, y, mo, d, hh, mm] = m;
  const date = new Date(Number(y), Number(mo) - 1, Number(d));
  const month = date.toLocaleString("en-US", { month: "short" });
  const day = Number(d);
  const thisYear = new Date().getFullYear();
  const datePart = Number(y) === thisYear ? `${month} ${day}` : `${month} ${day}, ${y}`;
  if (!hh) return datePart;
  let h = Number(hh);
  const ampm = h >= 12 ? "p" : "a";
  h = h % 12 || 12;
  const minStr = mm === "00" ? "" : `:${mm}`;
  return `${datePart} ${h}${minStr}${ampm}`;
}

/**
 * Tiny line-level diff using LCS (longest common subsequence). Sufficient
 * for the History tab's `+N/-M` summary and the diff modal's side-by-side
 * view. Pure function, no dependencies.
 *
 * Output rows are tagged with a `kind`:
 *   - "same"   — line exists unchanged in both
 *   - "added"  — line exists only in `next`
 *   - "removed"— line exists only in `prev`
 */

export type DiffRow =
  | { kind: "same"; text: string; prevLine: number; nextLine: number }
  | { kind: "removed"; text: string; prevLine: number }
  | { kind: "added"; text: string; nextLine: number };

export type DiffSummary = {
  added: number;
  removed: number;
  rows: DiffRow[];
};

/**
 * Compute a line-level diff between `prev` and `next`. O(n*m) memory; fine
 * for the typical note size (~50–500 lines). Replace with patience-diff if
 * notes ever grow past several thousand lines.
 */
export function lineDiff(prev: string, next: string): DiffSummary {
  const a = prev.split("\n");
  const b = next.split("\n");
  const n = a.length;
  const m = b.length;

  // LCS table
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (a[i] === b[j]) lcs[i][j] = lcs[i + 1][j + 1] + 1;
      else lcs[i][j] = Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }

  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  let added = 0;
  let removed = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      rows.push({ kind: "same", text: a[i], prevLine: i + 1, nextLine: j + 1 });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      rows.push({ kind: "removed", text: a[i], prevLine: i + 1 });
      removed++;
      i++;
    } else {
      rows.push({ kind: "added", text: b[j], nextLine: j + 1 });
      added++;
      j++;
    }
  }
  while (i < n) {
    rows.push({ kind: "removed", text: a[i], prevLine: i + 1 });
    removed++;
    i++;
  }
  while (j < m) {
    rows.push({ kind: "added", text: b[j], nextLine: j + 1 });
    added++;
    j++;
  }
  return { added, removed, rows };
}

/**
 * Format a relative-time label for a UTC timestamp string. Examples:
 *   "now", "2m ago", "3h ago", "Yesterday 9:42 AM", "2026-04-28 14:03"
 */
export function relativeTime(iso: string, ref: Date = new Date()): string {
  // The server stores `datetime('now')` which is UTC without a timezone marker.
  // Append `Z` so the JS parser doesn't treat it as local.
  const utc = iso.endsWith("Z") || /[+-]\d{2}:?\d{2}$/.test(iso) ? iso : `${iso}Z`;
  const d = new Date(utc);
  const diffMs = ref.getTime() - d.getTime();
  const sec = Math.max(0, Math.floor(diffMs / 1000));
  if (sec < 60) return "now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 7) {
    const time = d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
    if (day === 1) return `Yesterday ${time}`;
    return `${day}d ago ${time}`;
  }
  return d.toLocaleString("en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

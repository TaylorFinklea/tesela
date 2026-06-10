/**
 * Pure date-walk helpers for JournalView's `visibleDailies` derivation. Kept
 * Svelte-free so the loop-termination contract is unit-tested directly
 * (web/tests/unit/journal-dates.test.mjs) — the in-component predecessor
 * (`while (true)` exiting only on `cursor === oldest`) hard-hung the page
 * when every on-disk daily was future-dated (e.g. a fresh mosaic whose only
 * daily synced from a TZ-ahead peer): the cursor only steps backward, so a
 * future `oldest` was never reached and a synthetic Note was allocated per
 * iteration, forever.
 */

/** Step `dateStr` back by one day. Pure UTC-friendly string math so the
 *  calendar stays consistent regardless of TZ. */
export function prevDate(dateStr: string): string {
  const d = new Date(dateStr + "T12:00:00Z"); // noon UTC sidesteps DST edges
  d.setUTCDate(d.getUTCDate() - 1);
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/**
 * The descending, gap-free list of daily dates the journal renders BEFORE the
 * padding tail: from `max(newest, today)` down to `min(oldest, today)`
 * inclusive. `newest`/`oldest` are the newest/oldest on-disk daily titles in
 * the visible window (`YYYY-MM-DD`); plain string comparison IS date order
 * for that shape.
 *
 *  - Normal case (oldest ≤ today): today → oldest, gap-free — unchanged
 *    behavior.
 *  - A future-dated `newest` (peer across the dateline created "tomorrow"):
 *    the walk STARTS there, so the future daily renders instead of being
 *    silently dropped.
 *  - A future-dated `oldest` (EVERY daily is future-dated): the walk still
 *    ends at today and terminates — this was the unbounded-loop hang.
 *
 * Today is always included (start ≥ today ≥ end), and every on-disk daily in
 * [oldest, newest] falls inside [end, start], so no post-loop append guard is
 * needed. The loop is bounded by construction: the cursor strictly decreases
 * one day per iteration toward `end`.
 */
export function dailyWalkDates(todayStr: string, newest: string, oldest: string): string[] {
  const start = newest > todayStr ? newest : todayStr;
  const end = oldest < todayStr ? oldest : todayStr;
  const out: string[] = [];
  let cursor = start;
  while (cursor >= end) {
    out.push(cursor);
    cursor = prevDate(cursor);
  }
  return out;
}

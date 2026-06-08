/**
 * Task-token detection for the editor's "detect-inline, lift-below" gesture
 * (Model B, decided 2026-06-08). Scans a block's FIRST line (the prose line —
 * never the `tags::`/`status::` continuation lines) for inline tokens that
 * should lift OUT of the text into structured properties when the block is
 * committed (Enter / blur).
 *
 * Part 2a covers priority (`p1`..`p4`). Part 2b adds natural-language dates
 * (delegating to `parseDateInput` from date-parser.ts).
 *
 * The WRITE side of the lift is `onSetProperty` (a structured container op),
 * NOT a `key:: value` text line — see the Model-B spec / P1.13 read-model.
 */
import { priorityLevel } from "./priority";

export type LiftedProp = { key: string; value: string };
export type DetectResult = { stripped: string; props: LiftedProp[] };
export type TokenRange = { from: number; to: number; level: 1 | 2 | 3 | 4 };

/** A standalone p1..p4 token (word-boundaried: matches `p1`, not `sp1`/`p10`). */
const PRIORITY_RE = /\bp[1-4]\b/gi;

/** First-line of the block (the prose; continuation lines start after \n). */
function firstLine(text: string): string {
  const nl = text.indexOf("\n");
  return nl === -1 ? text : text.slice(0, nl);
}

/** Priority token ranges in the block's first line — for inline highlighting. */
export function priorityTokenRanges(text: string): TokenRange[] {
  const line0 = firstLine(text);
  const out: TokenRange[] = [];
  PRIORITY_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = PRIORITY_RE.exec(line0)) !== null) {
    const level = priorityLevel(m[0]);
    if (level) out.push({ from: m.index, to: m.index + m[0].length, level });
  }
  return out;
}

/**
 * Detect liftable tokens in `text` (first line only). Returns the text with the
 * tokens removed + the structured props to set, or the input unchanged + `[]`
 * when nothing is detected. Last priority token wins; all are stripped.
 */
export function detectTaskTokens(text: string): DetectResult {
  const nl = text.indexOf("\n");
  const line0 = nl === -1 ? text : text.slice(0, nl);
  const rest = nl === -1 ? "" : text.slice(nl);

  const ranges = priorityTokenRanges(line0);
  if (ranges.length === 0) return { stripped: text, props: [] };

  const value = `p${ranges[ranges.length - 1].level}`;
  let s = line0;
  for (const r of [...ranges].sort((a, b) => b.from - a.from)) {
    s = s.slice(0, r.from) + s.slice(r.to);
  }
  s = s.replace(/ {2,}/g, " ").trim();
  return { stripped: s + rest, props: [{ key: "priority", value }] };
}

/**
 * Task-token detection for the editor's "detect-inline, lift-below" gesture
 * (Model B). Scans a block's FIRST line (prose) for inline tokens — priority
 * (`p1`..`p4`) and natural-language dates (`today` / `next tuesday` / `jun 9` /
 * `in 3 days` / ISO; a leading `due`/`deadline` keyword → deadline, else
 * scheduled; a trailing recurrence rides along) — that lift OUT of the text into
 * structured properties when the block is committed.
 *
 * GATING (whether detection runs at all) is the CALLER's job: detection only
 * applies to blocks carrying a detect-enabled tag (default `#Task`) — see the
 * Model-B spec / decisions.md 2026-06-08. This module stays pure.
 *
 * The WRITE side of the lift is `onSetProperty` (a structured container op), NOT
 * a `key:: value` text line (P1.13 read-model). Date validation + NL parsing is
 * delegated to `parseDateAndRecurrenceInput` (date-parser.ts) — not reinvented.
 */
import { priorityLevel } from "./priority";
import { parseDateAndRecurrenceInput } from "./date-parser";

export type BareDateField = "scheduled" | "deadline";

export type DetectedToken = {
  from: number;
  to: number;
  kind: "priority" | "date";
  /** Property to set on lift: "priority" | "scheduled" | "deadline". */
  key: string;
  /** Stored value: "p1".."p4" or bare "YYYY-MM-DD" (matches the DatePicker). */
  value: string;
  /** Priority level — for the inline highlight color; priority tokens only. */
  level?: 1 | 2 | 3 | 4;
  /** Recurrence rule lifted alongside a date (→ `recurring::`), if any. */
  recurrence?: string | null;
};

export type LiftedProp = { key: string; value: string };
export type DetectResult = { stripped: string; props: LiftedProp[] };

/** A standalone p1..p4 token (word-boundaried: matches `p1`, not `sp1`/`p10`). */
const PRIORITY_RE = /\bp[1-4]\b/gi;
/** Date phrases are short ("next tuesday", "in 3 days at 10am"). */
const MAX_DATE_WORDS = 5;

function firstLine(text: string): string {
  const nl = text.indexOf("\n");
  return nl === -1 ? text : text.slice(0, nl);
}

/**
 * Detect priority + date tokens in the block's first line. Ranges are
 * doc-absolute (the first line starts at index 0). The caller decides whether
 * to run this at all (tag gating).
 */
export function detectTokens(text: string, bareDateField: BareDateField = "scheduled"): DetectedToken[] {
  const line0 = firstLine(text);
  const out: DetectedToken[] = [];

  // Priority (p1..p4).
  PRIORITY_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = PRIORITY_RE.exec(line0)) !== null) {
    const level = priorityLevel(m[0]);
    if (level) {
      out.push({ from: m.index, to: m.index + m[0].length, kind: "priority", key: "priority", value: `p${level}`, level });
    }
  }

  // Dates — scan word windows longest-first, delegating validation to the
  // parser. The longest valid window from each start wins (so "next tuesday"
  // beats "tuesday", and trailing prose like "tuesday with sam" → "tuesday").
  const words = [...line0.matchAll(/\S+/g)].map((w) => ({ start: w.index!, end: w.index! + w[0].length }));
  let i = 0;
  while (i < words.length) {
    let best: { len: number; from: number; to: number; date: string; field: BareDateField | null; recurrence: string | null } | null = null;
    const maxLen = Math.min(MAX_DATE_WORDS, words.length - i);
    for (let len = maxLen; len >= 1; len--) {
      const from = words[i].start;
      const to = words[i + len - 1].end;
      const parsed = parseDateAndRecurrenceInput(line0.slice(from, to));
      if (parsed) {
        best = { len, from, to, date: parsed.date, field: parsed.field, recurrence: parsed.recurrence };
        break;
      }
    }
    if (best) {
      const overlapsPriority = out.some((t) => t.kind === "priority" && t.from < best!.to && t.to > best!.from);
      if (!overlapsPriority) {
        out.push({ from: best.from, to: best.to, kind: "date", key: best.field ?? bareDateField, value: best.date, recurrence: best.recurrence });
      }
      i += best.len;
    } else {
      i++;
    }
  }

  return out.sort((a, b) => a.from - b.from);
}

/** Priority-only ranges (with level) for the inline highlight's per-level color. */
export function priorityTokenRanges(text: string): Array<{ from: number; to: number; level: 1 | 2 | 3 | 4 }> {
  return detectTokens(text)
    .filter((t) => t.kind === "priority")
    .map((t) => ({ from: t.from, to: t.to, level: t.level! }));
}

/**
 * Detect + strip tokens from `text` (first line only). Returns the stripped text
 * + the structured props to set, or the input unchanged + `[]` when nothing is
 * detected. Last priority wins; a recurrence rides with its date.
 */
export function detectTaskTokens(text: string, bareDateField: BareDateField = "scheduled"): DetectResult {
  const nl = text.indexOf("\n");
  const line0 = nl === -1 ? text : text.slice(0, nl);
  const rest = nl === -1 ? "" : text.slice(nl);

  const tokens = detectTokens(line0, bareDateField);
  if (tokens.length === 0) return { stripped: text, props: [] };

  const props: LiftedProp[] = [];
  let priorityValue: string | null = null;
  for (const t of tokens) {
    if (t.kind === "priority") {
      priorityValue = t.value; // last wins
      continue;
    }
    props.push({ key: t.key, value: t.value });
    if (t.recurrence) props.push({ key: "recurring", value: t.recurrence });
  }
  if (priorityValue) props.push({ key: "priority", value: priorityValue });

  let s = line0;
  for (const t of [...tokens].sort((a, b) => b.from - a.from)) {
    s = s.slice(0, t.from) + s.slice(t.to);
  }
  s = s.replace(/ {2,}/g, " ").trim();
  return { stripped: s + rest, props };
}

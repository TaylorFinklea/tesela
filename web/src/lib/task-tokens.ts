/**
 * Config-driven task-token detection (Model B). Scans a block's FIRST line for
 * inline tokens that lift OUT of the prose into structured properties when the
 * block is committed (blur). What gets detected is driven entirely by the
 * block's detect-enabled tag(s) and each property's `value_type` + `nl_triggers`
 * — no hardcoded property names:
 *
 *   • select (e.g. Priority `["p1".."p4"]`): a trigger IS the value token.
 *   • number (e.g. Points `["points","pts"]`): `<number> <trigger>`.
 *   • date   (e.g. Deadline `["due","deadline"]`): `<trigger> <NL date+time>`.
 *   • the tag's DEFAULT date property: a BARE natural-language date (no trigger).
 *
 * Date validation + NL parsing (incl. multi-word "next tuesday", time, and
 * recurrence) is delegated to `parseDateAndRecurrenceInput` (date-parser.ts).
 * GATING (does detection run) is the caller's job via `resolveDetectSpec` on the
 * block's DIRECT tags — never inherited. The WRITE side is `onSetProperty`.
 */
// Explicit .ts extensions so node's type-stripping test runner can resolve
// these (the unit suite imports this module directly — cf. scratch-prune-pure).
import { priorityLevel } from "./priority.ts";
import { parseDateAndRecurrenceInput } from "./date-parser.ts";

/** A single property's detection rule, resolved from its Property page. */
export type PropertySpec = {
  /** lowercased property key — "priority" | "deadline" | "points" | … */
  key: string;
  /** lowercased value_type — "select" | "date" | "number" | … */
  valueType: string;
  choices: string[];
  /** lowercased nl_triggers. */
  triggers: string[];
};
/** The detection spec resolved for one block (its matched tags' properties). */
export type DetectSpec = { properties: PropertySpec[]; defaultDateProperty: string };
/** Per detect-enabled tag. */
export type TagDetectSpec = { defaultDateProperty: string; properties: PropertySpec[] };
/** The whole config (keyed by lowercased tag name) — carried via a CM facet. */
export type DetectConfig = Map<string, TagDetectSpec>;

export type DetectedToken = {
  from: number;
  to: number;
  key: string;
  value: string;
  /** Drives the inline highlight color. */
  kind: "priority" | "date" | "number" | "select";
  /** Priority level (priority tokens only), for per-level color. */
  level?: 1 | 2 | 3 | 4;
  recurrence?: string | null;
};
export type LiftedProp = { key: string; value: string };
export type DetectResult = { stripped: string; props: LiftedProp[] };

const MAX_DATE_WORDS = 5;

function firstLine(text: string): string {
  const nl = text.indexOf("\n");
  return nl === -1 ? text : text.slice(0, nl);
}
function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
function dateValue(p: { date: string; time: string | null }): string {
  return p.time ? `${p.date} ${p.time}` : p.date;
}

/**
 * Resolve the detection spec for a block from its DIRECT tags (own tags:: +
 * inline #tags, NEVER inherited). Returns null when no tag is detect-enabled —
 * i.e. detection is OFF for this block.
 */
export function resolveDetectSpec(blockTags: string[], config: DetectConfig): DetectSpec | null {
  const matched = blockTags.map((t) => t.toLowerCase()).filter((t) => config.has(t));
  if (matched.length === 0) return null;
  const properties: PropertySpec[] = [];
  const seen = new Set<string>();
  let defaultDateProperty = "scheduled";
  for (const tag of matched) {
    const spec = config.get(tag)!;
    if (spec.defaultDateProperty) defaultDateProperty = spec.defaultDateProperty;
    for (const p of spec.properties) {
      if (!seen.has(p.key)) {
        seen.add(p.key);
        properties.push(p);
      }
    }
  }
  return { properties, defaultDateProperty };
}

type WordSpan = { start: number; end: number };
type DateHit = { from: number; to: number; endWord: number; date: string; time: string | null; recurrence: string | null };

/**
 * Literal ranges on the first line that detection must never lift tokens out
 * of: `[[wiki links]]`, markdown links/images `[text](url)`, bare URLs, and
 * inline `` `code` `` spans. Pre-claimed in `detectTokens` so a trigger word
 * inside a link target / code span is never detected — and therefore never
 * stripped by the blur lift (`detectTaskTokens`), which would otherwise
 * rewrite the link target ("p1" excised from a URL, a date word lifted out of
 * a `[[wiki link]]`). Because cm-decorations' highlight layer renders from the
 * same `detectTokens`, highlight and lift stay consistent by construction.
 * Exported for unit tests.
 */
export function literalRanges(line0: string): Array<[number, number]> {
  const ranges: Array<[number, number]> = [];
  const collect = (re: RegExp) => {
    for (const m of line0.matchAll(re)) {
      ranges.push([m.index!, m.index! + m[0].length]);
    }
  };
  collect(/\[\[[^\]]*\]\]/g); // wiki links
  collect(/!?\[[^\]]*\]\([^)]*\)/g); // markdown links + images
  collect(/`[^`]*`/g); // inline code spans
  collect(/\bhttps?:\/\/\S+/g); // bare URLs (also covers link targets)
  return ranges;
}

/** Longest valid date phrase starting at word `i` (skips claimed ranges). */
function longestDateFrom(line0: string, words: WordSpan[], i: number, overlaps: (a: number, b: number) => boolean, today: Date): DateHit | null {
  const maxLen = Math.min(MAX_DATE_WORDS, words.length - i);
  for (let len = maxLen; len >= 1; len--) {
    const from = words[i].start;
    const to = words[i + len - 1].end;
    if (overlaps(from, to)) continue;
    const parsed = parseDateAndRecurrenceInput(line0.slice(from, to), today);
    if (parsed) return { from, to, endWord: i + len - 1, date: parsed.date, time: parsed.time, recurrence: parsed.recurrence };
  }
  return null;
}

/**
 * Detect tokens in the block's first line per the spec. Ranges are
 * doc-absolute. `today` anchors relative-date phrases ("tomorrow", "next
 * tuesday") — defaults to the real current time; callers only need to pass
 * it explicitly for deterministic tests/fixtures (mirrors iOS's
 * `InlineNLP.detect(..., today:)`).
 */
export function detectTokens(text: string, spec: DetectSpec, today: Date = new Date()): DetectedToken[] {
  const line0 = firstLine(text);
  const tokens: DetectedToken[] = [];
  // Seed the claimed set with the line's literal ranges (links / URLs / inline
  // code) so `overlaps` rejects any candidate token inside them — those spans
  // are addresses/verbatim text, not task metadata. No token is ever emitted
  // for a seeded range, so nothing here is highlighted or stripped.
  const claimed: Array<[number, number]> = literalRanges(line0);
  const overlaps = (from: number, to: number) => claimed.some(([a, b]) => from < b && to > a);
  const claim = (from: number, to: number) => claimed.push([from, to]);
  const words: WordSpan[] = [...line0.matchAll(/\S+/g)].map((w) => ({ start: w.index!, end: w.index! + w[0].length }));

  // 1. select props (e.g. Priority): each trigger is the value token.
  for (const p of spec.properties) {
    if (p.valueType !== "select" || p.triggers.length === 0) continue;
    for (const trig of p.triggers) {
      const re = new RegExp(`\\b${escapeRe(trig)}\\b`, "gi");
      let m: RegExpExecArray | null;
      while ((m = re.exec(line0)) !== null) {
        const from = m.index;
        const to = m.index + m[0].length;
        if (overlaps(from, to)) continue;
        claim(from, to);
        const lvl = p.key === "priority" ? priorityLevel(trig) : null;
        tokens.push({ from, to, key: p.key, value: trig, kind: p.key === "priority" ? "priority" : "select", level: lvl ?? undefined });
      }
    }
  }

  // 2. number props (e.g. Points): `<number> <trigger>`.
  for (const p of spec.properties) {
    if (p.valueType !== "number" || p.triggers.length === 0) continue;
    for (const trig of p.triggers) {
      const re = new RegExp(`\\b(\\d+(?:\\.\\d+)?)\\s*${escapeRe(trig)}\\b`, "gi");
      let m: RegExpExecArray | null;
      while ((m = re.exec(line0)) !== null) {
        const from = m.index;
        const to = m.index + m[0].length;
        if (overlaps(from, to)) continue;
        claim(from, to);
        tokens.push({ from, to, key: p.key, value: m[1], kind: "number" });
      }
    }
  }

  // 3. date props with triggers (e.g. Deadline `due`/`deadline`): `<trigger> <NL date>`.
  for (const p of spec.properties) {
    if (p.valueType !== "date" || p.triggers.length === 0) continue;
    for (const trig of p.triggers) {
      const re = new RegExp(`\\b${escapeRe(trig)}\\b`, "gi");
      let m: RegExpExecArray | null;
      while ((m = re.exec(line0)) !== null) {
        const trigFrom = m.index;
        const afterTrig = m.index + m[0].length;
        if (overlaps(trigFrom, afterTrig)) continue;
        const startWord = words.findIndex((w) => w.start >= afterTrig);
        if (startWord === -1) continue;
        const hit = longestDateFrom(line0, words, startWord, overlaps, today);
        if (hit && !overlaps(trigFrom, hit.to)) {
          claim(trigFrom, hit.to);
          tokens.push({ from: trigFrom, to: hit.to, key: p.key, value: dateValue(hit), kind: "date", recurrence: hit.recurrence });
        }
      }
    }
  }

  // 4. default date property: bare NL dates in still-unclaimed ranges — but
  // ONLY at line-start, trailing position, or right after a date-intent word
  // (mirrors iOS InlineNLP's locked trailing-position rule: a bare date
  // phrase mid-prose needs an intent word so "call her tomorrow about the
  // launch" doesn't lift, while "buy milk tomorrow" does).
  const defaultKey = spec.defaultDateProperty;
  const dateIntentWords = new Set(["on", "by", "at"]);
  for (const p of spec.properties) {
    if (p.valueType !== "date") continue;
    for (const trig of p.triggers) dateIntentWords.add(trig.toLowerCase());
  }
  // Trailing = nothing but whitespace and/or already-claimed spans (which
  // will themselves be stripped) follows `from` to the end of the line.
  const isTrailingFrom = (from: number): boolean => {
    let j = from;
    while (j < line0.length) {
      if (/\s/.test(line0[j])) { j++; continue; }
      const covering = claimed.find(([a, b]) => j >= a && j < b);
      if (!covering) return false;
      j = covering[1];
    }
    return true;
  };
  let i = 0;
  while (i < words.length) {
    if (overlaps(words[i].start, words[i].end)) {
      i++;
      continue;
    }
    const hit = longestDateFrom(line0, words, i, overlaps, today);
    if (hit) {
      const atLineStart = hit.from === 0;
      const prevWord = i > 0 ? line0.slice(words[i - 1].start, words[i - 1].end).toLowerCase() : null;
      const precededByIntent = prevWord !== null && dateIntentWords.has(prevWord);
      if (atLineStart || precededByIntent || isTrailingFrom(hit.to)) {
        claim(hit.from, hit.to);
        tokens.push({ from: hit.from, to: hit.to, key: defaultKey, value: dateValue(hit), kind: "date", recurrence: hit.recurrence });
      }
      i = hit.endWord + 1;
    } else {
      i++;
    }
  }

  return tokens.sort((a, b) => a.from - b.from);
}

/**
 * Detect + strip tokens from `text` (first line only). Returns the stripped text
 * + the structured props to set, or unchanged + `[]` when nothing is detected.
 * Per key, the last token wins; recurrences ride along as `recurring`. `today`
 * defaults to the real current time — see `detectTokens`.
 */
export function detectTaskTokens(text: string, spec: DetectSpec, today: Date = new Date()): DetectResult {
  const nl = text.indexOf("\n");
  const line0 = nl === -1 ? text : text.slice(0, nl);
  const rest = nl === -1 ? "" : text.slice(nl);

  const tokens = detectTokens(line0, spec, today);
  if (tokens.length === 0) return { stripped: text, props: [] };

  const byKey = new Map<string, string>();
  const recurrences: LiftedProp[] = [];
  for (const t of tokens) {
    byKey.set(t.key, t.value); // last wins
    if (t.recurrence) recurrences.push({ key: "recurring", value: t.recurrence });
  }
  const props: LiftedProp[] = [...byKey].map(([key, value]) => ({ key, value })).concat(recurrences);

  let s = line0;
  for (const t of [...tokens].sort((a, b) => b.from - a.from)) {
    s = s.slice(0, t.from) + s.slice(t.to);
  }
  s = s.replace(/ {2,}/g, " ").trim();
  return { stripped: s + rest, props };
}

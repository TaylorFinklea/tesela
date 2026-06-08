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
import { priorityLevel } from "./priority";
import { parseDateAndRecurrenceInput } from "./date-parser";

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

/** Longest valid date phrase starting at word `i` (skips claimed ranges). */
function longestDateFrom(line0: string, words: WordSpan[], i: number, overlaps: (a: number, b: number) => boolean): DateHit | null {
  const maxLen = Math.min(MAX_DATE_WORDS, words.length - i);
  for (let len = maxLen; len >= 1; len--) {
    const from = words[i].start;
    const to = words[i + len - 1].end;
    if (overlaps(from, to)) continue;
    const parsed = parseDateAndRecurrenceInput(line0.slice(from, to));
    if (parsed) return { from, to, endWord: i + len - 1, date: parsed.date, time: parsed.time, recurrence: parsed.recurrence };
  }
  return null;
}

/** Detect tokens in the block's first line per the spec. Ranges are doc-absolute. */
export function detectTokens(text: string, spec: DetectSpec): DetectedToken[] {
  const line0 = firstLine(text);
  const tokens: DetectedToken[] = [];
  const claimed: Array<[number, number]> = [];
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
        const hit = longestDateFrom(line0, words, startWord, overlaps);
        if (hit && !overlaps(trigFrom, hit.to)) {
          claim(trigFrom, hit.to);
          tokens.push({ from: trigFrom, to: hit.to, key: p.key, value: dateValue(hit), kind: "date", recurrence: hit.recurrence });
        }
      }
    }
  }

  // 4. default date property: bare NL dates in still-unclaimed ranges.
  const defaultKey = spec.defaultDateProperty;
  let i = 0;
  while (i < words.length) {
    if (overlaps(words[i].start, words[i].end)) {
      i++;
      continue;
    }
    const hit = longestDateFrom(line0, words, i, overlaps);
    if (hit) {
      claim(hit.from, hit.to);
      tokens.push({ from: hit.from, to: hit.to, key: defaultKey, value: dateValue(hit), kind: "date", recurrence: hit.recurrence });
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
 * Per key, the last token wins; recurrences ride along as `recurring`.
 */
export function detectTaskTokens(text: string, spec: DetectSpec): DetectResult {
  const nl = text.indexOf("\n");
  const line0 = nl === -1 ? text : text.slice(0, nl);
  const rest = nl === -1 ? "" : text.slice(nl);

  const tokens = detectTokens(line0, spec);
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

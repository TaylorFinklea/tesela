/**
 * Token-style query language for filtering blocks.
 *
 * Grammar:
 *   query   := token (whitespace token)*
 *   token   := key ':' op? value
 *   key     := identifier ("tag", "status", "priority", or any property name)
 *   op      := '>=' | '<=' | '>' | '<' | '!='   (optional, default '=')
 *   value   := bareword (stops at whitespace) | quoted string ("...")
 *
 * Examples:
 *   tag:Task
 *   tag:Task status:doing
 *   priority:>=3
 *   created:<2026-04-01
 *   tag:"To Read"
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

export type QueryOp = "=" | "!=" | ">" | "<" | ">=" | "<=";

export type QueryFilter = {
  key: string; // lowercased
  op: QueryOp;
  value: string;
};

export type ParsedQuery = { filters: QueryFilter[] };

const ISO_DATE_RE = /^\d{4}-\d{2}-\d{2}/;

export function parseQuery(input: string): ParsedQuery {
  const filters: QueryFilter[] = [];
  let i = 0;
  const s = input;

  while (i < s.length) {
    // Skip whitespace
    while (i < s.length && /\s/.test(s[i])) i++;
    if (i >= s.length) break;

    // Read key (alphanumeric + underscore)
    const keyStart = i;
    while (i < s.length && /[A-Za-z0-9_]/.test(s[i])) i++;
    if (i === keyStart) {
      // Unrecognized character — skip it to avoid infinite loop
      i++;
      continue;
    }
    const key = s.slice(keyStart, i).toLowerCase();

    // Expect ':'
    if (s[i] !== ":") continue;
    i++;

    // Optional op
    let op: QueryOp = "=";
    if (s.startsWith(">=", i)) { op = ">="; i += 2; }
    else if (s.startsWith("<=", i)) { op = "<="; i += 2; }
    else if (s.startsWith("!=", i)) { op = "!="; i += 2; }
    else if (s[i] === ">") { op = ">"; i++; }
    else if (s[i] === "<") { op = "<"; i++; }

    // Value: quoted or bareword
    let value: string;
    if (s[i] === '"') {
      i++;
      const valStart = i;
      while (i < s.length && s[i] !== '"') i++;
      value = s.slice(valStart, i);
      if (s[i] === '"') i++;
    } else {
      const valStart = i;
      while (i < s.length && !/\s/.test(s[i])) i++;
      value = s.slice(valStart, i);
    }
    if (value.length === 0) continue;

    filters.push({ key, op, value });
  }

  return { filters };
}

function compare(a: string, b: string): number {
  // Try number
  const an = Number(a);
  const bn = Number(b);
  if (!Number.isNaN(an) && !Number.isNaN(bn) && a.trim() !== "" && b.trim() !== "") {
    return an - bn;
  }
  // Try ISO date
  if (ISO_DATE_RE.test(a) && ISO_DATE_RE.test(b)) {
    const ad = Date.parse(a);
    const bd = Date.parse(b);
    if (!Number.isNaN(ad) && !Number.isNaN(bd)) return ad - bd;
  }
  // String compare (case-insensitive)
  return a.toLowerCase().localeCompare(b.toLowerCase());
}

function applyOp(actual: string, op: QueryOp, expected: string): boolean {
  if (op === "=") return actual.toLowerCase() === expected.toLowerCase();
  if (op === "!=") return actual.toLowerCase() !== expected.toLowerCase();
  const cmp = compare(actual, expected);
  if (op === ">") return cmp > 0;
  if (op === "<") return cmp < 0;
  if (op === ">=") return cmp >= 0;
  if (op === "<=") return cmp <= 0;
  return false;
}

export function blockMatches(block: ParsedBlock, query: ParsedQuery): boolean {
  for (const f of query.filters) {
    if (!filterMatches(block, f)) return false;
  }
  return true;
}

function filterMatches(block: ParsedBlock, f: QueryFilter): boolean {
  if (f.key === "tag") {
    const lower = f.value.toLowerCase();
    const allTags = [...block.tags, ...block.inherited_tags].map((t) => t.toLowerCase());
    if (f.op === "=") return allTags.includes(lower);
    if (f.op === "!=") return !allTags.includes(lower);
    return false; // comparison ops not meaningful for tags
  }
  // Property lookup (case-insensitive key)
  const propEntry = Object.entries(block.properties).find(
    ([k]) => k.toLowerCase() === f.key,
  );
  const actual = propEntry ? propEntry[1] : "";
  if (!propEntry && f.op === "!=") return true; // missing property != value matches
  if (!propEntry) return false;
  return applyOp(actual, f.op, f.value);
}

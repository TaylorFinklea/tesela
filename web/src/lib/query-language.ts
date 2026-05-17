/**
 * Token-style query language for filtering blocks and pages.
 *
 * Mirrors `crates/tesela-core/src/query.rs` so the same DSL parses identically
 * server-side (for execution) and client-side (for previews / stubs).
 *
 * Grammar:
 *   query    := token (whitespace token)*
 *   token    := negation? key ':' op? value
 *   negation := '-'
 *   key      := identifier ("kind", "tag", "status", "has", or any property name)
 *   op       := '>=' | '<=' | '>' | '<' | '!='   (optional, default '=')
 *   value    := bareword (stops at whitespace) | quoted string ("...")
 *
 * Special pseudo-keys:
 *   - `kind:block | kind:page` — narrows the result set; consumed into
 *     ParsedQuery.kind (default `block`), NOT a filter on individual blocks.
 *   - `has:foo` — block has property `foo` regardless of value. `-has:foo`
 *     for absence.
 *   - `tag:foo` — block's resolved tag chain (direct + inherited) includes foo.
 *
 * Examples:
 *   kind:block tag:Task -status:done
 *   kind:page note_type:Project
 *   tag:Task priority:>=3 deadline:<=2026-05-01
 *   has:deadline -has:status
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

export type QueryOp = "=" | "!=" | ">" | "<" | ">=" | "<=";

export type Kind = "block" | "page";

export type QueryFilter = {
  key: string; // lowercased
  op: QueryOp;
  value: string;
};

export type ParsedQuery = { kind: Kind; filters: QueryFilter[] };

const ISO_DATE_RE = /^\d{4}-\d{2}-\d{2}/;

function invertOp(op: QueryOp): QueryOp {
  switch (op) {
    case "=": return "!=";
    case "!=": return "=";
    case ">": return "<=";
    case "<": return ">=";
    case ">=": return "<";
    case "<=": return ">";
  }
}

export function parseQuery(input: string): ParsedQuery {
  const filters: QueryFilter[] = [];
  let kind: Kind = "block";
  let i = 0;
  const s = input;

  while (i < s.length) {
    // Skip whitespace
    while (i < s.length && /\s/.test(s[i])) i++;
    if (i >= s.length) break;

    // Optional negation prefix
    let negated = false;
    if (s[i] === "-") {
      negated = true;
      i++;
    }

    // Read key (alphanumeric + underscore + hyphen, so `has-link` parses as one key)
    const keyStart = i;
    while (i < s.length && /[A-Za-z0-9_-]/.test(s[i])) i++;
    if (i === keyStart) {
      // No key after '-' or unrecognized character — skip one byte to avoid loop
      i++;
      continue;
    }
    const key = s.slice(keyStart, i).toLowerCase();

    // Expect ':'. If missing, drop the token entirely.
    if (s[i] !== ":") continue;
    i++;

    // Optional op
    let opRaw: QueryOp = "=";
    if (s.startsWith(">=", i)) { opRaw = ">="; i += 2; }
    else if (s.startsWith("<=", i)) { opRaw = "<="; i += 2; }
    else if (s.startsWith("!=", i)) { opRaw = "!="; i += 2; }
    else if (s[i] === ">") { opRaw = ">"; i++; }
    else if (s[i] === "<") { opRaw = "<"; i++; }

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
    // `has:foo` may have an empty value when written as `-has:foo`. Other
    // empty-valued tokens are dropped (matches the Rust parser).
    if (key !== "has" && value.length === 0) continue;

    const op = negated ? invertOp(opRaw) : opRaw;

    if (key === "kind") {
      const v = value.toLowerCase();
      kind = v === "page" || v === "pages" ? "page" : "block";
      continue;
    }

    filters.push({ key, op, value });
  }

  return { kind, filters };
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
  // Tag-system Phase 16 — `tag:` (default), `pagetag:` (frontmatter alias),
  // `blocktag:` (excludes inherited). Mirrors Rust crates/tesela-core/src/query.rs.
  if (f.key === "tag" || f.key === "pagetag" || f.key === "blocktag") {
    const lower = f.value.toLowerCase();
    const includeInherited = f.key !== "blocktag";
    const allTags = includeInherited
      ? [...block.tags, ...block.inherited_tags].map((t) => t.toLowerCase())
      : block.tags.map((t) => t.toLowerCase());
    if (f.op === "=") return allTags.includes(lower);
    if (f.op === "!=") return !allTags.includes(lower);
    return false; // comparison ops not meaningful for tags
  }
  if (f.key === "has-link") {
    const needle = `[[${f.value.toLowerCase()}]]`;
    const present = block.raw_text.toLowerCase().includes(needle);
    if (f.op === "=") return present;
    if (f.op === "!=") return !present;
    return false;
  }
  if (f.key === "has") {
    const needle = f.value.toLowerCase();
    const present = Object.keys(block.properties).some(
      (k) => k.toLowerCase() === needle,
    );
    if (f.op === "=") return present;
    if (f.op === "!=") return !present;
    return false;
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

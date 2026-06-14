# L5 — Query DSL typed comparisons (spec)

**Status:** in progress (2026-06-14). Scope = L. Source of truth for parity = the shared conformance fixture.

## Goal

Comparisons in the property query DSL are currently a **type-guessing heuristic on raw strings** with no registry involvement. `compare(a,b)` tries `f64::parse` on both operands → numeric; else ISO-date shape → lexicographic; else case-folded string. It's numeric only *by luck of the string*. L5 plumbs the **PropertyRegistry's declared `value_type`** into comparison so a `number`-typed property compares numerically, a `date` property by ISO order, a `checkbox` by bool — authoritatively, not by guessing. Done identically in **Rust** (`query.rs`) and **TS** (`query-language.ts`), enforced by the shared fixture.

## The gap (concrete)

- `priority:: 10` vs query `priority > 5`: heuristic gets it right (both parse f64). But `priority:: 10` vs `priority:: 2` under a **select** type, or any value the string can't round-trip, sorts lexicographically. A `select` storing `"10"` must NOT sort numerically; a `number` storing `"10"` MUST. Only the registry knows which.
- No authority: `compare("10","5")` is numeric by accident. L5 makes the declared type authoritative.

## Locked decisions

1. **Additive, registry-free path preserved byte-for-byte.** Keep `compare(a,b)` / `apply_op` exactly as-is (the 100+ existing conformance cases, the in-memory sqlite refine path, and any registry-less caller keep current heuristic behavior). Add `compare_typed(a,b,vt)` / `apply_op_typed(...)` and a `*_typed` matcher entry that takes a `key→ValueType` map. Empty/absent map ⇒ current behavior.
2. **Branch on SEMANTIC categories, not the raw `value_type` string.** Rust `ValueType` spells it `multiselect`; web `PropertyType` spells it `multi-select` and adds `email/phone/object`; web lacks `datetime/node`. The comparison only cares about four buckets — **numeric** (Number) · **date-like** (Date, DateTime) · **boolean** (Checkbox) · **string** (everything else: Text/Select/MultiSelect/Url/Node/email/phone/object). Map both vocabularies into these buckets in one explicit, commented place per language so Rust↔TS parity is auditable.
3. **Coercion table (identical both languages):**
   - **numeric** → parse both operands as f64; both parse ⇒ numeric compare; either fails ⇒ fall back to case-folded string compare (never silently treat a non-number as 0).
   - **date-like** → extract ISO date (existing `extract_iso_date` / `isIsoDate`); ISO dates sort correctly lexicographically, so compare as strings. This preserves today's date behavior but makes it *authoritative* (a date-typed prop always uses it).
   - **boolean** → coerce each via `eq_ignore_ascii_case("true")` → bool; order `false < true`.
   - **string** → existing case-folded string compare. **No heuristic numeric promotion** — a select storing `"10"` compares lexicographically.
4. **Eq/Ne also coerce for numeric + boolean typed props.** So `count = 3` matches `count:: 3.0`, and `done = true` matches `done:: True`. Define typed Eq/Ne as `compare_typed(...) == Equal`. For date-like and string buckets, Eq/Ne stay as today (ISO string equality / case-fold) — `compare_typed == Equal` yields the same result there anyway. This keeps `count >= 3` and `count = 3` consistent for `"3.0"`.
5. **Threading:** build a `HashMap<String,ValueType>` / `Map<string,PropertyType>` keyed by **lowercased** property name, ONCE at the eval entry point; pass `&map` down `block_matches_typed → eval_expr → pred_matches → filter_matches`. Keep `block_matches(block,query)` as a wrapper passing an empty map (zero behavior change). Mirror in TS: `blockMatches(block, query, types?)`.
6. **Coercion is single-sourced with storage.** Rust numeric/bool coercion mirrors `property.rs::parse_scalar`/`PropScalar` so query semantics == storage semantics.
7. **Web registry source = `buildRegistry(notes)`** (client-side, built from Property-page frontmatter — matches the chip/editor surfaces). The server query path (if any) uses `get_all_property_defs`; document that the two must stay consistent (not reconciled in L5).
8. **iOS** (`LocalQueryEngine.compareValues`) is a third fixture consumer. Decision: extend it to honor an optional `propertyTypes` map IF the change is small; otherwise gate the new typed cases behind a fixture marker the Swift runner skips and file iOS as an L5 fast-follow. **Do not leave the iOS conformance test red.**

## Fixture contract (the parity enforcer)

- `crates/tesela-core/tests/fixtures/query-conformance.json` is read by **all three** runners. `FixtureBlock`/`Case` use `deny_unknown_fields` → any new field requires updating the Rust struct **and** the TS runner (and the iOS runner) together, or every conformance test breaks.
- Add an **optional** per-case `propertyTypes: { "<lowercased name>": "number"|"date"|"datetime"|"checkbox"|"text"|"select"|... }`. Cases without it exercise the registry-free path (all existing cases unchanged).
- **New cases proving typed beats lexicographic:**
  - `number` `priority="10"`, dsl `priority > 5` ⇒ **true** (string `"10" > "5"` is false).
  - `number` `priority="2"`, dsl `priority > 5` ⇒ false.
  - `number` `priority="10"`, dsl `priority < 5` ⇒ false; `priority="2"` `priority < 5` ⇒ true (string compare would match both).
  - `checkbox` `done="true"`, dsl `done = true` ⇒ true; `done = false` ⇒ false.
  - `number` `count="3.0"`, dsl `count = 3` ⇒ true (Eq coercion).
  - `date` `due="2026-06-01"` and `"2026-08-01"`, dsl `due < 2026-07-01` ⇒ only June.

## Acceptance / Verify

- `cargo test -p tesela-core query` (unit) + `cargo test -p tesela-core --test query_conformance` (shared fixture).
- `cd web && npm run test:unit` (mirror runner reads the same fixture ⇒ parity by construction).
- iOS conformance test green (updated or typed-cases-gated).
- Product-testable: a Query block with a number-typed `priority`, blocks `priority:: 10` / `priority:: 2`, query `tag:task priority > 5` returns **only** the 10.

## Risks (from scout)

- Default `priority` ships as a **select** (critical/high/medium/low) — the demo/test MUST define a `number`-typed property or it proves nothing.
- Two web registries (server `PropertyDef` vs client `buildRegistry`) can disagree — use client for query eval, document the consistency requirement.
- `deny_unknown_fields` — update all runners together.
- Eq/Ne coercion decision (locked #4) must be mirrored in both languages + fixture.

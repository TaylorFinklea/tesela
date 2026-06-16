# JQL iOS parity (query P2) — spec

**Goal.** Close the iOS query-engine gap so JQL queries (`OR`, parens, `IN (…)`,
`NOT IN`, `LIKE`/`NOT LIKE`, `BETWEEN`, `IS NULL`/`IS NOT NULL`, full infix ops)
filter the SAME blocks on iPhone as on web/server. Today iOS's `LocalQueryEngine`
deliberately flattens to AND-only and **drops** those constructs (degrades toward
match-all). Rust + web already parse the full grammar.

**The gate = the shared conformance fixture.** `crates/tesela-core/tests/fixtures/query-conformance.json`
has 113 cases, ALL colon-DSL, ZERO JQL. We add JQL cases (the cross-engine
contract), validate them against Rust (source of truth) + web, which makes the
iOS conformance test (`app/Tesela-iOS/Tests/QueryConformanceTests.swift`, runs
every case, zero skips) go RED. The implementation makes it GREEN.

## Phase A — author + validate the JQL conformance contract (Opus)

Add ~28 JQL cases covering: `OR`; AND-over-OR precedence; parens (incl. `NOT (…)`);
`IN (…)` / `NOT IN (…)` (whitespace-tolerant, exact-value); `LIKE`/`NOT LIKE`
(`%`, `_`, case-insensitive, on `text:` and on a property); `BETWEEN x AND y`
(desugars to `>=x AND <=y`); `IS NULL`/`IS NOT NULL` (= `NOT has` / `has`);
infix `= != > < >= <=`; a couple typed (`propertyTypes`) numeric/date cases.

- Expected values come from RUST. Author best-guess `expect`, append to the
  fixture, run `cargo test -p tesela-core --test query_conformance` — Rust's
  matcher computes actual; fix any `expect` mismatch to match Rust.
- Then `cd web && npm run test:unit` — web mirrors Rust; the JQL cases must
  pass there too (confirms web parity, no web change expected).
- Confirm iOS goes RED on the new cases (the implementer's target).

## Phase B — restructure iOS `LocalQueryEngine` (mirror Rust)

Restructure the flat AST to Rust's boolean tree. Read the patterns to mirror;
do NOT invent grammar:

- **AST** — replace `SimpleDsl.clauses: [Clause]` with `expr: BoolExpr`, mirroring
  Rust `crates/tesela-core/src/query.rs:141` `BoolExpr {And/Or/Not/Atom}` +
  `:114` `Predicate {Cmp/In}` + `:56` `QueryOp` (ADD `.like`, `.notLike`).
  Keep `kind` + `sort`. (The legacy `filters` flat view is Rust-only for SQL
  prefilter; iOS doesn't need it.)
- **Parser** — replace `parseClauses` (flat AND) with the recursive-descent
  `parse_or → parse_and → parse_unary → parse_predicate` chain, mirroring
  `query.rs` (and its faithful web port `web/src/lib/query-language.ts`
  `parseOr/parseAnd/parseUnary/parsePredicate`) EXACTLY: real `OR`, real
  parenthesized grouping, `key IN (…)` / `key NOT IN (…)` paren list, `key LIKE
  v` / `key NOT LIKE v`, `key BETWEEN x AND y` → `And[Cmp Gte, Cmp Lte]`,
  `key IS [NOT] NULL` → `has` / `Not has`. Keep the existing legacy-colon /
  tight-comma / `tag-in:` / `kind:` / infix handling that already passes.
- **Matcher** — replace `blockMatches = clauses.allSatisfy` with a recursive
  `evalExpr(BoolExpr)` (and = all, or = any, not = !, atom = predMatches);
  `predMatches` routes `Cmp` to `cmpMatches`, `In` to OR-over-Eq (negated flips).
  Add `Like`/`NotLike` to `applyOp`/`applyOpTyped` + the `text`/property paths
  (mirror Rust `like_matches` / `query-language.ts likeMatches`: `%`→`.*`,
  `_`→`.`, regex metachars escaped, case-insensitive).
- **Typed ORDER BY (secondary)** — optional in this phase: port `applySort`
  (web/src/lib/query-language.ts) to sort `queryItems` by `sort`, L5-typed via
  `compareValuesTyped`. Not gated by the fixture (filter-only); include if clean.

**Verify:** `xcodebuild test … -only-testing:TeselaTests/QueryConformanceTests
-only-testing:TeselaTests/LocalQueryEngineTests` — all green (the new JQL cases
+ no regression on the 113 existing). Then the full suite.

## Phase C — adversarial review + ship (Opus)

Adversarial review (does the iOS tree match Rust's precedence/associativity?
edge cases: empty `IN ()`, unterminated paren, `NOT IN` on missing property,
`LIKE` underscore, BETWEEN typed). Commit. Cut iOS build N+1 only with Taylor's
OK. Product test: JQL on iPhone matches web.

## Non-goals / parity notes

- `kind:`/`type` aliasing, missing-property-Ne, the degrade-toward-match-all
  posture for genuinely malformed input — unchanged (mirror Rust).
- Page-kind queries stay out of fixture scope (block-kind only).
- iOS sort parity with the SERVER's string-only `apply_sort` is NOT the goal —
  the typed local sort (matching the web inline block) is the better behavior.

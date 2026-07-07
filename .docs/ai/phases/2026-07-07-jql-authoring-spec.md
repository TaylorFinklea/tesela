# JQL first-class authoring (tesela-vp9) — Lead decomposition spec

2026-07-07 (Fable). Grounded in a 3-reader map (web surfaces / parser tooling /
iOS surfaces). Engines are DONE (182 shared conformance cases, one unified
grammar; colon-DSL is legacy sugar inside it). This bead is authoring UX only.

## Product locks (binding)
- JQL is THE documented/default idiom everywhere a query is authored (Taylor
  lock 2026-06-15). Colon-DSL keeps parsing (backcompat) but chips, placeholders,
  examples, and docs all write JQL.
- Parse RESULTS never change: both parsers are deliberately total/infallible and
  conformance-gated. All diagnostics are ADDITIVE, non-authoritative UI metadata.

## Design decisions (locked here)
1. **One shared authoring widget per platform.** Web: a single `QueryInput`
   component replaces the three divergent surfaces (GrInbox saved-view `<input>`,
   QueryBlock inline `<input>`, RawDslSheet `<textarea>`). Completion UI mirrors
   the repo's hand-rolled AutocompleteMenu pattern (NOT @codemirror/autocomplete —
   not installed, not the repo idiom for popups); highlighting = colored overlay
   behind a transparent-text input (no CM instance for query fields).
2. **Export the real tokenizers.** Web: export `tokenize` + span/token types from
   query-language.ts (the SAME function the parser uses — zero drift). iOS: make
   `tokenizeDsl`/`SpannedDslToken` internal. Rust tokenizer stays private (no
   Rust-side tooling in scope).
3. **Diagnostics = dropped-span recording.** The parsers silently drop
   unrecognized tokens and re-sync; instrument that drop path (or a parallel
   authoring-only pass) to record `{span, got, hint}` without changing behavior.
   Diagnostics are unit-tested per platform; the shared 182-case conformance
   fixture is NOT extended for them (it stays matching-semantics only).
4. **Completion tiers:** keys (property names + meta keys) → operators
   (`= != < <= > >= IN NOT IN LIKE BETWEEN IS NULL AND OR ORDER BY ASC DESC`) →
   VALUES (select-property choices via the property registry; type names for
   `type =` / `kind:`). Sources: web `api.listProperties()` (`.values` is unused
   today!) + `listTypes`; iOS `PropertyRegistry.properties` + `typeNames()`.
5. **Chips write JQL.** Migrate web CHIP_REGISTRY + iOS InboxChips fragments to
   JQL predicate strings; replace whitespace-token toggling with parse-aware
   toggling (clause present in the AST → remove via token spans; absent →
   append space-separated, which the grammar reads as implicit AND).
6. **iOS highlighting v1:** ride the existing `InlineNLPHighlighter` PAINTER with
   a new JQL token detector (kinds: key/operator/value/string/number/paren).
   In-editor: `query::` lines in the UITextView-backed block editor. The
   saved-view editor keeps its SwiftUI TextField (no representable migration in
   v1) and gets a highlighted token PREVIEW row + diagnostics line instead.

## Child beads
- **vp9.1 web foundation (M, senior):** export tokenizer + diagnostics pass in
  query-language.ts; unit tests. No UI.
- **vp9.2 web QueryInput (L, senior, dep vp9.1):** shared component (overlay
  highlight + completion popup + diagnostics); adopt in GrInbox editor,
  QueryBlock, RawDslSheet — one widget, three mounts, delete the divergent logic.
- **vp9.3 web chips→JQL (M, senior, dep vp9.1):** CHIP_REGISTRY to JQL +
  parse-aware toggle; JQL examples in the /query palette command description.
- **vp9.4 iOS foundation (M, senior):** tokenizer internal + mirrored diagnostics
  pass in LocalQueryEngine; unit tests.
- **vp9.5 iOS editor UX (L, senior, dep vp9.4):** GrViewEditorSheet completion
  (keys/operators/values) + token-preview row + real diagnostics; InboxChips →
  JQL + parse-aware toggleFragment.
- **vp9.6 iOS query:: highlighting (M, senior, dep vp9.4):** JQL detector +
  painter wiring for `query::` lines in the block editor.
- **vp9.7 conformance audit (S, junior):** audit JQL-syntax coverage of the 182
  fixture for forms the completion emits (IN lists, BETWEEN, ORDER BY multi-key,
  quoted values); add matching-semantics cases where thin (all three engines).

Sequencing: vp9.1 ∥ vp9.4 first; then vp9.2/vp9.3 ∥ vp9.5/vp9.6; vp9.7 anytime.
Machine note: iOS lanes dispatch after ya4.5 lands (xcodebuild contention).

## Out of scope (explicit)
CM6 language mode; LSP; Rust-side tooling; NL→JQL; changing parse semantics or
the conformance schema; structured no-code query builder UI.

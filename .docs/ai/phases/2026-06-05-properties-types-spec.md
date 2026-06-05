# Properties + Types System — Milestone Spec

Authored 2026-06-05 (Opus, brainstormed w/ Taylor). Substantial multi-session work → `phases/` spec per AGENTS.md. Roadmap item: step 3(b) "properties + types system" (`project_property_system_vision`). Supersedes the "v1 = scalar strings, multi-value deferred" line in `project_structured_first_crdt_truth` (multi-value now ships here).

Grounding map (verified): workflow `properties-current-state-map` (9 readers + synth, 2026-06-05). Treat any map line numbers as approximate; cite by file+symbol.

---

## Decisions (locked 2026-06-05)

1. **Scope** — full Logseq-DB / AnyType property+type system, designed end-to-end, shipped in phases with daily-driver value at each step.
2. **Editing model = Hybrid** — properties are real CRDT data; edited as `key:: value` text *or* via chips / `/p`. The text line is a *view* over the container (same pattern as block text already being a view over its `text_seq` LoroText). No "raw text" escape hatch.
3. **New-entity confirmation guard** — when a commit would create an entity that doesn't already exist (new property / tag / type / page / out-of-`choices` value), intercept with a confirm; when a near-match exists, lead with "Did you mean **X**? · [use existing] / [create new]". Globally toggleable (default on), possibly per-category. Solves the daily pain: typo'd Enter / missing the autocomplete pick mints junk entities.
4. **Config UI = all three surfaces** over one shared registry foundation: entity page (canonical) · inline drawer gear · ⌘K modal.
5. **Hard scope = multi-value AND node-references both ship** this milestone.
6. **Phase order = foundation-first** (Option A). Closes the multi-device data-loss early, puts every value-write on the right substrate (no rework), config UI lands as phase 2.

---

## End-state design

### Data model — typed property containers

Each property becomes a typed container on its owner (a block node, or the page), addressed by lowercased key. Topology (→ confirmed by the architectural-review gate before the Phase-1 merge):

- **`props`** — a `LoroMap` on the owner. Value encoding by `value_type`:
  - **single scalar** (select / number / date / datetime / checkbox / url) → a primitive `LoroValue` under the key. Concurrent set = LWW (acceptable for an atomic scalar).
  - **single free-text** (`text`) → a nested `LoroText` under the key (concurrent char-merge).
  - **multi-value** (multi-select / tags / multiple node-refs) → a nested `LoroList` under the key. Concurrent add/remove **union-merges**; dedup at materialize. *This is what fixes the cross-device tag-merge clobber.*
  - **node-reference** → the value is the target page's stable id (slug→uuid via the existing `stable_uuid_from_slug`); single = primitive id, multi = `LoroList` of ids. A backlink index materializes the reverse direction (Phase 6).
- **`prop_keys`** — a sibling `LoroList` of keys in display order. **Mandatory:** Loro `LoroMap` iteration order is NOT guaranteed; the materializer walks `prop_keys` so the markdown view is deterministic across replicas.

Page-level properties use the same `props` + `prop_keys` shape at the doc root, replacing today's interleaved `[k,v,k,v]` `page_props` list that is wholesale clear-and-repushed on every `NoteUpsert` (`engine/loro_engine.rs`).

### Wire / op model

Property mutations must NOT ride inside block prose text (today's bug: `BlockUpsert.text` carries `key:: value` lines, and `BlockUpsert` (`oplog/op.rs`) has no properties field, so a prose edit / stale-buffer whole-text update can drop a peer's property line). Recommended: dedicated property ops — `BlockPropertySet { block_id, note_id, key, op: Set|AddToList|RemoveFromList|Clear, value }` and a page-property analog — so prose and properties are independent on the wire and merge separately. `BlockUpsert.text` becomes prose-only. The existing `POST /blocks/set-property` route already isolates the write site; it emits the new op instead of splicing text.

> Alternative (review will choose): a `#[serde(default)] properties` field on `BlockUpsert`. The dedicated-op route is preferred for wire clarity + independent merge. **Either way:** `#[serde(default)]` / additive-only decode, and the install-new-build-before-cross-device-edits migration discipline (same hazard as the block-text→LoroText change, memory `project_block_text_crdt`).

### The Hybrid seam (the trickiest UI piece)

- **Outbound (type → container):** the block editor detects a committed `key:: value` continuation line, strips it from the prose, resolves the key against the registry, and emits a property op. Mirror the existing splice seam (`splice_block_text` / the cm-decorations property-line hiding in `web/src/lib/cm-decorations.ts`) — do not re-author whole text.
- **Inbound (container → view):** a property container change re-renders the block's `key:: value` line(s) + chips without a full refetch. Mirror the existing inbound live-apply (`active-note-doc` / `NoteDoc.subscribe`).
- Chips (`DisplayChip.svelte`) and the `/p` chord write the SAME container. Both paths converge on one property op.

### Registry as data

- `value_type` becomes a real Rust enum (today a bare `String` in the type model) + a `Property` struct mirroring the existing `TypeDefinition`. Validation at index time (`db/sqlite.rs` index path): unknown value_type → error/warn; select value ∉ `choices` → the guard (Phase 2) / warn policy (decide explicit error-vs-coerce in Phase 3).
- Tag / Property pages stay the **source of truth** (markdown frontmatter); SQLite `tag_defs` / `property_defs` / `block_properties` stay rebuilt caches. The config UI is CRUD over page frontmatter (mirror `property-registry.ts` `updateFrontmatterKey`), never direct cache writes.
- Adding a property to a tag = search the registry (existing Property pages) first → the guard's "use existing / create new" surface.
- Carry `extends` onto the exported `TypeDefinition` (today flatten-only at read via `get_resolved_tag_def`, `db/sqlite.rs`) so the hierarchy round-trips and is editable.

### Views over data

Type/tag pages render the type's nodes as table (`TagTable.svelte`) / kanban (`KanbanBoard.svelte`) / list, columns = resolved `tag_properties`, typed cells. Saved queries (**Sets**) become a persisted object (new — none today; only ephemeral localStorage + `note_type: Query` pages). `QueryWidgetView.svelte` gains a table mode (kanban/list only today).

### Markdown materialization

A deterministic, Logseq-compatible materialized **view**: same CRDT state → same bytes (NOT byte-preserving of hand-edits). Properties serialize from `props` in `prop_keys` order — block properties as `key:: value` continuation lines, page properties as frontmatter / body-top per existing convention (`note_tree.rs` `split_page_properties`, `markdown.rs`).

---

## Hard constraints / landmines

1. **Loro map order is not guaranteed** → `prop_keys` ordered list is mandatory for deterministic materialization.
2. **Wire-compat** → additive-only op change, `#[serde(default)]`, install-new-build-before-cross-device-edits. iOS FFI (`tesela-sync-ffi`) rebuild + bindings regen + `xcodebuild` when the op/engine surface changes; copy the new header into `app/Tesela-iOS/CFFI/` (not just `Generated/`).
3. **CRDT is truth; SQLite is a cache** → never write `tag_defs`/`property_defs`/`block_properties` directly; mutate the note → parse → index.
4. **Backward-compat** → existing `key:: value` notes parse/render losslessly; lazy migrate-on-write (mirror `read_block_text`'s legacy-`text`-register fallback).
5. **Silent-failure policy** (decide explicitly, don't inherit): dangling `extends` silently drops parent props; missing Property page → text fallback; invalid select value accepted; malformed `choices_json` round-trips silently. Phase 2/3 must choose surface-vs-coerce.
6. **Case rules** (don't change casually): lookups case-insensitive; storage case-preserving; block keys lowercased on write.

---

## Verified current-state anchors (corrected paths)

- `crates/tesela-sync/src/oplog/op.rs` — `OpPayload` (`BlockUpsert` = block_id, note_id, parent_block_id, order_key, indent_level, **text**, positional-insert hint; **no properties**). [VERIFIED]
- `crates/tesela-sync/src/engine/loro_engine.rs` — Loro engine: `page_props` handling, materialize/`note_tree_from_doc`, `read_block_text`/`write_block_text`, `apply_payload`. [path VERIFIED; confirm symbols on edit]
- `crates/tesela-core/src/db/schema.rs` — `tag_defs`, `property_defs(value_type, choices_json, default_value, multiple_values, hide_empty, …)`, `block_properties(block_id, note_id, property_name, value)` + indexes. [VERIFIED]
- `crates/tesela-core/src/db/sqlite.rs` — `index_block_properties`, `index_type_info`, `get_resolved_tag_def` (extends walk, max 10), `execute_block_query`. [map]
- `crates/tesela-core/src/block.rs` — `parse_blocks`, `extract_properties`, `PROPERTY_RE`. [map]
- `crates/tesela-core/src/note_tree.rs` — `split_page_properties`. [map]
- `crates/tesela-core/src/query.rs` — DSL (`tag:/type:/has:/has-link:/IN/BETWEEN/LIKE/NOT`), `filter_matches`, `compare`. [map]
- `crates/tesela-server/src/routes/notes.rs` — `POST /blocks/set-property`, `upsert_block_property_in_note`; `GET /types/{name}`. [map]
- Web — `BlockEditor.svelte`, `BlockOutliner.svelte` (`displayChipsFor`), `BottomDrawer.svelte`, `PropertiesView.svelte` (READ-ONLY today), `DisplayChip.svelte`, `TagTable.svelte`, `KanbanBoard.svelte`, `QueryWidgetView.svelte`, `cm-decorations.ts`; `lib/triage.svelte.ts`, `block-tags.ts`, `property-registry.ts`, `property-update.ts`, `api-client.ts`, `query-language.ts`. [map]

---

## Phase plan (foundation-first)

Each phase = one or more commits, self-QA'd, with the listed Verify green before moving on. "Touches" names patterns/files to read+mirror — implementers READ them, don't trust prescribed code.

### Phase 1 — Foundation: typed property values in the CRDT + registry-as-data  · L · ⚠ data-loss fix
- **Scope:** `props`/`prop_keys` containers on block node + page root; value encodings (scalar/text/list/node) per the data model; dedicated property op(s) (or `BlockUpsert.properties`, per review); block properties leave `text`; page props leave interleaved `page_props`; `value_type` enum + `Property` struct; lazy migrate-on-write from legacy `key:: value`-in-text + `page_props`; deterministic materializer walks `prop_keys`.
- **Touches:** `oplog/op.rs`, `engine/loro_engine.rs` (mirror the `text_seq` LoroText seam + `read/write_block_text` legacy fallback), `tesela-sync-ffi` (+ bindings regen + iOS header copy), `db/schema.rs`/`db/sqlite.rs` (index from containers), `block.rs`/`note_tree.rs` (serialize from containers).
- **Acceptance:** concurrent prose edit + property edit on one block → both survive (no clobber); concurrent multi-value add on two devices → union (no LWW loss); materialized markdown byte-stable across replicas given equal CRDT state; legacy notes round-trip losslessly.
- **Verify:** `cargo test -p tesela-sync` (+ new convergence tests: prose-vs-prop, multi-value union, determinism), `cargo test -p tesela-server`, `cargo build -p tesela-sync-ffi`, `cargo test --workspace`.
- **REVIEW GATE:** adversarial architectural review of container topology + op shape + wire-compat + determinism BEFORE merge (as the Loro cutover + block-text CRDT changes had).

### Phase 2 — Config UI + global registry + new-entity guard  · M · (biggest visible win)
- **Scope:** registry CRUD endpoints (`POST/PUT/DELETE /properties`, `/types`); one reusable property/tag config component mounted 3 ways (entity page = canonical editor + the type's live table; inline drawer gear; ⌘K modal); edit value_type/choices/default/multiple/hide + extends/tag_properties + display chips; **new-entity confirmation guard** w/ did-you-mean near-match, on new property/tag/page/out-of-choices value, globally toggleable (+ per-category); make `PropertiesView` editable (it's read-only today).
- **Touches:** new server routes (mirror existing note/type route idioms in `routes/notes.rs`); web config component + the three mounts; `property-registry.ts` `updateFrontmatterKey`; autocomplete/commit sites in `BlockEditor.svelte` + wiki-link/tag pickers for the guard; a settings toggle.
- **Acceptance:** create+configure a Property and attach it to a Tag entirely via UI (zero YAML); typo'd new `#tag`/`[[page]]`/property triggers the guard with a working "use existing" near-match; toggle off → no prompts.
- **Verify:** `pnpm --dir web check`, `pnpm --dir web build`, a new `web/tests/config-ui.e2e.mjs` (create property → attach to tag → guard intercept → use-existing), `cargo test -p tesela-server` (registry routes).

### Phase 3 — Defaults auto-fill + validation enforcement  · S–M
- **Scope:** tagging `#Tag` pre-fills its resolved properties from each `default`; enforce value_type/choices at index/write per the chosen surface-vs-coerce policy.
- **Touches:** on-tag handler (web `block-tags.ts` / server write path); `db/sqlite.rs` index validation; reuse `get_resolved_tag_def` defaults.
- **Acceptance:** tagging a block `#Task` populates Status/Priority/Deadline/Scheduled blanks (or defaults); out-of-choices value is surfaced, not silently stored.
- **Verify:** `cargo test -p tesela-core` (resolve+default+validate), web e2e (tag → chips appear).

### Phase 4 — Multi-value + typed query DSL  · M
- **Scope:** multi-select / tags-as-list add-remove UI over `LoroList` props; registry-aware query matcher — typed comparisons (number/date real, not f64-guess), select-order semantics, `status:invalid` rejected, custom properties inherited down the tag chain (only tags inherit today).
- **Touches:** `query.rs` `filter_matches`/`compare` (thread a `PropertyRegistry`), `db/sqlite.rs` execute paths, `query-language.ts` (TS mirror is a stub — bring to parity), web multi-value editor.
- **Acceptance:** `priority >= 3` typed-correct; multi-select add/remove merges across devices; a custom property in a query inherits via `extends`.
- **Verify:** `cargo test -p tesela-core` (query suite), `cargo test -p tesela-sync` (multi-value convergence), web e2e.

### Phase 5 — Type-driven views: saved Sets + table mode + column config  · M
- **Scope:** persisted Sets object (define one); table mode in `QueryWidgetView`; column hide/reorder + sort persistence; group-by picker for non-kanban; reconcile with `TagTable`/`KanbanBoard`.
- **Touches:** new Set persistence (server + a `note_type: Query`-adjacent model, decide), `QueryWidgetView.svelte`, `TagTable.svelte`, tag-view-prefs.
- **Acceptance:** save a filtered Set, reopen it, render as table/kanban/list with persisted columns+sort.
- **Verify:** `pnpm --dir web check` + e2e (save Set → reload → same view), server tests for Set CRUD.

### Phase 6 — Node-references (bidirectional)  · L
- **Scope:** backlink index (reverse links materialized — new SQLite table); link-picker value editor for `node` properties; linked-property columns/queries (`assignee:: [[Alice]]` queryable as structured data, not substring).
- **Touches:** `db/schema.rs` (backlink table) + index path, `query.rs` (node predicate beyond `has-link:` substring), web link picker + linked columns, `engine` (node-ref list container from Phase 1).
- **Acceptance:** set `assignee:: [[Alice]]` → appears in Alice's backlinks; query "tasks assigned to Alice" returns structured matches; renaming a target surfaces its references.
- **Verify:** `cargo test -p tesela-core` (backlink + node query), web e2e (assign → backlink shows).

---

## Deferred / out of scope (this milestone)

- Cascade rename/delete of Property/Tag pages (name is the FK; no enforcement) — robustness follow-up.
- Per-(note_id,block_id) value index widening (`idx_block_props_value` excludes ids today).
- Collections (manual groupings) — distinct from Sets; later.
- Property UI-position / hide-empty advanced layout controls beyond the basics.
- Whiteboarding, AnyType widget rail (separate roadmap items 3c/3d).

---

## On completion (doc + memory updates)

- `decisions.md` — the 6 locked decisions + the foundation-first rationale + the dedicated-op-vs-field architectural choice (once review picks).
- `roadmap.md` Now — mark step 3(b) progress per phase; carry per-phase checkboxes in `current-state.md` `## Plan`.
- Memory — update `project_structured_first_crdt_truth` (multi-value no longer deferred — ships here); add a `project_property_system_milestone` note pointing at this spec; the new-entity guard as a product decision.
- harness-deck — the design-plan roadmap card (`20260605-properties-design-plan`) is the dashboard record.

---

## Architectural review addendum (2026-06-05)

Adversarial review (workflow `properties-phase1-arch-review`, 7 lenses, all claims code-verified). **This supersedes the Phase-1 op/migration framing above.**

### Resolved decisions
- **Op shape = DEDICATED PROPERTY OPS** (not a `BlockUpsert.properties` field — a field still rides the stale-base whole-block `text_c.update()` Myers-diff → per-key LWW, defeats multi-value union). Shape:
  - `BlockPropertySet { note_id:[u8;16], block_id:[u8;16], key:String, value:PropOp }`
  - `PagePropertySet  { note_id:[u8;16], key:String, value:PropOp }` (note_id on BOTH — `doc_for_note_mut` needs it)
  - `enum PropOp { SetScalar(PropScalar), SetText(String), AddToList(PropScalar), RemoveFromList(PropScalar), Clear }` where `PropScalar = String|i64|f64|bool` (plain Rust, NOT `loro::LoroValue` — wire must not couple to the CRDT lib version).
  - Append variants at END of `OpPayload` + an `OpKind` discriminant each. `prop_keys` maintenance lives in the apply arm, not on the wire. `record_local`/`OpPayload` is a LOCAL apply API — never serialized to wire/disk (wire = opaque Loro deltas; persistence = Loro snapshots), so `#[serde(default)]` is wire-irrelevant for these variants (keep it only on FFI request structs).
- **Container topology = as proposed; `prop_keys` STAYS.** `props` `LoroMap` via `get_or_create_container`; scalar → **primitive** `LoroValue` (`props.insert`, zero sub-containers — snapshot-budget load-bearing, do NOT containerize scalars); text → nested `LoroText`; multi/tags/multi-ref → nested `LoroList`; node-ref single → primitive id string. **Always `get_or_create_container` at a stable key, NEVER `insert_container` at an existing key** (mints a rival container that overwrites instead of merges). Same proven pattern as `text_seq`.
- **Failure policy = COERCE-AND-KEEP, SURFACE-IN-UI, NEVER REJECT at write/index** (CRDT-is-truth → a reject is unenforceable across opaque-delta peers; validation is a view). Out-of-choices → store verbatim + advisory invalid (recompute lazily at read; do NOT add a stored `valid` column that needs cross-note re-index). Unknown value_type → degrade to text. Dangling extends / missing Property page → keep partial + badge. Malformed `choices_json` → our-bug, error-log + degrade. **Remove "error/reject at index" from the plan.**
- **Page-props indexing in Phase 1 = NO.** Zero SQLite schema delta; index stays downstream of materialized markdown. Phase-1 index obligation = one golden round-trip test, not new tables.

### Blocking issues folded in (must-fix before/within Phase 1)
1. **Migrate-on-APPLY, not just read.** On EVERY `BlockUpsert` apply: strip recognized `key:: value` continuation lines from incoming `text`, fold into `props`/`prop_keys`, write `text_seq` prose-only — one atomic commit, idempotent. (Mixed-fleet old peer authors in-text props; without this they re-inject + double-emit.)
2. **NoteUpsert non-authoritative over props.** `set_page_properties` clears+repushes whole `page_props`; reseed (`clear_block_tree`+`seed_tree_from_flatblocks`) drops props. Make prop ops the SOLE writers of `props`; gate reseed off props-only deltas; reconcile `set_page_properties` so NoteUpsert can't clobber a concurrent `PagePropertySet`.
3. **Disjoint-twin heal must carry props.** `PeerBlockChange` (loro_engine ~2095) is text-only; dedup tombstones the loser twin → its `props` vanish = reintroduces the data-loss class. Grow `PeerBlockChange` with resolved props; re-assert onto survivor. Acceptance: two disjoint twins each w/ a distinct property → merge → BOTH survive.
4. **`prune_bare_leaf_blocks` (note_tree.rs:226) deletes property-only blocks** once props leave `text` (`bare = text.trim().is_empty()`). Treat non-empty `props` as non-bare. Regression test.
5. **Re-point the write path off the text-diff.** `set_block_property` (notes.rs:1675) → `upsert_block_property_in_note` (whole-note rewrite) → re-diff = the clobber path. Route emits `BlockPropertySet` directly (DELETE the rewrite+rediff); `clear_block_property` → `Clear`/`RemoveFromList`.
6. **Reconcile post-save logic.** `apply_post_save_bumps_with_info` + `apply_dependency_cycles` (notes.rs ~310) read `block.properties` from re-parsed markdown (recurring-roll, dependency unblock) → must read from container / re-materialized view or they regress.
7. **Canonical materialization = prose lines (text order) THEN property lines (`prop_keys` order), appended after prose** (Logseq-compatible reflow). `FlatBlock` gains ordered `properties: Vec<(String, MaterializedValue)>`; `note_tree` owns the join — do NOT rebuild property lines back into `text` in the engine.
8. **ONE shared `prop_keys` read helper** (materializer + index + chips): read `prop_keys`, dedup keeping FIRST occurrence, drop keys absent from `props`, append `props`-only keys lexicographically. Never iterate the `props` map for order. `Clear` removes from BOTH.
9. **Multi-value materialization = STABLE dedup** (first-occurrence over merged list order), comma-joined per existing `tags::` convention.
10. **Pin canonical scalar→string formatting per value_type** (checkbox→`true`/`false`; number no trailing-zero drift; date ISO) — else byte-determinism breaks despite equal CRDT state.
11. **Page-prop precedence + legacy teardown.** New reader prefers `props`/`prop_keys`, falls back to `page_props`; migrate-on-write CLEARS the legacy `page_props` entry when it lifts (else it resurfaces on the next old-build NoteUpsert).
12. **Old-FFI render-asymmetry (highest severity).** Old build imports new containers without error but its materializer can't READ them → renders property-less markdown + may re-broadcast it. Mitigations ALL required: (a) dual-read forever; (b) keep emitting `key:: value` lines in the materialized VIEW during transition; (c) migrate-on-write behind a flag **default-OFF**, flipped only after the WHOLE fleet (incl. iOS old FFI) is read-capable. No relay version negotiation exists. iOS FFI = long pole (rebuild `tesela-sync-ffi`, regen bindings, `xcodebuild`, copy header into BOTH `app/Tesela-iOS/CFFI/` AND `Generated/`).
13. **Fix stale doc-comment at `engine/loro_engine.rs:17`** claiming a per-block `properties: LoroMap` already exists (it does NOT) — it would make an implementer skip the migration.
14. **Correct the wire-compat framing** (above section's "decode old oplog" / "`#[serde(default)] properties`" was wrong for the Loro engine — wire is opaque `LoroDocUpdate`, persistence is snapshots). Real compat surface = dual-read + the render-asymmetry hazard.

### Phase-1 build order (TDD, foundation-first; steps 1–9 = the data-loss fix, must be green before the web seam)
1. value_type enum + `Property` struct + canonical scalar→string formatting (pure data; round-trip test).
2. Engine `props`/`prop_keys` get-or-create + per-type write/read helpers (mirror `write_block_text`).
3. The shared `prop_keys` read helper (dedup/order/stable multi-value).
4. `BlockPropertySet`+`PagePropertySet` ops + apply arms (resolve via `doc_for_note_mut`/`find_node_by_block_id`, `prop_keys` maint, commit, return `Some(note_id)`). Convergence tests: concurrent prose splice + prop set → both survive; concurrent AddToList → union.
5. Materializer: `FlatBlock.properties` + `serialize_note` join. **REVIEW-GATE determinism test:** same prop ops in different orders on two engines → import/export converge → `render_note_full` byte-identical.
6. Lazy migrate-on-write on APPLY (strip from text → props, prose-only `text_seq`, one commit; page_props → root props + clear legacy). Dual-read. Behind flag default-OFF.
7. Pruner fix (non-empty props = non-bare).
8. NoteUpsert non-authoritative over props (reseed/`set_page_properties` don't clobber).
9. Disjoint-twin heal extension (`PeerBlockChange` carries props; both survive).
10. Re-point routes (`set_block_property`/`clear` emit ops; reconcile post-save logic). `cargo test -p tesela-server` green.
11. FFI surface (mirror `splice_block_text`; regen bindings; build; copy header to BOTH dirs; `xcodebuild`).
12. Index passthrough golden test (NO schema change; container→materialized→`parse_blocks`→`block_properties` rows == pre-migration).
13. Web seam (after engine green): outbound detect-strip-emit at line-termination (NOT per-keystroke) via a new `setActiveBlockProperty` on the Loro/WS path — rewrite `applySlash` "property"/"date" + `writePropertyContinuation` + `/s` off the full-doc-replace; a SECOND inbound subscription on `props` (line-replace re-render, not the char-remap path); unify editor + TagTable/Kanban onto the one op.

### Resolved PRODUCT decisions (2026-06-05, Taylor via harness-deck `20260605-properties-product-qs`)
1. **Property reflow = APPROVED.** Mid-prose properties reflow to the block END on first materialize (Logseq-compatible; determinism-required). → P1.5 materializer.
2. **Out-of-choices guard default = ADD AS NEW CHOICE.** Typing a select value not in `choices` defaults to "add '<value>' as a new choice" (curated vocab grows naturally); "keep as a one-off (unvalidated)" stays available as the secondary action. → Phase-2 new-entity guard.
3. **In-editor chip timing = RENDER-ON-BLUR.** A committed `key:: value` line stays raw text while the block is focused and renders as a chip on blur (mirrors the existing markdown-on-unfocus behavior; caret-safe). → P1.13 web seam.

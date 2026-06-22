# Spec — Rock-solid per-type property/type system (Anytype/Logseq-DB parity)

Status: **APPROVED 2026-06-22** (5 product decisions locked below; Phase 1 implementation in progress) · Author: Opus · v2 — revised after a 3-lens adversarial review caught a dual-resolver blocker + citation errors
Supersedes the stale `memory/project_property_system_vision.md`.

## 1. Goal & non-goals

**Goal.** Make the type/property system *rock solid* before views. The unifying capability behind every gap Taylor named is **per-type property configuration** — the same global Property (e.g. `Status`) carrying *different* config *per type*:

- **Per-type choices** — `Task` Status = `[todo, doing, done, blocked]`; `Project` Status = `[planned, active, shipped]` — one global `Status`, different option lists.
- **Per-type visibility** — `on_new` (auto-added to a new block of that type) / `on_set` (available, shown only when valued) / `hidden`.
- **Per-type default** — `todo` for Tasks, `backlog` for Projects.
- **Type metadata** — a Tabler **icon** + a **plural** name per type.

**Non-goals (this spec).** View rendering (kanban/table/gallery) — deferred (keyboard-first is the hard part there, tracked separately). Computed/rollup properties. The bidirectional `node` reference picker. Property/type **rename propagation** (see §7 limitations).

## 2. Current architecture (verified 2026-06-22, citations corrected in v2)

Everything-is-a-page is already real:

- **Tag page** = note `type: "Tag"`; frontmatter `tag_properties: [Status, Priority]` (string array), `extends: "Root Tag"`, `icon` (emoji default `📄`), `color`, plus an existing per-tag choice filter `hidden_{PropertyName}: [choice…]`. Built-ins seeded in `crates/tesela-server/src/lib.rs:196-234` (NOT a fixtures file).
- **Property page** = note `type: "Property"`; frontmatter `value_type` (9 types, `property.rs:11-38`), `choices`, `default`, `multiple_values`, `hide_empty`, `hide_by_default`, `description`, plus **web-only** chip metadata (`chip_icon`, `chord_key`, `nl_triggers`, … `property-registry.ts:41-80`).
- **Caches** (DB-only, rebuilt on every note write via `index.reindex`): `tag_defs(id,name,extends,icon,color,properties_json,note_id)` + `property_defs(id,name,value_type,choices_json,default_value,multiple_values,hide_empty,description,note_id)` (`db/schema.rs:179-201`). **No synced schema** — Tag/Property pages sync as ordinary Loro `NoteUpsert` (`crates/tesela-server/src/routes/notes.rs:~1190-1210`); caches rebuild on index. ⚠ **`hide_by_default` is NOT a `property_defs` column** — it is read **only client-side** (`property-registry.ts:111`), never by the Rust resolver.
- **TWO resolution engines** (the key structural fact, §3.2):
  1. **Rust** `get_resolved_tag_def` (`db/sqlite.rs:529-629`) → consumed by **view/config** surfaces (TagTable, KanbanBoard, TagPropertyConfig, Inbox) via `GET /types`.
  2. **Client-side TS** `buildRegistry` + `getTagPropertyDefs` (`property-registry.ts:84-221`) → reads raw Property/Tag-page frontmatter directly; consumed by the **editor seeding + visibility** path (`BlockOutliner.svelte`, `block-tags.ts`). **This path never calls `GET /types`.**
- **Create/seed** (web): adding a tag (`toggleBlockTag`, `block-tags.ts`) appends empty `key:: ` lines for **non-`hide_by_default`** props (filtered via the TS registry); `BlockOutliner.autoFillTagDefaults` (`BlockOutliner.svelte:198-208`) emits `BlockPropertySet` ops for props with a default (idempotent). Render hiding = `cm-decorations` `hiddenKeysFacet` (`cm-decorations.ts:365-377,883`).
- **Config UI** (web): `TagPropertyConfig.svelte` — add/remove names, toggle `hide_by_default`/`hide_empty` (**global** to the Property), edit per-tag `hidden_{Prop}`. Reads Tag-page frontmatter directly.
- **Icon render**: `resolveChipIcon` (`icon-registry.ts:63-71`) resolves a **bare Tabler name** (e.g. `checkbox`) from a **curated `TABLER_ICONS` subset** to a component, with emoji/raw-string fallback. A name not in the subset silently renders as text.
- **iOS**: `LocalQueryEngine` has **zero property-definition awareness** (`LocalQueryEngine.swift:1-42`); no iOS property registry.

The only missing concept: a property's `choices`/`default`/visibility is shared across every type. `hidden_{Prop}` is the *only* per-type override today (subtractive, choices-only, web-only).

## 3. Design

### 3.1 On-disk format

Keep `tag_properties: [names]` as the *membership* list (back-compat untouched). Add an optional sibling map + two metadata keys, written as **single-line FLOW YAML** (gray_matter parses inline maps via `pod_to_json`; the existing single-line `updateFrontmatterKey` writer persists the whole map as one line's value — no nested-YAML writer needed):

```yaml
icon: checkbox            # NEW — bare Tabler name (curated subset) or emoji
plural: Tasks             # NEW — plural display name
tag_properties: [Status, Priority, Deadline]
property_overrides: {Status: {choices: [todo, doing, done, blocked], show: on_new, default: todo}, Priority: {show: on_set}}
```

`property_overrides` is keyed by property name (**matched case-insensitively**, §3.5). Membership (`tag_properties`) and config (`property_overrides`) are separate: an override for a property not in the resolved membership set is **ignored**; a property in membership with no override uses the global config.

### 3.2 TWO resolution engines must BOTH implement the merge

This is the load-bearing correction. The editor's seeding/visibility reads the **client-side TS registry**, not `GET /types`. So the override merge must be implemented **twice** — in `get_resolved_tag_def` (Rust, for views/config) **and** in `getTagPropertyDefs`/`buildRegistry` (TS, for the editor) — and kept in sync, exactly like the query DSL and recurrence engines already are (Rust + TS + Swift mirrors). Phase 1 delivers **both**; the per-phase acceptance below names which engine each surface reads. Shared test vectors keep them honest.

### 3.3 Choice override = REPLACE, then subtract

- `property_overrides.{Prop}.choices` **replaces** the global Property's choices for that type's instances. The global page's `choices` is the fallback for untyped `key:: value` and for types with no override.
- Legacy `hidden_{Prop}` (and its new alias `property_overrides.{Prop}.hide_choices`) then **subtracts** from the effective list. Precedence: **replace → subtract**. (`choices` override + `hidden_{Prop}` = subtract from the replaced list.)
- Both layers live in **both** resolvers (§3.2). `hidden_{Prop}` is web-only today; Phase 1 ports it to the Rust resolver so kanban columns (Rust) and chips (TS) agree.

### 3.4 Visibility — 3-state `show` (+ server-side `hide_by_default`)

| `show` | seed on tag-add | in `/p` | shown when empty | legacy equivalent |
|---|---|---|---|---|
| `on_new` | yes (+ default) | yes | yes | `hide_by_default:false` |
| `on_set` | no | yes | no (visible only when valued — per-type `hide_empty`) | (new) |
| `hidden` | no | yes | no | `hide_by_default:true` |

Back-compat derivation when a type has no `show` override: `on_new` if `hide_by_default=false`, else `hidden`. **Because that derivation needs `hide_by_default`, Phase 1 adds `hide_by_default` (and confirms `hide_empty`) to `property_defs` + `index_type_info`** so the *Rust* resolver can derive `show` too (the TS registry already reads it). (Open decision §6-B confirmed both engines.)

### 3.5 Merge mechanics (both engines)

The override merge is a **separate pass**, not folded into the existing name loop — because the name dedup (`sqlite.rs:582-583`) keeps only the first (child) occurrence and discards parent tag rows *before* the resolve loop. So:

1. Walk tag rows child→parent (the existing `extends` walk).
2. Build `overrides: Map<lower(prop) → override>` with **first-insert-wins** (child beats parent — same precedence as the name dedup, stated explicitly because they are two distinct operations).
3. In the resolve loop, for each resolved property look up `overrides[lower(name)]` and flatten `choices`/`default`/`show`/`hide_choices` into the returned `PropertyDef`.

Edge cases (Phase 1 Verify must cover each): (a) an override applies to a property by name regardless of which ancestor's `tag_properties` lists it (membership = ∪ along the chain, config = child-wins); (b) an override for a property not in the resolved membership set is ignored; (c) an override for a property with **no global Property page** still applies its `choices`/`default` to the text-stub PropertyDef. `get_all_tag_defs` (`sqlite.rs:632-691`) — CORRECTION (verified during impl): it does **not** walk the `extends` chain (it resolves each tag's own direct `tag_properties` only). Apply the separate-pass merge over its single own-row overrides (a one-deep chain), consistent with that existing no-inheritance behavior. Legacy `hidden_{Prop}` is folded into the cached `property_overrides_json.{Prop}.hide_choices` at **index time** (`index_type_info`), so the resolver's only subtract source is the cached map and both engines agree.

The flattened `PropertyDef` is the *effective* config for consumers; the **config editor reads the raw `property_overrides` map straight from Tag-page frontmatter** (TagPropertyConfig already reads frontmatter) so it can show overridden-vs-inherited.

### 3.6 Icons (Tabler) + plural

- `icon:` = a **bare Tabler name** (e.g. `checkbox`) routed through `resolveChipIcon` (`icon-registry.ts:63-71`), which already falls back to emoji/raw text. Constraint: the chosen name must be in the curated `TABLER_ICONS` subset or it renders as text — Phase 2 adds the type's icons to that subset. Emoji still accepted (default `📄`).
- `plural:` = new top-level Tag frontmatter string → new `tag_defs.plural` column; falls back to `name`. Used wherever a type is labelled plural.

## 4. Phases (each one reviewable commit; Verify is the gate)

- **Phase 1 — data model + BOTH merge engines (Rust core + TS registry).** New `tag_defs.property_overrides_json` + `tag_defs.plural` columns (ALTER ADD COLUMN — table is owned by migration 005; append the tuple to `MIGRATIONS` at `schema.rs:12`, bump `SCHEMA_VERSION` for hygiene); add `hide_by_default` to `property_defs` + `index_type_info` (`sqlite.rs:208`, INSERT at 253-265 — also read `metadata.custom["property_overrides"]`+`["plural"]`). Implement the separate-pass override merge (§3.5) in `get_resolved_tag_def`+`get_all_tag_defs` (Rust) **and** `getTagPropertyDefs`/`buildRegistry` (TS), with case-insensitive keys, replace-then-subtract choices, and `show`/`hide_by_default` derivation. `PropertyDef.show` (Rust + `ts-rs` regen) + `PropertyDefinition.show` (TS). Built-ins (`server/lib.rs:196-234`) get example overrides (Task vs Project Status) written as inline FLOW YAML in the existing `\n`-escaped frontmatter strings. **Acceptance:** `GET /types/Task` and the TS registry BOTH return Status `[todo,doing,done,blocked]`+`show:on_new`; `GET /types/Project` + TS BOTH return Status `[planned,active,shipped]`; a no-override tag is byte-identical to before; the §3.5 edge cases pass. **Verify:** `cargo test -p tesela-core` (resolver + edge + `hidden_` parity tests) + `cargo test -p tesela-server` + `cd web && npm run check && npm run test:unit` — the registry-vs-Rust parity vector lives in `test:unit`, NOT `check` (which only typechecks). ⚠ The `tesela-server` `put_base_diff.rs` sync tests are non-deterministically flaky under the full parallel run (pre-existing shared-resource race; pass in isolation + as a file) — not a Phase-1 regression.
- **Phase 2 — seeding + visibility + icon/plural render (web).** Route `toggleBlockTag` seeding off `show==on_new` (not `!hide_by_default`); per-type `default` applied on seed; `cm-decorations` hides `on_set`(empty)/`hidden`; add the built-in type icons to `TABLER_ICONS`; render icon + plural on tag pages. **Acceptance:** adding `#Task` seeds only `on_new` props with per-type defaults; `on_set`/`hidden` don't seed; Task page shows its icon + "N Tasks". **Verify:** `npm run check` + Chrome-DevTools product-test.
- **Phase 3 — config UI (web).** Per-property override editor in `TagPropertyConfig.svelte` (choices / `show` / default), reading the raw `property_overrides` map from frontmatter and writing it back as single-line FLOW YAML via `updateFrontmatterKey`; Tabler icon picker; plural field. **Acceptance:** editing Task's Status choices persists to `task.md` `property_overrides` + re-resolves in both engines. **Verify:** `npm run check` + product-test (round-trip).
- **Phase 4 — polish.** Per-choice color/icon (Status `done`→green) on the Property page + chip render; defaults-on-create audited; `on_set`/`hide_empty` empty-suppression audited. **Verify:** `npm run check` + product-test.
- **Phase 5 — iOS parity (later milestone, spec'd separately).** An iOS property-registry cache from `GET /types`+`/properties`; mirror the merge in Swift; offline contract per §6-D.

## 5. Product decisions — LOCKED 2026-06-22 (Taylor: "do your best thoughts on it, then lock it in")

1. **Choice override = full REPLACE** (a type states its own list; global is the fallback for untyped/un-overridden use). LOCKED.
2. **`on_set`** = settable in `/p`, **not** auto-seeded on tag-add, **shown when it has a value, hidden when empty** (per-type `hide_empty` semantics) — the "optional but visible-when-used" middle state. LOCKED.
3. **Override depth** = choices / visibility / default only, **never** `value_type`. LOCKED.
4. **`hidden_{Prop}`** = kept working as an alias for `property_overrides.{Prop}.hide_choices` (dual-read; precedence replace-then-subtract). LOCKED.
5. **Icons** = bare Tabler name (curated `TABLER_ICONS` subset) **or** emoji; prefer Tabler. LOCKED.

## 6. Architectural calls I already made (review-driven; flagging, not asking)

- **A. Two engines.** The merge is implemented in **both** the Rust resolver and the TS registry (mirrors, kept in sync via shared vectors), because the editor never reads `GET /types`. Phase 1 delivers both.
- **B. `hide_by_default` → server index.** Added to `property_defs`+`index_type_info` so the Rust resolver can derive `show` (parity with the TS side), rather than leaving the derivation client-only.
- **C. On-disk = nested map as single-line FLOW YAML** (not flat `override_Status_choices` keys), persisted via the existing `updateFrontmatterKey` pipeline.
- **D. iOS offline fallback = FREE-TEXT** (not the global choice list) when no per-type cache exists — presenting globally-valid-but-type-invalid options is worse than free text. Phase 5 adds the cache.

## 7. Known limitations (documented, not solved here)

- **Rename** of a property or type does **not** rewrite `property_overrides`/`tag_properties`/`extends` references (all name-keyed by design today); an orphaned override silently no-ops. A rename-propagation pass is a separate future task.
- Until Phase 5, iOS in `.relay` (offline) shows free-text entry for typed properties (no per-type choice picker).

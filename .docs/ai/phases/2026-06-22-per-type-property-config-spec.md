# Spec — Rock-solid per-type property/type system (Anytype/Logseq-DB parity)

Status: DRAFT for Taylor review · Author: Opus · 2026-06-22
Supersedes the stale `memory/project_property_system_vision.md` (89-day-old "current state" is wrong — most of the foundation shipped).

## 1. Goal & non-goals

**Goal.** Make the type/property system *rock solid* before building views. The unifying capability behind every gap Taylor named is **per-type property configuration**: the same global Property (e.g. `Status`) carrying *different* config *per type*. Concretely:

- **Per-type choices** — `Task` Status = `[todo, doing, done, blocked]`; `Project` Status = `[planned, active, shipped]` — same global `Status` property, different option lists.
- **Per-type visibility** — a property is `on_new` (auto-added to a new block of that type), `on_set` (available but not auto-added, shown only when valued), or `hidden`.
- **Per-type default** — `Status` defaults to `todo` for Tasks, `backlog` for Projects.
- **Type metadata** — a **Tabler icon** per type and a **plural** name (Anytype-style).

**Non-goals (this spec).** View rendering (kanban/table/gallery) — deferred; the known hard part there is keyboard-first, tracked separately. Computed/rollup properties. The bidirectional `node` reference picker.

## 2. Current architecture (verified 2026-06-22 against code)

Everything-is-a-page is **already real**:

- **Tag page** = markdown note with `type: "Tag"`; frontmatter: `tag_properties: ["Status", "Priority"]` (string array), `extends: "Parent"`, `icon`, `color`, and an existing per-tag choice filter `hidden_{PropertyName}: [choice…]`. (`notes/task.md`; `crates/tesela-fixtures/src/lib.rs:217`)
- **Property page** = markdown note with `type: "Property"`; frontmatter: `value_type` (one of 9: text/number/date/datetime/checkbox/url/select/multiselect/node — `property.rs:11-38`), `choices`, `default`, `multiple_values`, `hide_by_default`, `hide_empty`, `description`, plus web-only chip metadata (`chip_icon`, `chord_key`, `nl_triggers`, `value_chord_keys` — `web/src/lib/property-registry.ts:41-80`).
- **Caches** (DB-only, rebuilt on every note write via `index.reindex`): `tag_defs(id,name,extends,icon,color,properties_json,note_id)` and `property_defs(id,name,value_type,choices_json,default_value,multiple_values,hide_empty,description,note_id)` (`db/schema.rs:179-201`). **No sync table** — Tag/Property pages sync as ordinary Loro `NoteUpsert` (`notes.rs:1200-1222`).
- **Resolver** `get_resolved_tag_def(name)` (`db/sqlite.rs:529-629`): walks `extends` (max 10 hops), collects property *names* child-first + dedups, resolves each name against `property_defs`, returns `TypeDefinition{name,description,icon,color,properties: Vec<PropertyDef>}`; icon/color from the leaf type. `PropertyDef{name,value_type,values,default,required,hide_by_default,hide_empty}` (`types.rs:38-83`). Served by `GET /types`, `GET /types/{name}` (`routes/types.rs`).
- **Create/seed** (web): adding a tag (`toggleBlockTag`, `block-tags.ts:75-140`) appends empty `key:: ` lines for **non-`hide_by_default`** properties; `BlockOutliner.autoFillTagDefaults` (`BlockOutliner.svelte:198-208`) then emits `BlockPropertySet` ops for properties with a non-null default (idempotent). Visibility at render = `cm-decorations` hides `hide_by_default` keys via `hiddenKeysFacet` (`cm-decorations.ts:365-377,883`).
- **Config UI** (web): `TagPropertyConfig.svelte` — add/remove property names, toggle `hide_by_default`/`hide_empty` (these apply **globally** to the Property), and edit per-tag hidden choices (`hidden_{PropName}`). Icon/color are read-only today.
- **iOS**: `LocalQueryEngine` is a pure local eval engine with **zero property-definition awareness** (`LocalQueryEngine.swift:1-42`). No iOS property registry. Per-type config on iOS is a separate, larger effort.

**The one missing concept:** a property's `choices`/`default`/visibility are shared across every type that references it. `hidden_{PropName}` is the *only* per-type override today (subtractive, choices-only).

## 3. Design

### 3.1 On-disk format — a `property_overrides` map on the Tag page

Keep `tag_properties: [names]` as the *membership* list (which properties the type uses — back-compat untouched). Add an optional sibling map carrying the per-type config:

```yaml
title: Task
type: Tag
icon: tabler:checkbox      # NEW — Tabler icon name (see §3.6)
plural: Tasks              # NEW — plural display name
extends: Root
tag_properties: [Status, Priority, Deadline, Scheduled, Points]
property_overrides:        # NEW — per-type config, keyed by property name
  Status:
    choices: [todo, doing, done, blocked]   # REPLACE the global choices for this type
    show: on_new                            # on_new | on_set | hidden
    default: todo
  Priority:
    show: on_set
  Deadline:
    show: on_set
```

Rationale for a separate map (vs enriching `tag_properties` into a mixed string/object array): `tag_properties` stays a clean name list (zero back-compat risk); overrides are purely additive; it generalizes the existing `hidden_{PropName}` (which becomes `property_overrides.{Prop}.hide_choices`, §5-D). The frontmatter parser already turns any YAML object into `metadata.custom` JSON (`storage/markdown.rs:13-89`) — no parser change needed to *read* it.

### 3.2 Choice-override semantics — REPLACE, with the global as fallback

Per Taylor ("status is very general; a different status per type"): a type's `choices` **replaces** the global Property's choices *for that type's instances*. The global Property page's `choices` is the **fallback/superset** used by any type without an override and by the bare `key:: value` (untyped) usage. So:

- No override → global choices (today's behavior).
- `choices: [...]` override → exactly that list for this type.
- The existing subtractive `hidden_{Prop}` stays valid as a convenience (hide some global choices without restating the whole list); it is sugar for `property_overrides.{Prop}.hide_choices` (§5-D).

One global `Status` property, N per-type choice sets. (Open decision §5-A: confirm replace vs subset-only.)

### 3.3 Visibility — 3-state `show`

Replaces the binary `hide_by_default` *at the per-type level* (the global flag remains the default when a type has no `show` override):

| `show` | On tag-add (seed) | In `/p` menu | Rendered when empty | Maps to today |
|---|---|---|---|---|
| `on_new` | auto-add empty line + apply default | yes | yes | `hide_by_default: false` |
| `on_set` | **not** seeded | yes | no (shown only when valued) | (new middle state) |
| `hidden` | not seeded | yes (still settable) | no | `hide_by_default: true` |

Back-compat: a property with no `show` override resolves `on_new` if `hide_by_default=false`, else `hidden`. `on_set` is the genuinely new state (Open decision §5-B: confirm `on_set` = "available + visible-when-valued + hidden-when-empty" — effectively `hide_empty` semantics scoped per-type).

### 3.4 Resolver merge + index

- **Index** (`index_type_info`, `db/sqlite.rs:227-265`): also extract `metadata.custom["property_overrides"]`, `["plural"]` and store on the tag row. Add `tag_defs.property_overrides_json TEXT NOT NULL DEFAULT '{}'` and `tag_defs.plural TEXT` (icon column already exists). DB-cache-only migration (`migration 00X_per_type_overrides`); **zero sync impact** (§2).
- **Resolver** (`get_resolved_tag_def`, the merge point at `sqlite.rs:587-620`): after fetching each global `PropertyDef`, apply the override **collected along the same `extends` walk, child-overrides-win** (a child type's `property_overrides.Status` beats a parent's). Flatten the result into the returned `PropertyDef` so the field set the API exposes is the *effective* config: `values` ← override choices; `default` ← override default; plus a new `show` field on `PropertyDef`. `get_all_tag_defs` (`sqlite.rs:632-691`) gets the same treatment.
- **`PropertyDef` (types.rs:38-83)** gains `show: Option<Visibility>` (enum `OnNew|OnSet|Hidden`); existing `hide_by_default` stays for back-compat/derivation. Mirror the existing `ts-rs` derive so the web type regenerates.

### 3.5 API / back-compat

`TypeDefinition.properties` continues to carry resolved `PropertyDef`s — now with effective (overridden) `values`/`default`/`show`. **The API shape is additive** (one new optional field), so an un-migrated web client keeps working. A Tag page with no `property_overrides`/`plural` → byte-identical resolved output to today. Built-in type pages (`task.md`, `project.md`, …) gain example overrides as part of Phase 1's fixture update.

### 3.6 Icons (Tabler) + plural

- `icon:` accepts a Tabler name (the `TypeDefinition.icon` field already documents "emoji or Tabler", `types.rs:15-29`) — **verify the web type-icon renderer resolves Tabler names** (Phase 2 task; if it only renders emoji today, add a Tabler lookup mirroring the existing chip-icon path).
- `plural:` is a new top-level Tag frontmatter string → `tag_defs.plural`; falls back to `name` when absent. Used wherever a type is labelled in the plural (tag-page header "12 Tasks", view headers later).

## 4. Phases (each is one reviewable commit; Verify is the gate)

- **Phase 1 — core data model + resolver (Rust).** `property_overrides`/`plural` indexed; `tag_defs` columns + migration; resolver merge (child-wins along `extends`); `PropertyDef.show`; built-in fixtures get example overrides (Task vs Project Status). **Acceptance:** `GET /types/Task` returns Status choices `[todo,doing,done,blocked]` + `show`, `GET /types/Project` returns Status `[planned,active,shipped]`, from one global `Status` page; a no-override tag is byte-identical to before. **Verify:** `cargo test -p tesela-core` (new resolver tests) + `cargo test -p tesela-server`.
- **Phase 2 — seed + visibility behavior + icon/plural render (web).** `show` drives `toggleBlockTag` seeding (`on_new` only) + `cm-decorations` hiding (`on_set`/`hidden`); per-type `default` applied on `on_new`; Tabler icon + plural rendered on tag pages. **Acceptance:** adding `#Task` seeds only `on_new` props with their per-type defaults; `on_set`/`hidden` don't seed; the Task page shows its icon + "N Tasks". **Verify:** `npm run check` + a Chrome-DevTools product-test (tag-add seeds correctly).
- **Phase 3 — config UI (web).** Per-property override editor in `TagPropertyConfig.svelte` (choices / `show` / default), an icon picker (Tabler), a plural field; all round-trip to Tag-page frontmatter via the existing `updateFrontmatterKey` pipeline. **Acceptance:** editing Task's Status choices in the UI persists to `task.md` `property_overrides` + re-resolves. **Verify:** `npm run check` + product-test (round-trip).
- **Phase 4 — polish.** Per-choice color/icon (e.g. Status `done`→green) stored on the Property page + rendered in chips; defaults-on-create enforced everywhere; `hide_empty`/`on_set` empty-suppression audited. **Verify:** `npm run check` + product-test.
- **Phase 5 — iOS parity (later milestone).** An iOS property-registry cache fetched from `GET /types`+`/properties` (online) so iOS shows per-type choices/visibility and can edit offline; mirror the resolver merge in Swift. Bigger; spec'd separately when reached.

## 5. Open decisions (need Taylor's call before Phase 1)

- **A. Choice override = REPLACE** (a type states its own full list, global is fallback) vs subset-only (types can only hide global choices). Spec assumes **replace** (matches "different status per type"). Confirm.
- **B. `on_set` semantics** = "settable + shown when valued + hidden when empty" (per-type `hide_empty`). Confirm, or define differently.
- **C. Per-type override depth** — also allow overriding `value_type` per type, or only choices/visibility/default? (value_type override is rarely sane; spec excludes it.) Confirm exclude.
- **D. `hidden_{Prop}` migration** — keep the legacy subtractive key working *and* fold it into `property_overrides.{Prop}.hide_choices` (dual-read), or hard-migrate built-ins now? Spec assumes dual-read (no breakage). Confirm.
- **E. Icon source of truth** — Tabler names only, or keep emoji-or-Tabler? Spec assumes accept both, prefer Tabler.

## 6. iOS scoping note

iOS has no property registry today, so per-type config is **server-resolved**: iOS reads effective defs from `GET /types` when online and renders chips accordingly; offline (`.relay`) it degrades to the raw `key:: value` (no per-type choices). A true offline iOS property registry (Phase 5) is a later milestone — flagged here so the web/server v1 isn't blocked on it.

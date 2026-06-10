# Saved Views registry — spec (product-locked 2026-06-10)

Taylor's product decisions (AskUserQuestion round, this session):
1. **Dedicated views REGISTRY** — first-class synced objects, NOT Query pages, NOT device-local.
2. **DSL-first editing + chip builders** — a text query box is primary (Todoist-filter feel) with
   autocomplete; the existing chips become one-tap inserters that write into the query string.
3. **Views ARE the Inbox surface** — the Inbox tab/pane becomes the view switcher; "Inbox" is the
   built-in default view. Target: 6–12 user views (Todoist/Jira-saved-filters mental model).

His Inbox definition (the seeded default, user-editable):
`status:backlog,todo -has:scheduled -has:deadline` — "backlog or todo status, no scheduled or
deadline date". Use case quote: "I like to be able to save queries as views… 6-12 different views
I look at on a regular basis."

## Model (spec-derived)
- Registry = ONE dedicated always-resident Loro doc (mirror the index-doc precedent in
  tesela-sync — find how `_index` is created/synced/excluded-or-included in relay streams and make
  the views doc ride the SAME machinery; it's small and changes rarely).
- View entry: `{ id (uuid), name, dsl (string), order (int), builtin (bool), display: { mode:
  list|table|kanban, groupBy?: key, showDone?: bool } }`. CRDT shape: map-per-view + an ordering
  list; LWW per field is fine (single user, rare concurrent edits).
- Built-ins seeded once: **Inbox** (the DSL above). Built-ins are editable (dsl + display) but not
  deletable; "reset to default" affordance.
- Server routes: `GET /views`, `PUT /views/:id`, `POST /views`, `DELETE /views/:id`,
  `POST /views/reorder` — thin wrappers emitting registry ops (mirror how block-property routes
  emit engine ops). WS event on change.

## DSL extensions (tesela-core `query.rs` + the TS mirror + iOS LocalQueryEngine — ALL THREE)
- Multi-value OR within a key: `status:backlog,todo` (precedent: `tag-in:`; pick one syntax and
  make `status:`/`tag:` accept comma-lists uniformly).
- Verify `-has:scheduled` / `-has:deadline` work today (property-absence negation); fix if not.
- ⚠ THREE implementations exist (Rust query.rs, web TS parser, iOS LocalQueryEngine.swift). Add a
  SHARED CONFORMANCE FIXTURE: one JSON file of (dsl, block-fixture, expected-match) cases checked
  into the repo, consumed by tests in all three languages. Drift here = views silently differ per
  device — the audit's two-implementations lesson applies.

## Web UI
- The Inbox pane becomes **Views**: switcher (dropdown or tab-chips) + the result list (reuse the
  inbox ambient list rendering; `display.mode` table/kanban reuses QueryWidgetView/KanbanBoard).
- Editor row: DSL text input w/ autocomplete (keys, values from the property registry/status
  choices) + the existing InboxChips re-pointed as inserters into the string + save/save-as/rename/
  delete/reorder.
- Triage verbs (x/t/d keys, swipes N/A) keep working on results (the container-op paths).

## iOS UI
- Inbox tab top: horizontally scrollable view chips (selected = active view); result list as today.
- Edit: a sheet with the DSL text field + chip inserters + name + display mode. Registry read/write
  via new FFI fns (mirror set_block_property's shape) or server routes in .http mode.
- LocalQueryEngine executes the extended DSL in .relay (the conformance fixture gates it).

## Sequencing
Engine/DSL + fixture → server routes → web Views surface → iOS. After the 2026-06-10 wave (B3,
iOS round-2 bugs, importer, backups) ships. Est. 2–4 sessions.

## Out of scope (explicitly)
Cross-view dashboards, view sharing, per-view notifications, NL→DSL ("show me overdue") — later.

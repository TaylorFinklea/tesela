# Configurable NL triggers + lift-on-blur (spec)

Approved by Taylor 2026-06-08 ("looks good"). Generalizes Model B Part 2's hardcoded detection into a config-driven engine, + fixes the lift-timing bug. Builds on `2026-06-07-task-prop-modelB-spec.md`.

## Goals

1. **Lift-on-blur (timing fix).** Lifting tokens out of prose happens when you LEAVE the block (blur), not while focused. ⌘↵ make-task just tags; Enter moves off → source blurs → lifts. One gated lift path (blur), no double-lift. Fixes "it lifted p1/scheduled while I was still in insert mode."
2. **Config-driven detection.** Any property of any tag can declare NL triggers. The detector reads the block's detect-enabled tag → its `tag_properties` → each property's `value_type` + `nl_triggers`, and scans by type. Today's hardcoded p1-p4 / due / deadline / scheduled become config. Adding `Points` (number) to Task + triggers → `5 points` works, zero code.
3. **Dates keep the time** (`due thu at 8` → deadline Thu 20:00). Currently discarded.

## Config schema

- **Property page** frontmatter: `nl_triggers: [...]` (lowercased phrases). Parsed into `PropertyDefinition.nl_triggers: string[]` (property-registry.ts `parsePropertyPage`).
  - **select** (Priority): triggers ARE the value tokens — `nl_triggers: ["p1","p2","p3","p4"]`; a matched trigger sets the property to that value.
  - **date** (Deadline): `nl_triggers: ["due","deadline"]` — `<trigger> <NL date+time>`.
  - **number** (Points): `nl_triggers: ["points","pts"]` — `<number> <trigger>`.
- **Tag page** frontmatter: `default_date_property: "scheduled"` — a BARE NL date (no trigger) lifts to this property. Defaults to `"scheduled"`.
- A property is detectable iff it has `nl_triggers` non-empty, OR it is the tag's default date property (catches bare dates).

## Detection engine (`task-tokens.ts`, rewrite)

`detectTokens(text, spec)` where `spec: { properties: PropertySpec[]; defaultDateProperty: string }` and `PropertySpec = { key, valueType, choices, triggers }`. Scans the FIRST line:
1. **select** props w/ triggers → `\b<trigger>\b` → token value = the trigger (the choice).
2. **number** props w/ triggers → `(\d+)\s*<trigger>` → value = the number.
3. **date** props w/ triggers → `<trigger>\s+<phrase>`, validate phrase via `parseDateAndRecurrenceInput`; value = `YYYY-MM-DD[ HH:mm]` (KEEP time). recurrence → `recurring`.
4. **default date** prop → bare NL dates in still-unclaimed ranges (word-window scan, longest-valid-first, via `parseDateAndRecurrenceInput`), value = date(+time).
Track claimed char ranges so a triggered date isn't also bare-matched. Returns `DetectedToken[]` (from/to/kind/key/value/+level for color). `detectTaskTokens(text, spec)` → `{stripped, props}` (strip ranges, build props). `resolveDetectSpec(blockTags, config)` → merged spec for the block's detect-enabled DIRECT tags, or null.

## Config plumbing

- **`DetectConfig`** = `Map<lowercased tag, TagDetectSpec>` (only tags with `detect_tokens`). `TagDetectSpec = { defaultDateProperty; properties: PropertySpec[] }`, built in BlockOutliner via `getTagPropertyDefs(tag, …)` + each def's `value_type/choices/nl_triggers`.
- Replace `detectEnabledTagsFacet` (Set) with `detectConfigFacet` (DetectConfig). BlockOutliner computes + passes it as the `detectConfig` prop; BlockEditor sets the facet (compartment) for cm-decorations.
- cm-decorations highlight: `const spec = resolveDetectSpec(getBlockTags(doc), config); if (spec) highlight detectTokens(doc, spec)` (priority→per-level color, date→teal, number→a mark).

## Timing refactor (lift-on-blur)

- **Remove** the Enter-handler lift (BlockEditor ~1688) — revert to the plain split.
- **Remove** the make-task lift (BlockOutliner `handleStatusCycle`) — revert to just tagging + status (the 2b detectTaskTokens block goes away).
- **Add** the blur lift (BlockEditor blur handler ~1486): `const spec = resolveDetectSpec(getBlockTags(doc), detectConfig); if (spec) { det = detectTaskTokens(doc, spec); if props → v.dispatch(strip) + onSetProperty per prop }`. Gated on `!showSlashMenu && !showAutocomplete`. Single path → no double-lift guard needed. Enter→source blur→lift covers the "Enter to commit" case.

## Seeds

- Priority page: `nl_triggers: ["p1","p2","p3","p4"]`.
- Deadline page: `nl_triggers: ["due","deadline"]`.
- Scheduled page: `nl_triggers: ["scheduled"]` (also the default).
- Task tag page: `default_date_property: "scheduled"` (already has `detect_tokens: true`).
- (Demo/optional) a `Points` number property + add to Task — verifies the zero-code path.
- Apply to: fixtures `crates/tesela-fixtures/src/lib.rs`, live mosaic, gitignored `notes/`.

## Verify (e2e)

- `due thu at 8` (Task block, blur) → deadline = `<date> HH:mm` (time kept), prose stripped.
- `fold laundry tom 5 points` (Task, Points property added) → scheduled tom + Points=5.
- bare `tomorrow` → scheduled (default date prop).
- ⌘↵ make-task does NOT lift while focused; blur lifts.
- non-Task / #journal block → nothing detected (gate unchanged).

## Carry-overs

- Fresh-block property race (pre-existing) — blur timing means the block's usually saved first.
- Time-value shape: `formatDateMonthDay` already renders `YYYY-MM-DD HH:mm`. DatePicker still stores date-only (separate; could align later).

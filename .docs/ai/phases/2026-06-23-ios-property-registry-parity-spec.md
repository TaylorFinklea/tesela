# iOS Property-Registry Parity (Phase 5) — Spec

Status: APPROVED (Taylor chose "full Phase-5 parity push", 2026-06-23)
Origin: real-device test findings — authoring a task on iPhone exposed 4 gaps.

## Problem (4 findings, one root)

While creating a task on iPhone, Taylor hit:
1. **Raw property lines** — after a server round-trip, the task block shows `status:: todo` / `tags:: Task` as raw editable text ("really distracting").
2. **No date authoring** — no reachable way to set `scheduled`/`deadline`.
3. **No slash commands** — `/p1`, `/scheduled`, `/status` do nothing.
4. **No inline NLP** — typing `p1` / `on Friday` isn't detected.

**Root:** iOS task/property handling is **entirely hardcoded** — it never consumes the property registry (no client-side build, no `/types`). Web drives creation, chips, slash, and NLP from a registry parsed off Property/Tag pages; iOS bakes a fixed task shape in. (#1 has an additional engine-level cause — see P5.1.)

## Investigation (2026-06-23, 3-agent map — see run wf_1d5a9034-873)

### The raw-lines mechanism (convergence-critical)
- `readEngineBlockText` (iOS) → FFI `read_block_text` (`tesela-sync-ffi/src/lib.rs:766`) → `loro_engine.rs:2093-2113` returns the **raw `text_seq`** with **no property dedup**. The *materialized* `<slug>.md` path (`flatblock_from_node`, `loro_engine.rs:2012-2048`) DOES dedup (line 2040 `dedup_intext_props_against_container`). So view mode (parses materialized md) is clean; the live-reconcile (reads raw FFI) is polluted.
- `text_seq` gets the property lines because **`reconcile_tree_to_blocks` (`loro_engine.rs:2974-3042`) writes the UNstripped `block.text` into `text_seq`** at lines 3000 (drift) + 3027 (new node). `parse_note` folds the `key:: value` continuation lines back into `FlatBlock.text` (`loro_engine.rs:2832`). The strip-and-lift that would keep `text_seq` prose-only exists ONLY in the BlockUpsert seed arm (`loro_engine.rs:4075-4093`), gated on `migrate_in_text` (default **OFF** — `loro_engine.rs:302-323`).
- Reaches the buffer: `reconcileOpenBlockLive` (`MockMosaicService.swift:1863-1900`) reads polluted `merged`, `splitTrailingTags` (`:713`) only peels trailing `#tags` (a `status::` line is NOT a trailing tag → stays in `body` → `rawText` at `:1895`), then `inserter.reconcile(toEngineText: merged)` (`:1898`) pushes raw text into the UITextView.

**Decision (raw-lines fix):**
- **SHIP NOW — iOS defense-in-depth strip (no convergence risk):** in `reconcileOpenBlockLive`, strip solely-`key:: value` lines from `merged` (reuse the `parseProperty` predicate already used at `MockMosaicService.swift:3072-3074`) BEFORE `splitTrailingTags` + `inserter.reconcile`, stripping consistently so the splice offsets stay aligned (the buffer is built from `displayText`+`#tags` = prose-only already, so a prose-only `merged` RESTORES alignment). Display-only — never writes `text_seq`.
- **DEFER — engine root fix:** strip-and-lift in `reconcile_tree_to_blocks` (mirror `4075-4093`) at lines 3000+3027. This is the proper source fix for ALL clients but is **gated on `migrate_in_text`/fleet-readiness** (an old FFI that can't read the container could re-broadcast a fleet-wide property erase — `loro_engine.rs:302-312`). Do NOT un-gate until the whole fleet is container-props-read-capable. Tracked as a separate convergence task, not in the P5 iOS scope.

### The registry (build client-side, NOT from /types)
- Web builds the registry **client-side from synced notes** (`web/src/lib/property-registry.ts:155-162`, no `/types` call). The rich layer — `nl_triggers` (`:92`), `choice_colors` (`:86`), `chip_*` (`:58-69`), `chord_key`/`value_chord_keys` (`:76,80`) — lives **only in Property-page frontmatter** (`parsePropertyPage:97-153` reads `note.metadata.custom`). The DB/API (`GET /types`, `/properties`) **does NOT carry these** (only name/value_type/values/default/show/hide_* — `crates/tesela-core/src/types.rs:68-93`). So iOS must parse Property/Tag pages, same as web.
- Resolution to mirror EXACTLY (`getTagPropertyDefs` `:356-429` + `applyOverride` `:321-343`, which mirror Rust `sqlite.rs:143-176`): walk `extends` child→parent (cycle-safe, max 10); union each tag's `tag_properties` deduped child-first; fold `property_overrides.{Prop}` + legacy `hidden_{Prop}` (first-insert/child wins); then per property — choices **REPLACE** if override has them, **SUBTRACT** `hide_choices`, `default` override wins, 3-state `show` = override else `hide_by_default ? "hidden" : "on_new"`.
- iOS today **drops** frontmatter `custom`: `APINoteMetadata` decodes only title/tags/note_type/created/modified (`MockMosaicService.swift:2077-2083`); the relay path uses single-line scrapers (`parseNoteTypeFromFrontmatter:2483`), no nested-YAML parser. So it can't see `nl_triggers: [...]` / `property_overrides: {...}` yet.

### The editor seams
- **Typed per-key seam (STRUCTURED, converging — PREFER):** `setBlockProperty(blockId:key:value:)` (`MockMosaicService.swift:3834`) → `.relay` `onLocalPropertySet` (`:194`) → FFI `BlockPropertySet` container op; `.http` POST `/blocks/set-property`.
- **Whole-list seam (coarse — the date sheet uses this today):** `setBlockProperties(id:properties:)` (`:3220`) → replaces the array → `scheduleWriteback`. Functional but re-pushes the whole list.
- Toolbar: `KeyboardToolbarItem` (10 cases); **default set lacks `.date`** (`KeyboardToolbarItem.swift:58`); dispatch `handleToolbarAction` (`BlockRow.swift:564`). No per-key `onSetProperty` closure on BlockRow yet (only whole-list `onSetProperties` `:77`).
- Slash: `SlashVerbs` (`EditorAutocomplete.swift:163`) all **text-inserting**; single dispatch `commitSuggestion` (`BlockRow.swift:410`); `Suggestion` (`:10`) carries only `insert: String`.
- NLP: `DateParser` (`Data/DateParser.swift`, full) used ONLY in `DateInputSheet`; not wired to live typing.
- Chips: `displayProperties`/`PropertyChip` already render from structured `properties` (`BlockRow.swift:165-191`).

## Phases (each = one fresh-context iteration; build/cut TestFlight per the always-cut convention)

Ordered so the distracting bug gets fixed FIRST, then the registry foundation, then registry-driven features.

### P5.1 — Raw-property-lines fix (the distracting bug) — iOS-only, no registry dependency
- Scope: the iOS defense-in-depth strip in `reconcileOpenBlockLive` (above). Strip solely-property lines from `merged` consistently before `splitTrailingTags`/`inserter.reconcile`.
- Acceptance: after making a block a task + a server/relay round-trip, the edit buffer shows ONLY the prose ("Do this thing") — no `status::`/`tags::` lines; chips still render in view mode; a concurrent same-block edit still converges (the strip is display-only).
- Verify: `xcodebuild` build succeeds; a unit test on the strip helper (property lines removed, prose + trailing tags preserved, offsets consistent). Real-device: create a task, confirm no raw lines appear on sync.
- Note inline: engine root fix (reconcile_tree_to_blocks strip-and-lift) is DEFERRED (migrate_in_text gate) — roadmap item, not here.

### P5.2 — Registry foundation (read-side)
- Scope: (a) a general frontmatter→`[String:Any]` parser (nested arrays + maps — `nl_triggers: [...]`, `property_overrides: {...}`, `value_chord_keys: {...}`); (b) Swift models `TypeDef`/`PropertyDef`/`Visibility`/`ChipLabelMode`/`ChipValueFormat`/`PropertyType` mirroring `property-registry.ts:8-93`; (c) a `Registry` with `buildRegistry`(from Property pages) + inheritance(from Tag `extends`) + `resolvedType(forTag:)` porting `getTagPropertyDefs` (chain walk + override merge + `applyOverride` semantics above); (d) re-add `custom` to the note decoders + build the registry in `refresh(from:)` from the synced Property/Tag notes — ONE local-parse path for all backends (relay has no server; http pages are already synced locally; mock seeds the built-ins).
- Acceptance: registry resolves Task→Status `[todo,doing,done,blocked]` show on_new default todo, Project→Status `[planned,active,shipped]`, priority `nl_triggers [p1..p4]`, deadline `nl_triggers [due,deadline]` — matching the web registry + server `/types` on the same mosaic.
- Verify: unit tests ported from `web/tests/unit/*registry*`/`*property-config*` (resolution + override REPLACE/SUBTRACT + 3-state show). `xcodebuild`. (Registry not yet wired to UI — pure read layer.)

### P5.3 — Date authoring (Finding 2)
- Scope: add `.date` to the default toolbar set (and/or `.setScheduled`/`.setDeadline` cases routing to `DateInputSheet` with the field preset); add a typed `onSetProperty:((key,value)->Void)` on BlockRow wired to `setBlockProperty` (the STRUCTURED seam) and make the date commit use it (replacing the whole-list path where a single key changes).
- Acceptance: from a task, set a scheduled and a deadline date (picker + the sheet's existing NLP field); both persist as structured properties, render as chips, and sync.
- Verify: `xcodebuild`; real-device set-a-date round-trip.

### P5.4 — Registry-driven slash commands (Finding 3)
- Scope: add a `SuggestionAction` discriminator to `Suggestion` (`.insertText` / `.setProperty(key,value)` / `.openDateSheet(field)` / `.setStatus(choice)`); branch in the single dispatch `commitSuggestion` → structured writes; generate verbs from the registry (each select property's choices → `/status <choice>`, `/priority <choice>`/`/p1`; date properties → `/scheduled`,`/deadline` open the sheet). Keep the existing format verbs.
- Acceptance: `/p1`, `/scheduled`, `/status doing` write the structured property (not raw text) and dismiss; the verb list reflects the resolved type's properties + choices.
- Verify: `xcodebuild`; on-device `/p1` sets priority chip.

### P5.5 — Inline NLP (Finding 4)
- Scope: wire `DateParser` + property `nl_triggers` into live typing (detect a token in the splice/onChange path → offer a suggestion chip to lift it into a structured property, applied via the typed seam; don't leave raw text). Reuse the existing suggestion-chip strip.
- Acceptance: typing `p1` / `due tomorrow` / `on Friday` surfaces a lift suggestion that sets the structured property; declining leaves the text as prose.
- Verify: `xcodebuild`; on-device NLP lift.

### P5.6 — Chip parity polish
- Scope: drive status/select chip colors from the registry `choice_colors` (the Phase-4 web feature); any remaining chip-metadata gaps (chip_icon/label_mode).
- Acceptance: a select value's chip renders its `choice_colors` color, matching web.
- Verify: `xcodebuild`; visual parity.

## Cross-cutting invariants
- **Convergence first:** never write property lines into `text_seq`; all property writes use the typed per-key seam; the raw-lines strip is display-only. Run `cargo test -p tesela-sync` for any engine-adjacent change.
- **Mirror, don't reinvent:** the resolution semantics MUST match web `applyOverride`/`getTagPropertyDefs` and Rust `apply_override` (the two engines already agree post-Phase-1; iOS becomes the third — port, don't redesign).
- **All-Opus + adversarial verify** per phase (the session convention); cut a TestFlight build after each shippable phase (Taylor is sole tester).

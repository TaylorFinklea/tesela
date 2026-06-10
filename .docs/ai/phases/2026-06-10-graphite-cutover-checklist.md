# Graphite cutover — B3 deletion checklist (parity sweep 2026-06-10)

Source: 4-agent read-only parity sweep (56 surfaces; full structured output incl. per-item file
evidence: `/private/tmp/claude-501/-Users-tfinklea-git-tesela/46a0c253-50ae-4737-8c9a-0866cacd082d/tasks/w04btliwj.output`,
key extracts inline below). Deletion target = `routes/v4`, `lib/components/v4`, `lib/components/v5`
chromes; preserve = `lib/v4` + `lib/v5` behavior modules + everything /g imports.

**GATE STATUS: ✅ B3 DONE (2026-06-10).** Taylor's product pass: "PASS — delete v4/v5 (I'm
fully on /g)". Deletion executed: relocate `1916cb5` + delete `a12a804` (+31/−4287; 21 files
deleted). `/v4` → redirect stub to `/`; `/v4/p/<slug>` → `/g#tile=<slug>` stub. Verified:
grep-zero `components/v4|v5` imports; svelte-check **0 errors** (the pre-existing
VoiceCaptureButton error died with the file); build clean; unit 221/221 (node 24, ci.yml
invocation); e2e vs static build + sandbox mosaics: nlp 10/10 (vite :5173 → sandbox :7788),
vim-counts 4/4, vim-undo 8/8, vim-registers 7/7, property-readmodel 9/9 fresh (8/9 first
run = the known disk-poll flake, A/B'd via fresh-mosaic rerun); browser QA 17/17 (/, /v4 +
/v4/p deep-link redirects, /p query-table, tag page, `:agenda` list triage, ⌘I peek, gear
Data tab, ⌘G graph), 0 page errors.

**Final preserve-list locations:**
- `lib/components/shell/` — ColonCommandLine, FullscreenOverlay, SettingsOverlay,
  PeekPopover, ScratchPruneSettings (+ Gate-A peers, `93607cb`)
- `lib/components/` root — TagPageRenderer, PageTagsChips (ex components/v4),
  CompactQueryView (ex components/v5) — moved `1916cb5`
- `lib/v4/` (commands.ts registry, tokens.css) + `lib/v5/` (leader-tree) behavior modules,
  `lib/renderers`, `lib/ambients`, `lib/buffer/*` — **incl. `buffer/migration.ts`: NOT an
  orphan, it hosts the live `loadFromLocalStorage`/`saveToLocalStorage` used by
  `buffer/state.svelte.ts`** (only its v4→v5 `migrate()` is now latent) — `ChordMenu.svelte`
- Deleted orphan: `lib/commands.ts` (zero importers; the live registry is `lib/v4/commands.ts`)
- Swept: `/v4` branch in `stores/active-pane-nav.svelte.ts` gotoNote; stale chrome comments
  in `routes/+page.ts` / `routes/+layout.svelte` / GrCommandPalette / GrLayoutTree

Historical sweep state below (pre-deletion): 30 present / 14 partial / 12 missing. B2
(default flip, `b46b756`) cleared the deep-link ordering hazard.

## Gate A — import hazards — ✅ DONE (`93607cb`, all moved to `lib/components/shell/`, QA'd on the static build)
- [x] `lib/components/v4/ColonCommandLine.svelte` (GraphiteShell — `:` ex-mode + GrRail quick-capture)
- [x] `lib/components/v4/FullscreenOverlay.svelte` + `SettingsOverlay.svelte` (settings ⌘,/gear + ⌘G graph; desktop Tauri menu)
- [x] `lib/components/v4/PeekPopover.svelte` (⌘I peek) + its `lib/renderers` deps `{OutlineTab,PropertiesView}.svelte`
- [x] `lib/components/v5/ScratchPruneSettings.svelte` (routes/settings/data → /g overlay Data tab)
- [x] `lib/v4/tokens.css` self-imports by the above (kept; lib/v4 is preserve-list anyway)
- [x] `ChordMenu.svelte` type export used by GrLeaderOverlay (kept in lib/components/ root; post-deletion importers: chord-keys, leader-tree, BlockEditor, BlockOutliner, GrLeaderOverlay)
- Preserve-list (already-safe, imported by /g): `lib/v4/commands.ts`, `lib/v5/leader-tree.svelte.ts`, `lib/fuzzy`, shared `buffer/*`, `stores/{station,colon-mode,pane-state,peek,fullscreen-overlay,toast,save-state}`, `state/shared`, `loro`, `ws-client`, `JournalView`, `BlockOutliner`, `ambients/inbox/chips`, `QueryWidgetView`/`KanbanBoard`/`CompactQueryView` (lib/components root + v5… verify final homes during the move).

## Gate B — capability gaps (feature work, ~1-2 sessions)
- [x] **GrPage note_type dispatch** — DONE. Dispatch mirrored inside GrPage (NoteRenderer itself stays in the deletion target): query → QueryWidgetView (CompactQueryView <350px), tag → TagPageRenderer, property → PropertyTypeConfig, `mode: document` → DocumentEditor, else BlockOutliner; PageTagsChips above body-text pages; `tesela:open-tag` listener added to GraphiteShell; query/property pages render full-width (no References pane), everything else keeps it. v4-/v9-token shim scoped on `.gr-outline`. Browser-QA'd on a sandbox mosaic (table, kanban, tag instances, property config, ordinary page, chip→tag nav all good).
  - LANDMINE for the B3 `rm`: /g now imports `lib/components/v4/{TagPageRenderer,PageTagsChips}.svelte` — move them out (Gate-A style) before deleting `lib/components/v4`.
  - Discovered: `CompactQueryView` reads `executeQuery` results as `{rows}` but the server returns `{groups}` → always "no results" (pre-existing; same on v5 narrow panes).
- [x] **Agenda triage parity** — DONE. GrAgenda gained a LIST mode (default; persisted in `tesela:graphite:agenda-mode`; `v` or the head's List|Week segmented control toggles) that embeds the v5 agenda ambient (`lib/ambients/agenda` — preserve-list) directly, so overdue deadline/scheduled buckets, x done / d reschedule (+ bulk DatePicker) / s skip, show-done, j/k and the 60d scroll are the SAME code paths as /v4 — zero duplicated triage logic. `.gr-aglist` re-maps the shadcn alias tokens (--foreground/--primary/…) onto Graphite tokens so the ambient + DatePicker render native inside `.gr-root`. Week mode shows a "⚑ N overdue" badge (overdue rows live before the visible week) that jumps to list. No GrLeaf change needed. Browser-QA'd on a sandbox mosaic: 21/21 checks incl. disk writes (status:: done, scheduled:: reschedule, recur-bump skip) + week h/l/t nav.

## Non-blocking gaps (accepted losses or later; Taylor may veto)
missing: Station Dashboard tab (section::pinned Query widget cards — power-user), History tab +
diff/restore, library/pages list (⌘K covers), **voice capture button on /g** (flag to Taylor),
⌘[/⌘] journey nav, Alt+1-9 tab switch, ⌘B sidebar, `tesela:open-tag` doc listener (folded into
Gate B dispatch), ambient placeholders (calendar / in-progress / dashboard / ai).
partial: ⌘W/⌘⇧W/⌘T deliberately unbound (browser-reserved), status-line hint hygiene,
tab/workspace persistence details, theme/token scoping, pinned/recents UI, inbox/linked-tasks/
properties-drawer fidelity notes — see the sweep output.

## Order of operations for B3 — ✅ ALL DONE
1. [x] Gate A moves (no behavior change) → `pnpm check` + build green (`93607cb`).
2. [x] Gate B features → browser-QA'd (`7cf456d`, `263c77e`).
3. [x] Deleted `routes/v4`, `lib/components/v4` (minus moved), `lib/components/v5` (minus
   moved); grep-zero imports; build + unit + e2e green; Taylor product-pass cleared the
   gate (`1916cb5` relocate, `a12a804` delete). ⚠ During the deletion a concurrent agent
   session hard-reset the working tree (uncommitted stage clobbered once) — redone +
   committed immediately; commit-early when sharing main with other live sessions.

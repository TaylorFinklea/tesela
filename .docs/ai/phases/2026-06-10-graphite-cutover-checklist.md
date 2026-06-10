# Graphite cutover — B3 deletion checklist (parity sweep 2026-06-10)

Source: 4-agent read-only parity sweep (56 surfaces; full structured output incl. per-item file
evidence: `/private/tmp/claude-501/-Users-tfinklea-git-tesela/46a0c253-50ae-4737-8c9a-0866cacd082d/tasks/w04btliwj.output`,
key extracts inline below). Deletion target = `routes/v4`, `lib/components/v4`, `lib/components/v5`
chromes; preserve = `lib/v4` + `lib/v5` behavior modules + everything /g imports.

**GATE STATUS: ❌ NOT GREEN.** 30 present / 14 partial / 12 missing. Blocking = the two
sections below. B2 (default flip, `b46b756`) cleared the deep-link ordering hazard.

## Gate A — import hazards (mechanical, ~½ session)
/g compiles against files INSIDE the deletion target. Move them (lib/graphite or neutral lib/)
before any `rm`:
- [ ] `lib/components/v4/ColonCommandLine.svelte` (GraphiteShell:51 — `:` ex-mode + GrRail quick-capture)
- [ ] `lib/components/v4/FullscreenOverlay.svelte` + `SettingsOverlay.svelte` (GraphiteShell:52 — settings ⌘,/gear + ⌘G graph; desktop Tauri menu depends on it)
- [ ] `lib/components/v4/PeekPopover.svelte` (GraphiteShell:53 — ⌘I peek) + its `lib/renderers` deps `components/v4/{OutlineTab,PropertiesView}.svelte`
- [ ] `lib/components/v5/ScratchPruneSettings.svelte` (routes/settings/data imports it → /g overlay Data tab)
- [ ] `lib/v4/tokens.css` self-imports by the above (keep; lib/v4 is preserve-list anyway)
- [ ] `ChordMenu.svelte` type export used by GrLeaderOverlay (type-only; file lives in lib/components/ root — NOT in the deletion set, just don't sweep it accidentally)
- Preserve-list (already-safe, imported by /g): `lib/v4/commands.ts`, `lib/v5/leader-tree.svelte.ts`, `lib/fuzzy`, shared `buffer/*`, `stores/{station,colon-mode,pane-state,peek,fullscreen-overlay,toast,save-state}`, `state/shared`, `loro`, `ws-client`, `JournalView`, `BlockOutliner`, `ambients/inbox/chips`, `QueryWidgetView`/`KanbanBoard`/`CompactQueryView` (lib/components root + v5… verify final homes during the move).

## Gate B — capability gaps (feature work, ~1-2 sessions)
- [ ] **GrPage note_type dispatch** — GrPage mounts BlockOutliner for EVERY non-daily page; /g loses: Query notes → QueryWidgetView/CompactQueryView (raw markdown today — the user's task lists ARE Query notes), tag pages → instances-of-tag section (TagPageRenderer), property pages → PropertyTypeConfig, document-mode → DocumentEditor, PageTagsChips strip + a `tesela:open-tag` listener (exists only in routes/v4/+layout:342). Mirror NoteRenderer's dispatch inside GrPage (reuse the components read-only — they survive deletion).
- [ ] **Agenda triage parity** — GrAgenda is a view-only week grid; v5 agenda's overdue buckets, mark-done (x), reschedule (d + bulk DatePicker), skip (s), show-done, 60d scroll are ABSENT on /g, and GrLeaf hard-routes `agenda` so the v5 ambient is unreachable. Port the triage verbs into GrAgenda (or add a list mode hosting the v5 ambient).

## Non-blocking gaps (accepted losses or later; Taylor may veto)
missing: Station Dashboard tab (section::pinned Query widget cards — power-user), History tab +
diff/restore, library/pages list (⌘K covers), **voice capture button on /g** (flag to Taylor),
⌘[/⌘] journey nav, Alt+1-9 tab switch, ⌘B sidebar, `tesela:open-tag` doc listener (folded into
Gate B dispatch), ambient placeholders (calendar / in-progress / dashboard / ai).
partial: ⌘W/⌘⇧W/⌘T deliberately unbound (browser-reserved), status-line hint hygiene,
tab/workspace persistence details, theme/token scoping, pinned/recents UI, inbox/linked-tasks/
properties-drawer fidelity notes — see the sweep output.

## Order of operations for B3
1. Gate A moves (no behavior change) → `pnpm check` + build green.
2. Gate B features → browser-QA'd.
3. THEN delete `routes/v4`, `lib/components/v4` (minus moved), `lib/components/v5` (minus moved);
   grep-verify zero imports; build + e2e green; Taylor product-pass on /g before the commit that deletes.

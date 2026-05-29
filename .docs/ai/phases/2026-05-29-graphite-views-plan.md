# Graphite Redesign — Daily-Driver Views Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Fill the shell's pane/tab bodies with the real daily-driver views — daily journal, page/project outliner, inbox triage, agenda week — by REUSING the editing engines (`BlockOutliner`/`JournalView` web, `BlockRow`+`MockMosaicService` iOS) and data layer, wrapped/re-themed to Graphite. Reach a state where the **daily flow works end-to-end against the live backend** so it's testable for real. Search is already done (web ⌘K `GrCommandPalette`; iOS native `.search`).

**Architecture:** Web — the CodeMirror editing engine is reused untouched; a NEW Graphite CM theme + decoration CSS (Task A1) makes the editor + inline tags/links/props look Graphite inside `.gr-root`. `GrPane` body renders the focused buffer by kind: daily→`JournalView`, page→`BlockOutliner`, inbox→`GrInbox` (new), agenda→`GrAgenda` (new). New non-editor views (`GrInbox`/`GrAgenda`) are Graphite presentation over the existing `api.executeQuery`/`api.getAgenda` + inbox-DSL helpers. iOS — `BlockRow` (inline editor) + `MockMosaicService` reused; new Graphite tab views (`GrDailyView`/`GrPageView`/`GrLibraryView`/`GrAgendaView`/`GrInboxView`) bind to the same service + models, replacing the `GrTabPlaceholder`s. Old v4/v5 web views + old iOS Views referenced only, never edited.

**PRIORITY ORDER (testable core first):** A1 → A2 (web daily) → A3 (web page) → B1 (iOS daily) → B2 (iOS page) are the *testable core*; A4/A5 (inbox/agenda web) + B3/B4/B5 (iOS library/agenda/inbox) follow. If time-bound, the core makes the app testable for the daily flow.

**Depends on:** foundation (`7083956`/`e316a6f`) + shell (`88e4dfe`/`c897b98`). Reuse the foundation primitives + shell components; don't recreate.

**Tiering (AGENTS.md):** Graphite view CSS = spec-derived → prescribed verbatim below. Editor/data reuse = codebase-derived → the plan names the exact components/APIs but REQUIRES reading the real source (`BlockOutliner.svelte`, `JournalView`, `BufferShell`/`NoteRenderer`, `cm-decorations.ts`, `api-client.ts`, `MockMosaicService`, `BlockRow.swift`) and mirroring — do NOT hand-write editor internals you haven't read.

---

## Verbatim Graphite view CSS (scoped under `.gr-root`)

**Daily journal:** `.gr-dayhdr{display:flex;align-items:center;gap:11px;padding:6px 8px 10px;margin-top:4px;} .gr-dayhdr .d{font-size:15px;font-weight:600;color:var(--fg);letter-spacing:-.01em;} .gr-dayhdr .dow{font-family:var(--mono);font-size:10.5px;color:var(--faint);text-transform:uppercase;letter-spacing:.08em;} .gr-dayhdr .ln{flex:1;height:1px;background:var(--line);} .gr-dayhdr.today .d{color:var(--coral);}` · `.gr-outline{flex:1;overflow:auto;padding:14px 18px;}`
**Block (static-render reference for the CM theme to match):** `.gr-blk{position:relative;padding:7px 10px 7px 8px;border-radius:9px;border-left:2px solid transparent;} .gr-blk.sel{background:var(--raised);border-left-color:var(--coral);} .gr-blk-main{display:flex;align-items:flex-start;gap:10px;} .gr-bull{width:7px;height:7px;border-radius:50%;background:var(--faint);margin-top:7px;flex-shrink:0;} .gr-blk.task .gr-bull{background:var(--task);} .gr-blk.done .gr-bull{background:var(--query);} .gr-blk-text{font-size:14.5px;color:var(--fg);line-height:1.45;letter-spacing:-.005em;} .gr-blk.done .gr-blk-text{color:var(--subtle);text-decoration:line-through;text-decoration-color:var(--faint);}`
**Inline marks (the CM decorations must match these):** `.gr-tagchip{display:inline-flex;align-items:center;height:18px;padding:0 7px;margin-left:7px;border-radius:5px;font-family:var(--mono);font-size:10.5px;background:var(--coral-dim);color:var(--coral);} .gr-tagchip.alt{background:rgba(232,105,127,.15);color:var(--task);} .gr-link{color:var(--project);background:rgba(116,147,232,.14);padding:0 4px;border-radius:4px;} .gr-mention{color:var(--person);background:rgba(174,144,230,.14);padding:0 4px;border-radius:4px;}`
**Inline property chips:** `.gr-props{display:flex;flex-wrap:wrap;gap:6px;margin-top:8px;} .gr-pchip{display:inline-flex;align-items:center;gap:7px;height:23px;padding:0 9px;border-radius:7px;white-space:nowrap;background:var(--surface);border:1px solid var(--line);font-family:var(--mono);font-size:11px;} .gr-pchip .k{color:var(--faint);} .gr-pchip .v{color:var(--fg2);} .gr-pchip.doing .v{color:var(--coral);font-weight:600;} .gr-pchip.high .v{color:var(--task);font-weight:600;}`
**Child items:** `.gr-kids{margin:6px 0 2px 18px;padding-left:14px;border-left:1px solid var(--line);} .gr-kid{display:flex;align-items:center;gap:9px;padding:5px 6px;border-radius:7px;cursor:pointer;} .gr-kid:hover{background:var(--raised);} .gr-kid .kb{width:5px;height:5px;border-radius:50%;background:var(--faint);flex-shrink:0;} .gr-kid .kt{font-size:13px;color:var(--fg2);flex:1;min-width:0;}`
**Linked-refs side pane:** `.gr-side-body{flex:1;overflow:auto;padding:12px 14px;display:flex;flex-direction:column;gap:9px;} .gr-refcard{padding:10px 12px;border-radius:10px;background:var(--raised);border:1px solid var(--line);} .gr-refcard .src{display:flex;align-items:center;gap:7px;font-size:11px;color:var(--fg2);} .gr-refcard .snip{font-size:12.5px;color:var(--muted);margin-top:5px;line-height:1.4;} .gr-refcard .snip em{background:var(--coral-dim);color:var(--coral);font-style:normal;padding:0 2px;border-radius:3px;}`
**Properties list:** `.gr-proplist .ph{font-family:var(--mono);font-size:9.5px;letter-spacing:.10em;text-transform:uppercase;color:var(--faint);padding:4px 2px 7px;} .gr-prow{display:grid;grid-template-columns:18px 84px 1fr;align-items:center;gap:8px;padding:6px 7px;border-radius:7px;} .gr-prow:hover{background:var(--raised);} .gr-prow .chord{font-family:var(--mono);font-size:9.5px;text-align:center;color:var(--subtle);background:var(--surface);border:1px solid var(--line);border-radius:4px;padding:2px 0;} .gr-prow .k{font-family:var(--mono);font-size:11px;color:var(--subtle);} .gr-prow .v{font-family:var(--mono);font-size:11px;color:var(--fg2);}`
**Inbox:** `.gr-chipbar{display:flex;align-items:center;gap:7px;padding:11px 18px;border-bottom:1px solid var(--line);flex-wrap:wrap;}` (chips = foundation `GrChip`) · `.gr-inbox-body{flex:1;overflow:auto;padding:12px 18px;display:flex;flex-direction:column;gap:8px;} .gr-icard{display:flex;align-items:flex-start;gap:12px;padding:13px 14px;border-radius:11px;background:var(--surface);border:1px solid var(--line);transition:border-color .14s;} .gr-icard:hover{border-color:var(--line-2);} .gr-icard.sel{background:var(--raised);border-color:var(--coral-line);} .gr-icard .src{width:30px;height:30px;border-radius:8px;display:grid;place-items:center;background:var(--raised-2);color:var(--subtle);flex-shrink:0;} .gr-icard-body{flex:1;min-width:0;} .gr-icard .txt{font-size:14px;color:var(--fg);line-height:1.45;} .gr-icard .meta{display:flex;align-items:center;gap:10px;margin-top:7px;font-family:var(--mono);font-size:10.5px;color:var(--faint);} .gr-icard .meta .pill{padding:2px 7px;border-radius:5px;background:var(--raised-2);color:var(--subtle);} .gr-icard-acts{display:flex;align-items:center;gap:4px;flex-shrink:0;} .gr-iact{width:28px;height:28px;display:grid;place-items:center;border-radius:7px;color:var(--subtle);cursor:pointer;border:1px solid transparent;} .gr-iact:hover{background:var(--raised-2);color:var(--fg);border-color:var(--line);} .gr-iact.go:hover{color:var(--coral);}`
**Agenda:** `.gr-agenda{flex:1;overflow:hidden;display:flex;flex-direction:column;} .gr-agrid{flex:1;display:grid;grid-template-columns:56px repeat(5,1fr);overflow:auto;} .gr-ag-col{border-right:1px solid var(--line);position:relative;min-width:0;} .gr-ag-colhdr{height:46px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:1px;border-bottom:1px solid var(--line);border-right:1px solid var(--line);} .gr-ag-colhdr .dw{font-family:var(--mono);font-size:9.5px;letter-spacing:.08em;text-transform:uppercase;color:var(--faint);} .gr-ag-colhdr .dn{font-size:15px;font-weight:600;color:var(--fg);} .gr-ag-colhdr.today .dn{color:var(--coral);} .gr-ag-time{height:62px;font-family:var(--mono);font-size:9.5px;color:var(--faint);text-align:right;padding:2px 7px 0 0;border-top:1px solid var(--line);} .gr-ag-slot{height:62px;border-top:1px solid var(--line);} .gr-ev{position:absolute;left:5px;right:5px;border-radius:7px;padding:6px 8px;overflow:hidden;border:1px solid transparent;cursor:pointer;} .gr-ev .et{font-size:11.5px;font-weight:550;line-height:1.25;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;} .gr-ev .em{font-family:var(--mono);font-size:9.5px;opacity:.8;margin-top:1px;} .gr-ev.event{background:rgba(98,184,206,.16);border-color:rgba(98,184,206,.34);color:var(--event);} .gr-ev.task{background:rgba(232,105,127,.15);border-color:rgba(232,105,127,.34);color:var(--task);} .gr-now{position:absolute;left:0;right:0;height:0;border-top:1.5px solid var(--coral);z-index:3;} .gr-now::before{content:"";position:absolute;left:-3px;top:-3.5px;width:7px;height:7px;border-radius:50%;background:var(--coral);}` Layout constants: `GR_HOURS=[8..16]`, `GR_SLOT=62`, `GR_START=8`; event `top=(h-8)*62+(m/60)*62`, `height=dur*62-5`.

---

## Part A — Web views

### Task A1: Graphite CodeMirror theme + decoration styles (makes the reused editor look Graphite)

**Files:**
- Read: `web/src/lib/components/BlockOutliner.svelte`, `web/src/lib/cm-decorations.ts` (the decoration class names — `cm-*`/tag/link/wikilink/property classes)
- Create: `web/src/lib/graphite/editor/graphite-cm-theme.ts` (a CodeMirror `Extension`: `EditorView.theme({...})` mapping editor bg/fg/caret/selection/font to the tokens) + matching decoration CSS in `web/src/lib/graphite/editor/graphite-editor.css`

- [ ] **Step 1** — READ `cm-decorations.ts` to list the exact decoration class names BlockOutliner emits (inline `#tag` chip, trailing tag chips, `[[wikilink]]`, `@mention`, property key/value, hidden property lines). READ how BlockOutliner accepts theme extensions (does it take an `extensions` prop, or import a theme module? If no injection point, the Graphite styles go in `graphite-editor.css` imported by the `.gr-root` tree and target the `cm-*` classes — confirm which).
- [ ] **Step 2** — Write `graphite-cm-theme.ts`: an `EditorView.theme` (and/or `HighlightStyle`) using `var(--bg)`/`var(--fg)`/`var(--coral)`/`var(--sans)` etc., scoped so it only affects editors under `.gr-root`. Write `graphite-editor.css` styling the decoration classes to match the mockup's `.gr-tagchip`/`.gr-link`/`.gr-mention`/`.gr-pchip` (colors/radii above) and the block bullet/selection treatment. Import the CSS from `web/src/routes/g/+layout.svelte`.
- [ ] **Step 3** — Verify svelte-check clean; commit `feat(graphite-web): Graphite CodeMirror theme + decoration styles`.

### Task A2: Daily journal in the pane (reuse JournalView)

**Files:**
- Read: `JournalView` (find it — likely `web/src/lib/components/.../JournalView.svelte`), `BufferShell`/`NoteRenderer`, `lib/buffer/state.svelte.ts`
- Create: `web/src/lib/graphite/views/GrDaily.svelte`
- Modify: `web/src/lib/graphite/shell/GrPane.svelte` (render daily content)

- [ ] **Step 1** — READ `JournalView` (props `{anchorDate}`, how it stacks days + lazy-creates + wires BlockOutliner) and how it's normally mounted. Decide reuse: wrap `JournalView` directly (preferred — it carries the day-stack + lazy-create + cross-day nav), or replicate its thin mount around `BlockOutliner`. The Graphite block styling comes from A1's CM theme, so reusing JournalView yields Graphite-looking dailies.
- [ ] **Step 2** — Write `GrDaily.svelte` wrapping `JournalView` (pass today's `anchorDate`), inside a `.gr-outline` scroll container. The day-divider styling: if JournalView renders its own day headers, ensure A1's CSS / a wrapper restyles them to `.gr-dayhdr`; if not, GrDaily renders `.gr-dayhdr` between JournalView's per-day outliners (read JournalView to see which).
- [ ] **Step 3** — Wire `GrPane`/`GraphiteShell`: the default focused buffer renders `GrDaily`. Verify svelte-check clean; commit `feat(graphite-web): daily journal in pane (reuse JournalView)`.

### Task A3: Page/project outliner + linked refs + properties (reuse BlockOutliner)

**Files:**
- Create: `web/src/lib/graphite/views/GrPage.svelte`
- Read: how a `page` buffer fetches its note (`getNote` query) + `api.getBacklinks`/links

- [ ] **Step 1** — READ how a page buffer is rendered today (BufferShell → BlockOutliner for note kind; backlinks via `api` / a query). Confirm `BlockOutliner` props (`noteId, body, frontmatter, onContentChange, onCancelAndFlush, onfocusedblockchange, paneId`).
- [ ] **Step 2** — Write `GrPage.svelte`: a `.gr-pane focus` with `GrPane` head (title + `GrTypeTag` + meta), `.gr-outline` body hosting `BlockOutliner` (fetch the note via the existing query pattern; save via `onContentChange`→`api.updateNote`, mirroring BufferShell), and a `.gr-pane side` linked-references pane (`.gr-refcard`s from backlinks) + `.gr-proplist` (page properties). Reuse the data fetch pattern from BufferShell — read it, mirror it.
- [ ] **Step 3** — Wire GrPane to render `GrPage` for page buffers + open-page nav (`openPageInFocused`). Verify; commit `feat(graphite-web): page outliner + linked refs (reuse BlockOutliner)`.

### Task A4: GrInbox (new Graphite view over the data layer)

**Files:**
- Create: `web/src/lib/graphite/views/GrInbox.svelte`
- Read: the existing Inbox ambient renderer + `chipsFromDsl`/`dslFromChips` + `api.executeQuery`

- [ ] **Step 1** — READ the existing Inbox renderer + the DSL chip helpers + `api.executeQuery(dsl, group?, sort?)`.
- [ ] **Step 2** — Write `GrInbox.svelte`: `.gr-pane focus` head ("Inbox" + a "Process all" `GrButton cta`), `.gr-chipbar` of `GrChip`s (filters from `chipsFromDsl`, counts), `.gr-inbox-body` of `.gr-icard`s (src icon, text, meta pills, the 4 `.gr-iact` actions file/tag/snooze/open) bound to `executeQuery` results. j/k nav + actions wired to the existing mutations.
- [ ] **Step 3** — Verify; commit `feat(graphite-web): GrInbox triage`.

### Task A5: GrAgenda (new Graphite view over getAgenda)

**Files:**
- Create: `web/src/lib/graphite/views/GrAgenda.svelte`

- [ ] **Step 1** — READ `api.getAgenda(from, to, includeDone?)` + the `AgendaRow` shape + the existing Agenda renderer.
- [ ] **Step 2** — Write `GrAgenda.svelte`: the `.gr-agrid` time grid (56px gutter + 5 day cols, `GR_HOURS`/`GR_SLOT=62`/`GR_START=8`), type-colored `.gr-ev` blocks positioned by the `top`/`height` formula, the `.gr-now` indicator on today, column headers `.gr-ag-colhdr`. Bind to `getAgenda` for the visible week; vim nav (h/l week, t today) wired to a week-anchor `$state`.
- [ ] **Step 3** — Verify; commit `feat(graphite-web): GrAgenda week`.

### Task A6: Wire GrPane by buffer kind + web views gate

**Files:**
- Modify: `web/src/lib/graphite/shell/GraphiteShell.svelte` / `GrPane.svelte`

- [ ] **Step 1** — In the shell, render the focused buffer's content by kind: daily/journal→`GrDaily`, page→`GrPage`, inbox→`GrInbox`, agenda→`GrAgenda`, else the placeholder. Use the buffer store's focused-buffer accessor (read `lib/buffer/state.svelte.ts`). Wire the rail/leader/⌘K nav to open these (e.g. "go to today"→daily, open page→page, the rail Today/Tasks→inbox/agenda).
- [ ] **Step 2** — GATE: `cd web && pnpm exec svelte-check --threshold error 2>&1 | tail -20` — no NEW errors in graphite. Commit `feat(graphite-web): GrPane renders views by buffer kind`.

---

## Part B — iOS views

### Task B1: GrDailyView (reuse BlockRow + MosaicService.todayBlocks)

**Files:**
- Read: `Sources/Views/DailyView.swift`, `Components/BlockRow.swift`, `Data/MosaicService.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/Views/GrDailyView.swift`
- Modify: `Sources/Graphite/Shell/GrTabPlaceholder.swift` (or GrAppShell) — render GrDailyView for the daily tab

- [ ] **Step 1** — READ `DailyView.swift` (how it lists `todayBlocks`/`yesterdayBlocks`, lazy-create, the BlockRow callbacks: `onToggleTask/onCommitEdit/onTextChanged/onIndent/onCycleStatus/onSetProperties`) + `BlockRow.swift` (the inline editor + keyboard toolbar). Confirm the `MockMosaicService` daily methods (`editTodayBlock/appendTodayBlock/toggleTask/...`).
- [ ] **Step 2** — Write `GrDailyView.swift`: a Graphite-themed day list (GrHeader + day-divider sections + `BlockRow`s bound to `mosaic.todayBlocks`/`yesterdayBlocks`), reusing BlockRow's editing callbacks → the same MosaicService mutations. Theme via `@Environment(\.theme)` (`.graphite`). Reuse the foundation primitives (GrTypeDot bullets, GrChip props).
- [ ] **Step 3** — Render GrDailyView in GrAppShell's `.daily` tab (replace the placeholder). Build (`xcodebuild ... | tail`) → BUILD SUCCEEDED. Commit `feat(graphite-ios): GrDailyView (reuse BlockRow + MosaicService)`.

### Task B2: GrPageView (reuse BlockRow + page load)

**Files:**
- Read: `Sources/Views/PageView.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/Views/GrPageView.swift`

- [ ] **Step 1** — READ `PageView.swift` (page load via `mosaic.loadPage(id:)` → `loadedPageBlocks`/`loadedBacklinks`/`loadedLinks`, BlockRow editing, wiki-link nav via `TeselaLink`/`openURL`).
- [ ] **Step 2** — Write `GrPageView.swift`: GrHeader (title + GrTypeTag) + a BlockRow list bound to `loadedPageBlocks` + a linked-refs section. Reuse the load + nav. Wire wiki-link taps + `PageStack`.
- [ ] **Step 3** — Wire library/daily nav to push GrPageView; build SUCCEEDED; commit `feat(graphite-ios): GrPageView`.

### Task B3: GrLibraryView (workspace widget grid)

**Files:**
- Read: `Sources/Views/LibraryView.swift`, `Views/Ambients/WorkspaceGridView.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/Views/GrLibraryView.swift`

- [ ] **Step 1** — READ LibraryView (pages/tags/pinned/recent binding + filter) + the mobile `grm-*` Library grid design.
- [ ] **Step 2** — Write `GrLibraryView.swift`: the Graphite workspace widget grid (`.grm-acard`-style cards) over `mosaic.pages`/`pinned`/`recent`/`tags`, GrHeader, tap → push GrPageView.
- [ ] **Step 3** — Render in the `.library` tab; build SUCCEEDED; commit `feat(graphite-ios): GrLibraryView grid`.

### Task B4: GrAgendaView + Task B5: GrInboxView

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/Views/GrAgendaView.swift`, `GrInboxView.swift`
- Read: `Sources/Views/AgendaView.swift`, `InboxView.swift`, `Data/AgendaRow.swift`, `Data/InboxChips.swift`

- [ ] **Step 1 (B4)** — READ AgendaView + `fetchAgenda(from:to:includeDone:)` + AgendaRow. Write `GrAgendaView.swift`: Graphite agenda list/grid bound to `fetchAgenda`, type-colored rows, reschedule via DateInputSheet. Render in `.agenda` tab. Build SUCCEEDED; commit.
- [ ] **Step 2 (B5)** — READ InboxView + `InboxChips` (chipsFromDsl/dslFromChips) + `executeQuery`. Write `GrInboxView.swift`: Graphite chip bar (GrChip) + `.grm-icard`-style cards over `executeQuery`, triage actions → MosaicService mutations, `@AppStorage` filter persistence. Render in `.inbox` tab. Build SUCCEEDED; commit.

### Task B6: iOS views gate

- [ ] **Step 1** — `cd app/Tesela-iOS && xcodegen generate && xcodebuild -scheme Tesela -sdk iphonesimulator -configuration Debug -destination 'generic/platform=iOS Simulator' build 2>&1 | tail -20` → BUILD SUCCEEDED. (SourceKit false-positives ignored.)
- [ ] **Step 2** — Confirm GrAppShell tabs all render their Graphite views (no remaining placeholders for daily/page/library/agenda/inbox). Commit any fixes.

---

## Done — exit criteria

- Web `/g`: daily journal edits (reused JournalView/BlockOutliner, Graphite-themed via the CM theme), pages open + edit + show linked refs, inbox triages, agenda renders the week; ⌘K search works. svelte-check clean.
- iOS GrAppShell: all 4 tabs render real Graphite views over MosaicService (daily edits via BlockRow, pages, library grid, agenda, inbox); native search. xcodebuild SUCCEEDED.
- Editing engines + data layer 100% reused; only presentation new/re-themed. Old UI untouched.
- **Then:** make testable (wire entries / restart backend) + self-QA (Playwright web, sim iOS) → parity check → cutover.

## Self-review

- Spec daily-driver views (daily/page/inbox/agenda/search, both platforms) → A1-A6 + B1-B6; search reused (palette/native). ✓
- Reuse-vs-rebuild: editors (BlockOutliner/JournalView/BlockRow) + data reused; CM theme re-skins, doesn't rewrite; new components only for non-editor views. Every editor task starts with a READ step. ✓
- Testable core prioritized (A1-A3, B1-B2). ✓
- Old UI untouched; new files under `lib/graphite/`/`Sources/Graphite/`. ✓
- CSS spec-derived (verbatim); editor reuse codebase-derived (read+mirror). ✓

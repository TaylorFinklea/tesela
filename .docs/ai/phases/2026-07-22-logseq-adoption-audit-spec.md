# Stock Logseq Adoption Audit Spec

> **For agentic workers:** Execute one phase at a time. Device checks are `user-verify` gates on physical hardware; never infer them from source or simulator behavior. Record final evidence in the matching report, not in the production graph.

**Goal:** Decide whether stock Logseq DB plus supported desktop configuration/plugins can be Taylor's durable primary system without opening a speculative fork.

**Direction:** Stock Logseq DB plus extensions is the first trial. If that interaction model fails, the next trial is Emacs/Org plus a mobile companion. Only if the Emacs trial also fails should a rebranded Logseq fork become the new Tesela.

## Global constraints

- Do not rebrand Logseq, migrate to a new fork, or build another mobile notes app during the audit.
- Do not delete Tesela source, history, or unfinished user work.
- Do not copy a live SQLite file. Use Logseq's own backup/export/import paths.
- Never patch Logseq DB schema, validation, sync, conflict resolution, or storage semantics.
- Run destructive and divergent-edit exercises only in disposable graphs.
- iPhone and iPad are separate gates; desktop keyboard requirements apply only on macOS.
- Whiteboard editing is deferred and excluded. Mobile PDFs need inline viewing, not annotation.
- A rare explicit conflict is acceptable; a silent lost edit is not.
- Parakeet may use one deliberate external-app, keyboard, Shortcut, or clipboard handoff.
- The 14-day clock begins only after the recovery gate passes.

## Phase 1 — Recovery envelope

1. In the stock desktop app, open the production DB graph and use **Three dots → Export graph** to produce:
   - **SQLite DB and assets (.zip)**
   - **EDN file**
2. Import each artifact into its own newly named disposable graph. Never replace or delete the production graph.
3. Verify a fixed 20-item sample:
   - five blocks with links, block references, or backlinks;
   - five tasks spanning status, scheduled/deadline dates, recurrence, and priority;
   - five typed properties, tags, queries, or views;
   - five images, PDFs, or other assets in the SQLite+assets restore.
4. Verify the EDN import for semantic graph data; EDN is not required to carry asset bytes and is not the sole backup.
5. Record export time, import time, manual repair count, and every unexplained difference.

**Pass:** both imports open; SQLite+assets preserves the full sampled graph and assets; EDN preserves sampled semantic data; zero unexplained loss; zero manual repair.

**Fail:** missing sampled content, broken references/properties, unavailable assets in the SQLite+assets restore, or any repair needed to make the graph usable.

## Phase 2 — Durability and conflict matrix

Use one disposable synced DB graph on macOS, iPhone, and iPad. Seed unique sentinel strings before each case and capture the pre-sync text on both devices.

1. Edit the same block while both devices are offline.
2. Edit different blocks on the same page while offline.
3. Rename a page on one device while editing its content on another.
4. Delete a block on one device while editing it on another.
5. Paste a large multi-block payload while another device has delayed sync.

Reconnect, wait for each device to report synchronized state, then reopen the graph on all three devices.

**Pass:** every sentinel survives in converged content or in an explicit conflict/version/recovery artifact that preserves both authored versions. Manual merge is acceptable when the conflict is surfaced.

**Fail / kill switch:** any authored sentinel disappears without an explicit recoverable artifact. Stop the audit; do not treat a fork as a data-plane remedy.

## Phase 3 — Rich-mobile capability matrix

Run every row separately on physical iPhone and iPad:

1. Search the graph and open an arbitrary note.
2. Follow a page link and a block reference; open backlinks/references.
3. Edit existing text and create/move nested blocks.
4. Create, reschedule, complete, and reopen a recurring task.
5. Add and edit a typed property; open a saved query/view.
6. Render an inline image and open/view a PDF inside Logseq.
7. Render and navigate a block-reference embed and representative rich blocks.
8. Capture text through the share path, a photo, and an audio recording.

Classify each result as `pass`, `acceptable limitation`, `deferrable`, or `blocking`. Whiteboards are not a row.

**Pass:** no `blocking` row on either device.

**Fallback trigger:** one blocking mobile row fails the stock trial and triggers the Emacs/Org plus mobile-companion pilot. It does not authorize a Logseq fork or any DB/sync/schema change.

## Phase 4 — Desktop keyboard and command audit

Pin the DB-compatible Vim Shortcuts plugin, then map these 20 daily actions using **Settings → Keymap**, the supported Commands API, or the plugin:

1. global search;
2. command palette;
3. today's journal;
4. arbitrary page open;
5. navigation back/forward;
6. left/right sidebar toggle and focus;
7. favorites/recent navigation;
8. block focus;
9. insert/edit mode;
10. normal-mode block/text movement;
11. block selection;
12. block move/reorder;
13. indent/outdent;
14. collapse/expand;
15. create and follow a reference;
16. set/cycle task status;
17. schedule or set a deadline;
18. add/edit a property;
19. open and operate a query/view;
20. open an inline PDF.

Inventory desired literal Emacs keys separately: `C-n`, `C-p`, `C-f`, `C-b`, `C-a`, `C-e`, `M-f`, `M-b`, `C-k`, `C-y`, `C-s`, and `C-r`. Classify each as native, configurable, plugin-reachable, accepted approximation, or unreachable.

Use normal Logseq work for five consecutive days. Log every forced-mouse action, its surface, and daily frequency. Timebox investigation of each gap to four focused hours.

**Pass:** zero daily-critical forced-mouse actions; at least 90% of all named actions are mouse-free; every remaining chord difference is explicitly accepted.

**Fallback trigger:** a named daily-critical action remains unreachable after the supported config/API/plugin timebox and triggers the Emacs/Org plus mobile-companion pilot.

## Phase 5 — Parakeet capture

On each of iPhone and iPad, run ten captures through Spokenly's local Parakeet custom keyboard or Shortcut/clipboard flow into an intended Logseq block.

**Pass:** 10/10 captures land in the intended block as searchable text with no more than one deliberate handoff. Record latency and correction count, but transcription perfection is not the integration gate.

**Fail:** focus loss, wrong-block insertion, or more than one required handoff recurs. Try the alternate keyboard versus Shortcut/clipboard path before considering code.

## Phase 6 — Adjudication

### Confirm stock Logseq

Confirm option A when phases 1–5 pass and customization remains below five hours per week during the audit.

### Trigger the Emacs/Org plus mobile-companion pilot

Any blocking failure in phases 1–5 moves the next experiment to Emacs/Org; it does not open a Logseq fork. Create a separate pilot spec and bead before changing canonical data.

The Emacs pilot starts with existing iOS clients and proven file-sync/versioning options. It must test the same task, graph-navigation, rich-content, mobile, and no-silent-loss requirements rather than presuming Org can meet them. A purpose-built thin Org mobile companion is a separate design decision only if existing clients leave mobile as the sole blocking gap.

### Consider a Logseq fork as the new Tesela

Fork design begins only after the Emacs/mobile pilot fails a named non-negotiable requirement and Logseq remains the closest product model. That later decision requires its own spec covering upstream tracking, rebranding/trademark review, AGPL distribution, desktop and iOS builds, patch boundaries, and maintenance budget.

No fork implementation or patch queue starts during this audit. DB schema, validation, storage, sync, and conflict semantics remain upstream-owned unless Taylor explicitly approves a later architecture decision.

## Final evidence

Create `.docs/ai/phases/2026-07-22-logseq-adoption-audit-report.md` only after the audit. Include:

- recovery sample matrix and artifact types, without graph content;
- five conflict outcomes and whether both versions remained recoverable;
- separate iPhone and iPad capability matrices;
- 20-action keyboard map, Emacs-key inventory, and forced-mouse log counts;
- Parakeet results by device and path;
- actual maintenance time;
- final `stock`, `emacs-mobile pilot`, or `fork-design candidate` verdict with the applicable threshold cited.

Durable strategy review: `http://127.0.0.1:7420/r/tesela/2026-07-22-logseq-path-review`.

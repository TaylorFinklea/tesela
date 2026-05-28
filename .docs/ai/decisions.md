# Architecture Decision Records

Concise log of non-obvious decisions. Newest first.

---

### 2026-05-27 — Migrate sync data layer to Loro; relay protocol stays as-is

**Decision:** Replace the hand-rolled `tesela_sync::engine::sqlite_engine::SqliteEngine` with a Loro-backed implementation. The wire format (`SyncEnvelope`, AEAD-sealed `ciphertext`, HKDF per-group keys, pairing flow, Cloudflare Worker port) is unchanged — Loro updates slot into the existing opaque `ciphertext` field. Migration boundary: `engine/sqlite_engine.rs` + the FFI surface in `tesela-sync-ffi`.

**Why:** Taylor wants Savanne to be a real collaborator in Tesela, not just a viewer. That makes multi-user-within-a-mosaic an explicit product goal. The hand-rolled engine was designed for eventual sync with one writer at a time; we've been treating concurrent edits as the bug case but they're now the everyday case. Every recent bug class (lost-update on whole-file PUT, duplicate-block storm from per-save bid churn, "fella vs dude" race on PUT diffs) is a variant of the same root cause: an eventual-sync engine being driven as if it were a real-time-collab system. Loro is the system designed for the case we're actually in.

Bonus capabilities that fall out for free (not speculative):
- Cursor presence — see where Savanne is editing in the same note
- Intra-block character-level concurrent edits (current granularity is "the block")
- Replayable history with per-author attribution
- Time-travel ("show me this note as of last Tuesday") via Loro's snapshot/version graph

**Triangulation:** Triangulated across Claude Code (in-repo, has visibility into the existing engine's investment depth) and Claude Desktop (independent reviewer). Initial split was Claude Code at "Phase 7 if triggered", Claude Desktop at "step 2 of redesign". Converged on "migrate now" after the Savanne-collaboration question made multi-user concurrency definite rather than hypothetical.

**Trade-off:** 8–10 calendar weeks at 10–15 hr/week. Means roughly nothing else on Taylor's portfolio (Larkline, NebularNews, Joji, SimmerSmith, Finclade, Growjo, gardening, Telaradio) moves forward during that window. Patch path was the alternative — ~1–2 more weeks of work, no bonus features, no support for multi-user, and continued bug tail.

**Execution pattern:** Dual-write behind a feature flag. `SyncEngine` trait already exists; wrapper fans-out to both `SqliteEngine` (current) and `LoroEngine` (new). Compare outputs each tick. When divergence is zero for a week of normal usage, flip the flag. One device at a time, starting with iOS (highest sync pain, smallest surface). Keep rollback path until at least a week of clean dual-write convergence. HLC must be shared between both engines so timestamps don't diverge on identity alone.

**Gating spike (before committing weeks of work):** UniFFI compatibility with loro-swift; snapshot size vs current SQLite oplog; apply-changes latency on a representative batch; move-op semantics parity; oplog → Loro doc one-way import path. Spec at `.docs/ai/phases/2026-05-27-loro-spike-spec.md`. If any item reveals a structural problem, fall back to patch-then-migrate-later with a hard calendar deadline of Q1 2027.

**Supersedes:** [project_sync_redesign_plan](../../.claude/projects/-Users-tfinklea-git-tesela/memory/project_sync_redesign_plan.md)'s "Loro at Phase 7 if triggered" position. Loro is now Phase 4 in the 7-step plan; Phase 4 (APNs) and Phase 5 (CF Worker deploy) slide later because Loro changes the payload shape.

---

### 2026-05-21 — Workhorse/spark accent split; the spark is a theme, not a rule

**Decision:** `--accent-primary` is an earthy terracotta (`#E07A5F`) — the everyday accent for links, bullets, selection. The neon coral (`#FB5950`) is a separate `--accent-spark` token. `--accent-spark` defaults to `var(--accent-primary)`, so standard themes show no neon; only the opt-in **Prism Spark** theme overrides it (`[data-theme="prism-spark"] { --accent-spark: #fb5950 }`). iOS mirrors this with a `Theme.accentSpark` computed property keyed on `id == .prismSpark`.

**Why:** The logo coral at full saturation, used as the app-wide accent, read as harsh — a hot hue hit hundreds of times per screen is noise, not accent. Splitting a calm workhorse from a rare neon spark is the standard hero-vs-workhorse colour split. Making the spark a *theme toggle* rather than a hardcoded set of spots means the two variants can't drift apart — Prism Spark is definitionally "Prism + one token".

**Trade-off:** Three Prism themes now (Prism, Prism Spark, Prism Light) where one might do. But the spark is a genuine taste call, and a one-token override theme is nearly free to maintain. The logo keeps the neon `#FB5950` — a logo is a stamp seen briefly, the app a surface stared at, so they legitimately use the same hue at different saturations.

---

### 2026-05-21 — The v4/v5 chrome token layer aliases the role tokens

**Decision:** `web/src/lib/v4/tokens.css` `--v4-*` tokens (`--v4-bg`, `--v4-ink*`, `--v4-surface*`, `--v4-hair*`, `--v4-accent*`) now alias the role tokens (`var(--bg)`, `var(--fg-*)`, …) instead of being hardcoded literals. The `.v4-root`'s old `--accent-primary: var(--v4-accent)` override is removed.

**Why:** The live app's chrome (the `/v4` route — `BufferShell`, `Sidebar`, `StatusLine`, the `.v4-root` shell) reads `--v4-*`, which were hardcoded dark literals (`--v4-bg: #131521`, near-white ink) decoupled from the 30-theme role-token system. So *no theme changed the chrome*, and light themes appeared completely broken (background frozen dark). The `--v9-*` layer already aliased the role tokens; `--v4-*` should have too.

**Trade-off:** The v4 ink ramp had six levels; the role contract has four, so the two finest gradations double up (minor loss of text-hierarchy nuance). The dark Prism chrome's surface shifts from the old `#131521` to the real Prism `#23252F` — a visible change, but a correct one: the chrome had never actually used the designed Prism palette.

---

### 2026-05-21 — Default theme rebranded to warm-dark "Prism"; light variant ships

**Decision:** The app's default theme is a new warm-dark **Prism** palette derived from the locked logo — slate surfaces (`#23252F`→`#3D405B`), cream text (`#F4F1DE`), coral brand accent (`#FB5950`), sage secondary (`#81B29A`) — replacing the placeholder indigo `#7b8cff`. A **Prism Light** theme (cream / slate / deepened-coral `#DD4A3D`) ships alongside it. The cross-platform theme id `prism-indigo` becomes `prism`; the web FOUC default and the iOS `@AppStorage` default both move to `prism`.

**Why:** The logo was finalized as a two-tone mosaic mark; the app's chrome should agree with its own logo. The indigo `#7b8cff` was a leftover from the v4 proto mockup with no logo behind it. Keeping Prism *warm-dark* (rather than a straight light translation of the cream logo) preserves Tesela's dark-mode-first stance while every surface and the accent now harmonize with the mark.

**Trade-off:** (1) `accent-secondary` (sage `#81B29A`) is an extrapolation — a two-tone logo supplies only one accent, but the role-token contract needs a navigational secondary; sage is the natural completion of the logo's source terracotta palette and is chosen to recede rather than compete with coral. (2) Shipping Prism Light **supersedes** the earlier "always dark on first ship, light themes land later" decision (iOS design-followup #10); `preferredColorScheme` now tracks `Theme.isLight`. (3) The Swift enum case was renamed `prismIndigo`→`prism`; any persisted `"prism-indigo"` preference falls through to the new `prism` default — acceptable, since those users were on the default anyway. (4) The legacy `.v4-root` route keeps its own `#131521` surface and ink ramp; only its accent tokens were repointed to coral/sage.

---

### 2026-05-20 — `tesela-server` bind is config-driven; default stays loopback

**Decision:** Add a `[server] bind` key (`ServerConfig` in `tesela-core`). `tesela-server` resolves its bind address as `TESELA_SERVER_BIND` env → `[server].bind` in the global config → `127.0.0.1:7474`. The compiled default stays loopback; LAN exposure is opt-in per machine via config. Taylor's `~/.config/tesela/config.toml` sets `0.0.0.0:7474`.

**Why:** iOS↔desktop sync was impossible from a physical device because the server only ever bound loopback — reachable from the iOS simulator (shared host network) but not a real iPhone. The bind *must* live in config, not just the env var: `/server/restart` (used by iOS mosaic-switching) re-execs the binary without inheriting the environment, so an env-only bind would silently revert to loopback after every switch.

**Trade-off:** `0.0.0.0` exposes the server — which has no auth by default — to every device on the LAN. Acceptable for a single-user daily driver on a trusted network (the same posture as Syncthing/Logseq sync), but a coffee-shop Wi-Fi is genuinely unsafe until an auth token is enforced. Keeping the *compiled* default at loopback means only machines that explicitly opt in are exposed. `MosaicProfile.authToken` exists but is not yet checked server-side.

**Follow-up (same day):** even with `0.0.0.0`, the iPhone still couldn't reach the Mac's plain LAN IP — both devices are on the user's Tailscale tailnet, which advertises `10.x` subnet routes, so the phone routed the LAN subnet into the Tailscale tunnel. The reliable address is the Tailscale IP (`100.64.0.0/10`). `first_lan_ipv4()` now prefers a Tailscale CGNAT address when one exists, so pairing codes advertise the Tailscale IP automatically; it falls back to a plain LAN IP otherwise. Lesson: for a multi-device personal setup the overlay-network address is more reliable than the physical LAN IP.

---

### 2026-03-30 — Apple-first, web later (platform strategy)

**Decision:** SwiftUI/AppKit is the primary GUI. Use SF Symbols for icons. A Tauri/web app can be built later sharing the Rust backend API, with its own icon set (Tabler/Lucide) mapped from the same frontmatter `icon` field.

**Why:** Taylor is the sole user on macOS. Native AppKit gives the best keyboard-first editing experience. The Rust backend already runs cross-platform. Rewriting in a shared UI framework now would kill momentum for hypothetical users.

**Trade-off:** Two GUI codebases eventually. But the server API is the shared contract, and different icon libraries per platform is normal (like VS Code vs native IDEs).

---

### 2026-03-27 — Keyboard-navigable select popover (SelectListView)

**Decision:** Replace NSButton-based select popovers with a custom NSView subclass that handles keyDown (arrow/j/k, Enter, Escape) and mouse clicks.

**Why:** Mouse-only popovers broke the keyboard-first UX promise. NSMenu alternatives had target deallocation issues.

**Trade-off:** More custom AppKit code to maintain, but consistent with the Vim-everywhere philosophy.

---

### 2026-03-27 — Preserve caller frontmatter in store.create()

**Decision:** If content passed to `FsNoteStore::create()` already starts with `---`, write it as-is instead of prepending auto-generated frontmatter.

**Why:** Property and Tag pages created from the SwiftUI app include custom frontmatter (type, value_type, choices). The old behavior doubled the frontmatter block and lost those fields.

**Trade-off:** Callers that pass frontmatter are responsible for including `created` timestamps themselves.

---

### 2026-03-25 — Properties and Tags as pages, not config files

**Decision:** Adopt Logseq DB model — Tags, Properties, and Values are all markdown pages with YAML frontmatter. No more `types.toml`.

**Why:** "Everything is a page" aligns with Tesela's file-based philosophy. Users can browse, edit, and link to type definitions. Enables property inheritance through `extends` chains.

**Trade-off:** Server must understand Property/Tag page semantics. More complex indexing. But files remain the single source of truth.

---

### 2026-03-20 — Database-first architecture shift

**Decision:** TUI becomes an API client. Local tesela-server runs always. Central server planned for sync.

**Why:** SwiftUI app already uses REST API. Having TUI also use the API means one code path for all clients. Enables future multi-device sync.

**Trade-off:** TUI loses offline-only simplicity. But the server is local, so it's effectively the same.

---

### 2026-03-15 — Custom NSTextView outliner, not embedded Neovim

**Decision:** Build the block editor from scratch using one NSTextView per block inside an OutlinerView (NSView), wrapped in a single NSViewRepresentable.

**Why:** Embedded Neovim can't do block-aware motions (j/k between blocks, dd deletes block, >> indents hierarchy). WKWebView+TipTap adds web complexity. Native AppKit gives full control over Vim integration.

**Trade-off:** Significant upfront work for the editor. But it's the right long-term investment for keyboard-first UX.

---

### 2026-05-19 — iOS bottom chrome: native TabView with `Tab(role: .search)`, not a custom HStack

**Decision:** Use SwiftUI's `TabView` with `Tab(_:systemImage:value:)` for place-tabs (Daily/Inbox/Library) and `Tab(value:role:.search)` for the search slot. The system manages all visual chrome — pill geometry, Liquid Glass material, safe-area positioning, scroll-edge effects, the trailing-pinned search circle, accessibility. Capture stays a sheet trigger from the TopBar (no bottom-chrome slot).

**Why:** A hand-rolled `BottomChrome` HStack of `glassEffect` shapes was nominally correct but visually wrong — wrong height, wrong baseline above the home indicator, wrong selection treatment, and the three glass shapes refracted inconsistently because each had its own sampling region. Reference apps (Simmersmith, Seedkeep, Joji) all use plain native `TabView` and look correct effortlessly. Phone/Mail iOS 26's trailing search-circle look is `Tab(role: .search)`, which the system pins as a standalone Liquid Glass circle separate from the labeled pill.

**Trade-off:** No way to put a second standalone glass shape (e.g., a capture FAB) next to the search circle without abandoning the native chrome entirely. iOS 26 has only `.search` as a `TabRole`; `tabViewBottomAccessory` always renders as a row *above* the tab bar at rest on iPhone (docs: "the accessory appears above [the tab bar at normal size]; inline only when the tab bar is collapsed"). A custom three-shape `BottomChrome` was prototyped — it works but doesn't visually match Apple's chrome, so it was reverted. Capture lives in `DailyTopBar`'s icon row via the new `\.openCapture` environment value.

---

### 2026-05-20 — One process-wide `EKEventStore`, not one per operation

**Decision:** All EventKit access in `reminders/darwin.rs` goes through a single lazily-created `EKEventStore` held in a module `OnceLock` (`shared_event_store()`). Push, pull, and the access request previously each constructed their own.

**Why:** EventKit caps how many `EKEventStore` instances a process may hold. Each `sync_all` built four (the access request runs inside both `pull_all` and `push_all`), so auto-sync every 5 minutes exhausted the cap within ~an hour — EventKit then rejected every call with "too many EKEventStore instances. Use fewer event stores". A shared store keeps the live-instance count at exactly one.

**Trade-off:** The `Retained<EKEventStore>` is parked in a `static` behind an `unsafe impl Send + Sync` wrapper. That is sound only because every EventKit call is already serialized by `AutoSync`'s in-flight mutex — the store is never touched from two threads at once. A future caller that hits `push_all`/`pull_all` outside that mutex would break the wrapper's safety justification.

---

### 2026-05-20 — iOS on-device Parakeet ASR via the FluidAudio package

**Decision:** On-device Parakeet transcription is provided by the FluidAudio Swift package (`FluidInference/FluidAudio`). `LocalTranscriptionEngine` dispatches by model family — Whisper stays on SwiftWhisper, Parakeet routes to a FluidAudio `AsrManager`. FluidAudio owns Parakeet model download + caching (`AsrModels.downloadAndLoad`), so `TranscriptionCatalog`'s Parakeet entries carry no `downloadURL`; a `parakeetVersion` token (`v2` / `v3` / `tdtCtc110m`) maps to `AsrModels.Version`. Tesela passes a per-version cache directory under Application Support so `deleteModel` can remove the files.

**Why:** The catalog's old Parakeet `.zip` URLs 404'd and nothing ran inference. FluidAudio ships the same `parakeet-tdt-0.6b` CoreML build VoiceInk and Handy use and manages its own model download — far cheaper than hand-rolling a NeMo runtime.

**Trade-off:** FluidAudio's `downloadAndLoad` exposes no progress, so a Parakeet download shows an indeterminate spinner, not a percentage. The package is pinned to `branch = main` (no release tag). Whisper is completely untouched — it remains the URLSession-`.bin` path.

---

### 2026-05-21 — iOS `renderBody` drops bare leaf blocks instead of persisting them

**Decision:** `MockMosaicService.renderBody` (the iOS block-list → markdown serializer, shared by daily writeback and `pushPage`) filters out *bare leaf* blocks before serializing — a block with empty text, no tags, no properties, non-task kind, and no indented children is omitted from the written file. The block is NOT removed from the in-memory `todayBlocks` / `loadedPageBlocks` array, so the user still sees and can type into a freshly-added empty block; it simply isn't persisted to disk until it has content.

**Why:** `appendTodayBlock` (and block-split) write back to the server immediately, before the user types anything. Every abandoned "Add block" tap therefore saved a blank `- ` bullet; on the next refresh `parseBlocks` read it straight back as a real empty block, so empties accumulated permanently (one user's daily had 21).

**Trade-off:** `renderBody` is now lossy by design — a future reader diffing in-memory blocks against the written file will see fewer blocks on disk, which can look like a bug. Empty *task* blocks and empty blocks *with children* ARE kept (a checkbox or an outline parent with no text is intentional). If a use case ever needs a deliberately-blank standalone note block, it would need an explicit "keep" signal.

---

### 2026-05-22 — Recurrence is an rrule-shaped struct; `Until` end-dates built at noon-UTC

**Decision:** `tesela_core::recurrence::Recurrence` is a struct `{ freq: Freq, interval: u32, by_weekday: Vec<Weekday>, end: Option<RecurrenceEnd> }`, not the former flat `Copy` enum. `weekdays` / `weekends` are not special variants — they are ordinary `Weekly` recurrences with a `by_weekday` set. The series-end check lives in one function, `advance(&Recurrence, current, done_so_far) -> Option<NaiveDate>`; `count` progress is tracked by an engine-maintained `recurrence_done::` block property (the user never types it), `until` is stateless. When a `RecurrenceEnd::Until(date)` is pushed to EventKit, the `NSDate` is built at **noon UTC** of that date (`days*86400 + 43200`), not midnight UTC.

**Why:** BYDAY (`Vec<Weekday>`) and `until`/`count` are orthogonal to frequency and cannot bolt onto a `Copy` enum cleanly; the struct maps 1:1 onto `EKRecurrenceRule` (frequency/interval/daysOfTheWeek/recurrenceEnd), keeping the Apple Reminders round-trip a straight field copy. Noon UTC: `EKRecurrenceEnd.recurrenceEndWithEndDate:` interprets the `NSDate` against the user's *local* calendar — midnight-UTC of date D is the evening of D-1 for any user west of UTC, so EventKit would end recurring series a day early. Noon UTC lands on date D for every timezone from UTC-12 through UTC+11.

**Trade-off:** The noon-UTC `Until` is still wrong for the extreme UTC+12..+14 zones (a few Pacific territories) — the fully-correct fix is constructing the `NSDate` via `NSCalendar`/`NSDateComponents` at local noon, deferred as not worth the extra FFI. `count` requiring a companion `recurrence_done::` property means a recurring block carries an engine-owned property the user shouldn't edit; it is stamped by the server bump path, mirroring how `apple_reminder_synced_at::` is engine-owned.

---

### 2026-05-22 — Dates on task blocks are typed properties, not inline links

**Decision:** A date on a task in the web client is a `date`-typed block property — `scheduled:: 2026-05-25` / `deadline:: 2026-05-25`, a bare `YYYY-MM-DD` scalar with no `[[...]]` wrapper. The `/date` command writes such properties (via `upsertBlockProperty`); it no longer inserts an inline `[[YYYY-MM-DD]]` wiki-link into block text. A dated task does **not** auto-backlink onto that day's daily journal page. Recurrence (`recurring::`) is set alongside the date by the same command. A keyword-less date routes to a configurable `bareDateField` preference (default `scheduled`); a leading `deadline`/`scheduled`/`due` keyword overrides.

**Why:** Inline `[[date]]` links were the root of four user-reported problems — the date was un-editable text, deadline vs scheduled were indistinguishable, recurrence was detached, and skip failed because no `recurring::` property existed. The user confirmed they never author date links by hand and use the daily journal only to read what they wrote that day — so the journal-backlink behavior was unwanted clutter, not a feature. A typed property is the Logseq-DB model, is what the Rust engine already reads (`deadline::`/`scheduled::`/`recurring::`), and removes the link-parsing / backlink-index complexity.

**Trade-off:** Opening a day's journal no longer auto-lists tasks due that day — that surface is deliberately moved to the (not-yet-built) agenda/today view. Existing inline `[[date]]` links and bracketed `deadline:: [[..]]` values in old notes are left as-is (no bulk migration); renderers and the engine accept both bracketed and bare forms, so old data still works but isn't normalized until re-edited. iOS still uses the old inline-date flow — the web redesign was done first, iOS is a later effort.

---

### 2026-05-22 — Agenda is an ambient buffer; recurrence projection lives on the server

**Decision:** The agenda surface lives as a new `agenda` ambient buffer (joining `calendar`/`dashboard`/`ai-workspace`/`today-in-progress`), opened via `:agenda`. Recurrence projection — expanding a `recurring::` block's future occurrences within a `[from, to]` window — happens **on the server**, in the SQLite `SearchIndex::agenda_blocks` impl, calling the canonical `tesela_core::recurrence::advance` for each step. The agenda fetches the already-expanded `Vec<AgendaRow>` and renders.

**Why:** Two forks decided.

*Why ambient, not route or derived buffer:* Ambient is the established pattern for workspace-singleton views with no backing reference (Calendar, Dashboard). Derived buffers require a `Reference`; the agenda has none. A `/agenda` route would be a top-level page; ambients live in the pane tree, so the agenda can be split-paned alongside the focused note or a daily — better composition. Adding one is three small touchpoints (component, registry, verb) — no new routing or top-level layout work.

*Why server-side projection, not JS:* The canonical recurrence engine is `tesela_core::recurrence` (Rust). Projecting in JS would duplicate `parse`/`advance`/`until`-`count` gating semantics, drift over time, and ship `recurring::`/anchors for every recurring block over the wire. The server already has the index + the engine in the same process; `calendar_marks(from, to)` is the precedent (counts), `agenda_blocks(from, to)` returns the expanded rows. Recurrence math stays in one place.

**Trade-off:** Adding a non-recurrence-projection feature to the agenda (e.g. "what about projects whose deadline is in N days") still has to round-trip through a server endpoint — cheaper for projection, slightly higher latency for any cross-cutting client filter. Mitigated by a generous fetch window (`[today-90d, today+60d]` initial) and TanStack Query caching.

**Notable architectural sibling:** A new `POST /blocks/set-property { block_id, key, value }` endpoint was added so the agenda can mark-done / reschedule without touching `BlockOutliner` (which it has no handle on, being in an ambient). The handler reuses the canonical post-save pipeline (`apply_post_save_bumps_with_info` + `apply_dependency_cycles`), so recurring tasks bump correctly when status flips to done. Any future surface that needs to mutate a single block property goes through this endpoint.

---

### 2026-05-22 — iOS NL date parser is a Swift port, not a remote call

**Decision:** The iOS app parses natural-language date input via a Swift port of the web's `date-parser.ts` (shipped as `app/Tesela-iOS/Sources/Data/DateParser.swift`), with mirrored XCTest cases (`Tests/DateParserTests.swift`) line-for-line matching `web/tests/unit/date-parser.test.mjs`. The web app's TypeScript parser remains the source of truth — the Swift port translates it, doesn't reinterpret it.

**Why:** Three options were considered:
1. **Server endpoint** (`POST /parse-input` returning parsed result): keeps a single parser source, but iOS needs date entry to work offline (mock mode and field/airplane scenarios).
2. **Date picker only, no NL**: simpler iOS, but loses parity with the web "tomorrow / next fri / deadline may 23" mental model; user explicitly wanted the same flow on both platforms.
3. **Swift port** (chosen): offline-capable, full parity, and the lockstep test suite (web tests + Swift tests cover the same grammar cases) catches drift.

**Trade-off:** Two parsers must stay in sync. The mitigation is the mirrored test suite — any grammar change on the web side that ships a new test must be paired with a Swift test for the same grammar. Documented in the design spec (`.docs/ai/phases/2026-05-22-ios-dates-design.md` §2). Long-term, if Tesela ships an Android client, this same translation cost recurs; at that point it may be worth a shared WASM-backed parser instead.

**Related tech-debt:** Adding an XCTest target on Xcode 26 surfaced an explicit-module-scanner bug with the Rust-generated `CFFI/module.modulemap` — worked around with `SWIFT_ENABLE_EXPLICIT_MODULES=NO` in `project.yml`. Long-term fix is restructuring `CFFI/` so the new scanner finds the modulemap; out of scope for the dates work but worth a follow-up issue.

---

## 2026-05-28 — Loro doc model: hybrid (per-note docs + index doc), full-parity hard cutover

**Decision:** The Loro migration uses a **hybrid doc model** — one small always-resident **index doc** (note_id → metadata + graph) plus **per-note Loro docs** (lazy-loaded, evictable). NOT a single mosaic-wide doc. Cutover is a **hard flag-day** with **full parity** (byte-identical round-trip for all notes incl. frontmatter/properties/query pages) as the gate, then the hand-rolled `SqliteEngine` oplog is deleted.

**Why not single-doc:** Claude Code initially recommended one mosaic-wide CRDT ("fine at hundreds of notes"). Claude Desktop correctly rejected this on scale: dailies alone compound to thousands/decade and everything-is-a-block means millions of blocks. A single resident CRDT OOMs iOS (jetsam ceiling) on long sessions → app killed mid-write = the exact data-loss the migration exists to prevent. Cold-start would load the whole snapshot (grows forever); corruption blast-radius = whole mosaic. Every mature system shards (Logseq/Obsidian per-file, Notion per-block, Automerge many-docs, Yjs subdocuments). The hybrid also maps directly onto the existing per-note `.md` files + per-note relay routing — less of a departure than a mega-doc.

**Why full parity before cutover:** Taylor is on Logseq until Tesela sync is solid; nothing should regress vs Logseq when he switches back.

**Why hard cutover:** No daily-driver dependence during migration → no need for dual-protocol coexistence or gradual rollout. Flip all relay participants (Mac server, iOS, Savanne's devices) at once; web is an HTTP client and unaffected.

**Plan:** `.docs/ai/phases/2026-05-28-loro-cutover-spec.md` (Phases 0–7). Relay protocol + encryption unchanged; only the opaque ciphertext payload (Vec<EncodedOp> → Loro updates) and the engine swap.

---

## 2026-05-28 — Structured-first; CRDT is truth; structural (not byte) parity; scalar props for v1

Triangulated Claude Code + Claude Desktop, decided by Taylor. Refines the Loro cutover spec.

**1. Structured-first (Anytype direction).** `query::`/`type::`/`sort::` etc. are page PROPERTIES (first-class structured data), not raw text. The parser dropping non-bullet lines is a gap, not a content category. The per-note Loro doc models block = `{text, indent, properties: map}` + page-level properties. NO raw-text escape hatch (it'd be opaque, unreferenceable, and ripped out at property-system time).

**2. The CRDT is the source of truth; markdown files are a deterministic materialized VIEW.** Inverts the old `project_property_system_vision` line "files are truth." Files stay readable/diffable/greppable but are no longer authoritative. Correct for structured-first + collaboration.

**3. Parity bar = STRUCTURAL, serialization = DETERMINISTIC (not byte-identical).** Claude Desktop's key catch: byte-identical markdown round-trip is the Logseq-fidelity tar pit (whitespace/ordering/delimiter preservation) AND pointless under structured-first (you don't hand-edit a query-builder's output). Requirement is: same CRDT state → same bytes (clean diffs, stable grep), no verbatim-preservation of arbitrary input. The divergence check + Phase 1 acceptance compare PARSED STRUCTURE, not raw bytes. Cutover does a one-time canonical reserialization of the mosaic. This is what keeps Phase 1 from ballooning.

**4. Scope line:** Phase 1 *preserves + merges* properties; it does NOT build the property SYSTEM (global registry, type inheritance, `extends`, table views) — those sit on top, per `project_property_system_vision`.

**5. Property values scalar-string in v1; multi-value list semantics deferred.** Scalar achieves parity for the 13 notes (all scalar page-props). Clean union-merge for multi-value props (`tags`, aliases) needs Loro list containers + `value_type` knowledge → lands with the property system / collaboration phase. Known limit until then: concurrent multi-value edits are LWW-on-the-whole-string (tag merges misbehave). Conscious, not a surprise.

Spec: `phases/2026-05-28-loro-cutover-spec.md` (decisions 2–4 in the locked-decisions block; Phase 1 updated).

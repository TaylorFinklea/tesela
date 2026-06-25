# Architecture Decision Records

Concise log of non-obvious decisions. Newest first.

---

### 2026-06-25 — iOS→desktop push broken (zero relay PUT); diagnose before fixing

After builds 48/49 fixed liveness + APNs, a NEW symptom: iOS edits don't reach the desktop and desktop edits overwrite the iOS-authored block. Boundary-confirmed via `wrangler tail`: typing `PUSHTEST_IOS` on iOS produced **zero relay PUT** (desktop cursor frozen). So iOS edits are recorded locally but never pushed; the "clobber" is the consequence (the local-only edit is overwritten when the desktop's state comes down).

- **A 9-agent Workflow (5 parallel tracers → synth → 3 adversarial verifiers) traced the whole push pipeline.** Confirmed mechanism: the Graphite today editor pushes via a PER-KEYSTROKE splice seam (GrDailyView `onTextSplice` → `spliceTodayBlock` → `onLocalSplice` → `RelayTicker.spliceAndPush` → `engine.spliceBlockText`); `onCommitEdit` (blur) is a deliberate no-op. `spliceAndPush` **discarded** `spliceBlockText`'s op count (`_ = try await`, RelayTicker.swift:510); `spliceBlockText` returns **Ok(0) (not a throw)** when the block isn't a live tree node (loro_engine.rs:936) → a 0-op splice is silently swallowed (no PUT, no error). The sibling `setBlockPropertyAndPush` already guards this exact class (`applied==1` + surfaces error); the splice seam never got the guard.
- **BUT all 3 verifiers REFUTED that as THE cause** (high agreement): for a VISIBLE, desktop-bootstrapped block the splice should resolve (note_id = blake3(slug); bid read off the materialized `<!-- bid -->` = the live node's meta) → Ok(1) → version advances → exported → PUT. So zero-PUT means the loss is elsewhere: (a) stale/empty `serverDailyId` (midnight rollover 06-24→06-25 — there IS re-derive-on-refresh handling, may be incomplete), or (b) recorded-but-not-exported (outbound `broadcast_cursor`/produce), or (c) a real applied==0 splice miss.
- **Decision: OBSERVE before fixing** (iron law + 3 refutations; one candidate fix could create TWIN blocks on a bid mismatch). Build 50 (`824ed89a`) added a `lastSpliceDiag` ("Last splice" in Settings → Sync) capturing the previously-discarded `applied` + outbound `sent/failed` + the slug.
- **CONFIRMED via the on-device diagnostic: `slug=2026-06-25 applied=1 sent=0 failed=0`.** So the verifiers were right: NOT the splice-seam discard, NOT a stale slug — the splice records (version advances) but the outbound producer exports nothing. **ROOT CAUSE:** `produce_relay_updates` calls `export_doc_update(note, since=broadcast_cursor)`, which returned `None` when `since` failed to decode (corrupt / version-format change / stale lineage), and produce then **silently skipped** the dirty note (`if let Some(bytes)=…` with no else, loro_engine.rs:1017) → a note with a bad persisted broadcast cursor could NEVER re-broadcast; its local edits stranded forever, then clobbered by the desktop's version on the next inbound. **FIX `56d67001` (build 51):** `export_doc_update` self-heals — on a decode failure or an empty/failed incremental, fall back to a full `ExportMode::Snapshot` (idempotent on the receiver; the next confirmed PUT rewrites a fresh decodable cursor). A dirty resident note now ALWAYS exports; self-heals existing stuck notes on the next tick. RED test `produce_re_emits_when_broadcast_cursor_is_undecodable`.
- **Method note (worth repeating):** the 9-agent Workflow's value was the ADVERSARIAL VERIFY — it produced a confident-but-wrong synthesis (splice-seam discard) and its OWN 3 verifiers refuted it toward "recorded-but-not-exported," which the on-device diagnostic then confirmed. Don't ship a synthesis without the verify pass + a runtime observation. ~857k tokens, 9 agents.

### 2026-06-24 — iOS "data loss" was sync LIVENESS, not push logic; date chips must survive editing

Build 46/47 device test surfaced "iOS edits don't reach web for hours / look lost." Root-caused live against the desktop's CF-relay state, then fixed (`e6d1d83b`, `5c65e9d2`). Non-obvious calls:

- **It was never push LOGIC — edits converge; it's sync LIVENESS.** Diagnosed by curling the desktop's `/sync/relay/status` + daily notes while Taylor edited: the iPhone was on the SAME relay+group (inbound seq matched the desktop's), reads worked, and edits DID arrive — ~2h late, then instantly on app reopen. Two compounding bugs: (1) `RelayTicker` backoff slept `tickIntervalSeconds * (1 << min(consecutiveErrors, 12))` = `2 * 2^12 ≈ 8192s ≈ 2.3h` (every comment claimed ~60s; `maxBackoffMultiplier=12` was used as a SHIFT exponent, not a seconds cap); (2) `.active → start()` no-ops on an existing loop, so foregrounding couldn't wake a loop parked in that sleep — only a background→foreground cycle (which `stop()`s + rebuilds) recovered, hence "converged when I reopened the app." Fix: pure tested `backoffSleepSeconds` capping the RESULT seconds at 60; `wake()` (reset+stop+start → immediate tick) on `.active` in BOTH shells.
- **The desktop embed is loopback-only, so iOS↔Mac is 100% relay-dependent.** The Tauri desktop binds `127.0.0.1` and reads its relay URL from `~/Library/Application Support/tesela/desktop.toml` (`main.rs:163`); the mosaic `config.toml` has no `[sync.relay]`. So there is NO LAN HTTP path to the desktop app — iOS's "direct HTTP (LAN-fast)" mode is dead, relay is the only transport, and any relay-side stall = total iOS→Mac loss. (iOS still shows a stale `127.0.0.1:7474` "Connected" in relay mode — misleading; queued as a sync-UX-honesty follow-up.)
- **Date chips are STRUCTURED state, not prose — keep them visible while editing.** `BlockRow` gated the whole chip row on `!isEditing`; tags/inline props correctly hide during edit (they're in the editable text, would duplicate), but scheduled/deadline/recurring aren't in the text, so hiding them made a date set on a focused block look like it failed. Split via a pure `chipVisibility(...)` (dates ignore `isEditing`).
- **Diagnostic method worth repeating:** for a multi-device "where's my edit?" report, instrument the boundary — poll the receiving node's relay cursor + materialized note in the background while the user makes ONE labeled edit. It localizes the break (not-pushed vs pushed-not-received vs received-not-applied) far faster than reading the sync code.

**Delete-not-propagating (same day, build 48) → APNs background-wake gap.** A block deleted on web stayed on iOS ~44min until a force-close. Root-caused by ELIMINATION, not guessing:

- **Engine ✓ + refresh-chain ✓ ruled out by test.** Inbound apply DOES re-materialize (`apply_doc_update_status` → `refresh_note_derived` + `materialize_note`, loro_engine.rs:1063/69/74), and `applyRemoteChange → refresh` drops the block (new `MockMosaicServiceTests` guard). So neither was the bug.
- **Sim repro proved the foreground/running path WORKS on build 48.** Seeded a sim (`simctl spawn defaults write app.tesela.ios backend.mode relay` + `relay.cachedPairingCode <code from /sync/peer/pairing-code>`), paired to the live CF group; drove add+delete via the desktop API (`POST`/`DELETE /notes/{id}/blocks/{bid}`). A desktop delete dropped on the sim — engine file AND UI — in ~5s on warm `.active` reopen, ~16s on the running loop. ⚠ A sim can't reproduce true iOS background SUSPENSION (it keeps backgrounded apps alive) NOR APNs (no token) — those need a real device.
- **The real bug: APNs token registration wasn't relay-scoped.** While suspended, iOS does zero sync; the background wake is an APNs `content-available` push from the relay on deposit. `wrangler tail` on the live CF Worker showed ZERO `[apns]` on deposits → `listOtherApnsTokens` empty → the CF Worker had NO token for the group. Cause: `RelayTicker.maybeRegisterApnsToken` keyed registration by token ALONE, once-per-session (in-memory) — so after the HA→CF migration the token stayed registered with the OLD relay and never re-registered with CF in-session. A force-quit (fresh session) re-registered → tail then showed `registered device…` + `deposit → waking 1` + `push → 200 OK`. **Fix `594b0403`:** key registration by (token, `cursorScope`) via `apnsRegistrationKey` — re-register on relay change, mirroring the inbound-cursor scoping that fixed the same HA→CF class. Per-launch re-registration preserved.
- **Verify-on-CF gotcha:** `wrangler secret list` confirms the APNs secrets ARE set on the deployed Worker (they were fine); `npx wrangler tail --format pretty` from `cloudflare-relay/` shows the live `[apns]` logs — the fastest way to see whether a device is registered + getting pushed. `wrangler` is npx-only here (Taylor's CF OAuth).
- **APNs is best-effort, not a guarantee** — iOS throttles silent pushes. The RELIABLE path for "open phone → see current state" is the foreground sync (proven ~5s on build 48); APNs just narrows the asleep-and-not-looking window.

### 2026-06-23 — iOS property-registry parity: client-side registry, display-only raw-lines strip, typed composite writes, audit-as-process

iOS task/property handling was entirely hardcoded; Phase 5 made it the THIRD engine that consumes the type/property registry (spec `phases/2026-06-23-ios-property-registry-parity-spec.md`). Non-obvious calls:

- **Build the iOS registry CLIENT-SIDE from the synced Property/Tag pages, NOT `GET /types`.** The rich metadata — `nl_triggers`, `choice_colors`, `chip_*`, `chord_key`/`value_chord_keys` — lives ONLY in Property-page frontmatter; the API/DB (`get_all_property_defs`) carries only `name/value_type/values/default/show/hide_*`. So iOS mirrors web's `buildRegistry`/`getTagPropertyDefs`/`applyOverride` over the synced notes (the same code path works in `.relay` (no server), `.http` (pages already synced locally), and `.mock`). New `PropertyRegistry.swift` + a nested-YAML `FrontmatterParser` (the old single-line scrapers couldn't read `property_overrides: {...}` / `nl_triggers: [...]`). Perf: gate the nested parse to type pages, and rebuild OFF the main actor (it runs on the relay tick).
- **The raw-`status::`/`tags::`-in-the-edit-buffer bug = a display-only iOS strip, NOT the engine fix.** The live-reconcile reads the RAW engine `text_seq` via the FFI, which carries the property lines folded into the block text after a `NoteUpsert` round-trip (the materialized `.md` IS deduped; the raw read isn't). `stripPropertyLines` drops them before they hit the buffer — display-only, never written back, so ZERO convergence risk. The DURABLE root cure (`reconcile_tree_to_blocks` writes prose-only + lifts props into the container, mirroring the BlockUpsert seed arm) is **gated on `migrate_in_text`/fleet-readiness**: an old FFI that can't read the lifted container could re-broadcast a fleet-wide property erase. Deferred until the whole fleet is container-read-capable — a Rust/relay change, not an iOS build.
- **Every iOS property write goes through the typed per-key CONTAINER seam (`setBlockProperty` → `onLocalPropertySet`/`BlockPropertySet`), never a text splice** — that's what makes slash/NLP/date sets converge instead of re-triggering the raw-lines bug. The seam needs a COMPOSITE `"noteId:bid"` address (`resolveLocalBlockBid` → `splitBlockAddress`); a BARE bid silently no-ops (swallowed by `try?`). Freshly-created blocks have no `noteId` until a refresh, so `resolveLocalBlockBid` falls back to resolving the owning note from the in-memory containers.
- **Audit-as-process (the real lesson).** I shipped build 45 with 124 unit tests + 3 adversarial review lenses per phase all green — and the authoring was SILENTLY BROKEN (the bare-bid no-op). The lenses confirmed the writes were *structured* but never that the bid *resolved end-to-end*: "structured" ≠ "persists". A proactive 3-finder audit (find → adversarially verify each finding, 10 found → 7 confirmed) caught it + 6 more. Treat shipped-but-only-component-tested work as guilty until an integration audit proves it; that layer sits between "tests pass" and "works on device".

### 2026-06-22 — Per-type property config: one override map on Tag pages, THREE resolvers kept in sync

Anytype/Logseq-DB parity: one global Property (e.g. Status) carrying DIFFERENT choices/visibility/default PER TYPE (spec `phases/2026-06-22-per-type-property-config-spec.md`). Non-obvious calls:

- **Config = a `property_overrides` map in the Tag page's frontmatter**, resolved through the `extends` chain. Semantics (must match across all engines): choices **REPLACE** if the override has them, `hide_choices` **SUBTRACT**, `default` override wins, 3-state `show` = override else derived from `hide_by_default`. Legacy `hidden_{Prop}` is an alias that folds into `{Prop}.hide_choices` (the one intentional asymmetry).
- **The merge now lives in THREE engines that MUST stay in sync** — Rust `apply_override`/`get_resolved_tag_def` (views/config via `GET /types`), web TS `applyOverride`/`getTagPropertyDefs` (editor seeding/visibility), and (new this session) the iOS registry. Tag/Property pages are markdown notes; `tag_defs`/`property_defs` are DB caches rebuilt on index (zero sync impact). Migration via additive `ALTER ADD COLUMN`.
- **`tesela reindex` was silently incomplete** — `cmd_reindex` called `index.upsert_note` (search index only), never `index_type_info`, so on a mosaic whose type pages were synced-but-never-written, `tag_defs`/`property_defs` stayed empty and `GET /types` returned nothing. Fix: call `index.reindex` (= both). Surfaced only by landing per-type config on Taylor's real 540-note mosaic — fresh mosaics worked because the server SEEDS (writes → indexes) the built-ins.
- **Upgrade existing type pages in place via the note API, never delete+reimport.** A type is just a markdown note + a rebuildable cache, so upgrading = editing frontmatter through the Loro-aware note API (preserves block IDs → clean sync). Reimport would lose custom Tesela types and risk data.

### 2026-06-18 — iOS inline-autocomplete framework, the lazy-materialization page-list gap, and the word-wrap fix

Non-obvious calls from the iOS editor sprint (builds 21–28):

- **One trigger-detection framework for `[[` / `#` / `/`.** `LinkSuggest.detectTrigger` dispatches by kind: `[[` link (may contain spaces → checked FIRST, bounded by `]]`/newline) vs a single whitespace-delimited token starting with `#` (tag) or `/` (slash) at line-start/after-whitespace (so `C#`, `http://x`, `a/b` never trigger). State is one `EditorAutocomplete` carrying the `kind` + a generic `[Suggestion]`; `BlockRow` renders chips IN PLACE of the format buttons in the SAME keyboard-accessory pill (no fragile `inputAccessoryView` resizing). Slash verbs are text-insert/openers (link/tag insert just `[[`/`#` so the picker chains open); block-level actions stay on the toolbar.
- **The page/tag list MUST come from the Loro index, not materialized `.md` on disk.** `[[photography` found nothing because iOS materializes notes **lazily** — a synced-but-never-opened page has no local file, but its title lives in the always-resident index doc. Fix: new FFI `SyncEngineHandle.index_entries() → [IndexEntryRecord]` (wraps `LoroEngine::index_entries`), bridged via `RelayTicker` → cached in `MockMosaicService` (`indexPageCache`/`indexTagCache`, refreshed each `.relay` refresh) → `searchablePages`/`searchableTags` filter the complete list. The on-disk scan is now only a pre-first-index fallback.
- **Word-wrap = `sizeThatFits`, not compression-resistance.** A non-scrolling `UITextView` in SwiftUI reports its full single-line text as its intrinsic width; lowering horizontal compression-resistance (the first two attempts) only *changed* the breakage. The real fix: implement `func sizeThatFits(_:uiView:context:)` on the `UIViewRepresentable` to size the editor to the proposed width + report the wrapped height. (Verify UIKit-keyboard editor changes on-device; the sim sits behind onboarding and can't be driven headlessly to the editor.)
- **pi-offload division (post-confirmation):** self-contained NEW views → pi (gpt-5.5 built the Graphite Search view 5/5 from a tight spec naming the two reference files); framework-coupled editor work → Opus. gpt-5.5's one slip: it committed despite "don't commit." Logged to `~/.claude/model-scorecard.md`.
- **Arch-review verdict (eval of 3 open-source reports, `.docs/ai/review/`):** ~20% signal. Declined the god-module splits (`loro_engine.rs` is 4.3k prod + 5.1k co-located tests; `serve()` is 395 lines not the reported 1,800), trait-splitting for a one-implementation world, localhost-auth, the codegen query parser — team/SaaS patterns with no second implementer/user/hostile network here. Acted only on real findings (C23 data-loss guard + C19/C20/C21/C24/C6 hygiene).

### 2026-06-15 — Query language = JQL; the colon DSL is legacy sugar in the same grammar

Taylor prefers a **JQL-style** query language (`type = task AND points > 5`, `status IN (todo, doing)`, `priority >= 2 ORDER BY due ASC`) over the colon DSL (`tag:task points:>5`). Non-obvious finding that shaped the work: the **engines already parsed full JQL** — the Rust parser (`crates/tesela-core/src/query.rs`) is a recursive-descent grammar (`parse_or → parse_and → parse_unary → parse_predicate`, `BoolExpr{And/Or/Not/Atom}` + `Predicate{Cmp/In}`, `IN`/`LIKE`/`BETWEEN`/`IS NULL`/`ORDER BY`), with the colon DSL as legacy sugar in the SAME grammar; the web TS engine mirrors it 1:1. So JQL was a **front-end + iOS-parity** job, not a new parser.

- **P1 (web, `541a8322`):** made JQL the default authored/displayed form (`/query` template, placeholders, validation hint); `kind = page` validates (not just `kind:`); **`ORDER BY` now sorts the inline query block, L5-typed** (`applySort` reusing `compareTyped`) — the inline block sorts correctly where the server's string-only `apply_sort` doesn't (server typed-sort parity = a P3 follow-up).
- **P2 (iOS, `a38ed68d`):** iOS's `LocalQueryEngine` was deliberately flat-AND-only and dropped OR/parens/IN/LIKE/BETWEEN/IS NULL toward match-all. Restructured to the Rust `BoolExpr` tree (parser + `evalExpr` + `likeMatches`).
- **Parity is a TESTED contract:** the shared `query-conformance.json` fixture gained **69 JQL cases** (41 core `0d6b3f77` + 28 adversarial `cc9d316e`); **Rust is the source of truth** — every `expect` validated against it, and web + iOS must match. 182 cases now green on all three engines. A LIKE escape-set difference (Rust escapes `-&#~`, the iOS port doesn't) was reviewed + **verified harmless** (literal in NSRegularExpression normal mode anyway) and is guarded by cases.
- **Not done (P3):** palette JQL examples, an optional DSL→JQL one-shot converter for existing saved views, error surfacing on malformed JQL, server-side typed `apply_sort`, and the iOS inline-query-widget sort (filter works; order is source-order). The inbox chip composer stays colon-DSL (its whitespace token-split assumes single-token clauses; JQL clauses have spaces). `INBOX_VIEW_DSL` stays colon-DSL (conformance-pinned across Rust+fixture; parser treats both forms as equivalent).

### 2026-06-12 — Agent-pipeline roles: Opus = planner; Codex/Pi execute; Opus implements only L items

**Decision (Taylor), effective when Opus's rate limits recover (~1 week):** lock in a standing division of labor across the agent fleet.

- **Opus (Lead/T1) = PLANNER.** Default job is to spec/triage/architect work as self-contained backlog items for Codex + Pi Mono to execute — NOT to implement broadly. Opus owns the roadmap, decisions, and item decomposition.
- **Opus implements personally ONLY the `L` (and up — `XL`) complexity items** — the genuinely hard/risky ones (sync hot path, Loro/CRDT, FFI, crypto, architecture). Everything `S`/`M` is specced down, never hand-built by Opus.
- **Codex (Senior/T2) + Pi Mono (Senior/T2) = EXECUTE** the S/M items Opus plans. Interim arrangement (this week, while Opus is limited): Codex plans + reviews + merges, Pi implements — see the 2026-06-12 Codex/Pi batch report ledger. Once Opus is back, planning reverts to Opus and Codex/Pi go back to pure execution (+ peer review).
- **Fable** remains available for hard/complex work alongside Opus.

**Why:** conserve Opus's scarce capacity for the high-leverage work only it can do well (planning + L/XL implementation); keep the cheaper Senior agents saturated on safe S/M throughput. Routing stays backlog-driven per AGENTS.md Tiered model routing — this just sets Opus's *default mode* to planner.

**How to apply (future Opus, on return):** don't reflexively start implementing. Plan first: decompose into backlog items with tier_floor/complexity, hand S/M to Codex/Pi, and only pick up `L`/`XL` yourself. First task back: review the Codex/Pi batch (ledger at `.docs/ai/phases/2026-06-12-codex-pi-batch-report.md`) and clean up before planning new work. See [[project-emacs2-northstar]].

---

### 2026-06-12 — North star: Tesela = emacs 2.0; keyboard + command registry ALWAYS first; stay Svelte (no Zed fork)

**Context (Taylor):** "I'm wanting Tesela to be my personal emacs 2.0 so I can do everything purely from the keyboard on desktop — but one that has a real mobile and RTC experience, not just a pretty website." Asked whether to stay on SvelteKit web (Tauri desktop) + SwiftUI iOS, or pivot desktop to a **forked Zed**.

**Decision:**
1. **Keyboard-first and the command registry are the permanent #1 priority** — above any view, theme, or feature. Every action is reachable/driveable without the mouse; new surfaces ship keyboard-complete.
2. **The command registry is the architectural spine.** Every action = a named, metadata-carrying command. ⌘K palette, keybindings, leader/which-key chords, and the slash menu are all *dispatchers* into the one registry — never per-feature handlers. Rebindability, introspection, and eventual plugin extensibility hang off it. This is the emacs-ness, and it's a *command-system* property, not a renderer property.
3. **Stay SvelteKit web (Tauri desktop) + native SwiftUI iOS + Rust/Loro core. Do NOT fork Zed.**

**Why not fork Zed (the two headline "wins" evaporate):**
- **"RTC already built" — wrong CRDT.** Zed's collab is its own rope/clock CRDT, *not* Loro. We've committed to Loro as the canonical sync spine ([[project-loro-migration-committed]], relay + iOS + convergence work). A fork forces ripping out Loro (throwing away the convergence work *and* breaking iOS, which can't use Zed's CRDT) or ripping out Zed's collab (so "RTC free" gave nothing). It collides head-on with our most-invested subsystem.
- **"Neovim-like editor" — wrong data model.** Zed is a *text/rope* editor (lines, LSP, multibuffers). Tesela is a *block outliner* (nested blocks, properties, refs, transclusion, tiles). You'd fight GPUI on every feature that makes Tesela Tesela. Vim *navigation over an outliner* we already have in CodeMirror; that's not the hard part.
- **iOS stays Swift either way** → a fork buys zero mobile unification, and it *splits* today's "web == desktop" (one Tauri-wrapped renderer) into three renderers (GPUI desktop / SwiftUI iOS / web).
- **Fork cost:** owning ~500k+ lines of fast-moving Rust on bespoke GPUI, forever, understood by one person.
- **Emacs-ness specifically argues *against* a fork:** uniform command dispatch + total rebindability + extensibility are *frontend command-architecture* you want to own outright — not inherit from a code editor's code-centric command set.

**The legitimate pull (parked, not now):** Rust-all-the-way-down + native perf + a future where Savanne co-edits over the exact same Loro core. The right expression of that is **a thin GPUI shell over Tesela's own Loro-backed block core** (borrow Zed crates like vim, don't fork the editor), evaluated *after* the M3 spine is done and RTC co-editing is a near-term goal. Mine Zed, don't marry it.

**Clean test:** fork the thing whose data model matches yours. Zed is a text editor; Tesela is a block outliner — so own the command system in Svelte. **Tauri (not raw browser) is load-bearing** for the keyboard goal: it lets us suppress the webview's built-in shortcuts and claim the native keymap the browser would otherwise steal.

See [[project-tesela-vision]] + roadmap "What Tesela Is" / "Product Vision".

---

### 2026-06-10 — Backups capture the AUTHORITY (Loro state + sync identity); scheduled in-server; provable via /backup/status

**Context:** audit rec (l4.json "Back up the authority, not the export view") + Taylor: must be 100% certain backups happen before fully leaving Logseq. Pre-change, backups held only the materialized export view (`notes/` etc.) — a restore had zero CRDT history + no device/group identity → reseed → disjoint-lineage twin hazard.

1. **Capture set = the authority.** `.tesela/loro/**` (skipping in-flight `*.tmp.*`), `device_id.hex`, `group_id.hex`, `group_key.bin`, `relay_state.json`, `sync_peers.json` — all optional-presence. Manifest `SCHEMA_VERSION` 1→2; restore is manifest-driven so v1 backups stay restorable; old binaries refuse v2 (correct: they don't know it carries the authority).
2. **Local backups stay plaintext** (NOT encrypted as the audit suggested): the local destination lives in `.tesela/backups/` beside the live plaintext `group_key.bin` it copies — encrypting the copy adds nothing, and forcing it would make backups fail keyless. Non-local (external/git) destinations remain always-age-encrypted, so the group key never leaves the machine in plaintext.
3. **Scheduler lives in tesela-server** (`backup_scheduler.rs`, mirrors the notifications-scanner pattern): backup at startup (+15s settle) + every 6h, GFS prune after each. Env knobs: `TESELA_BACKUP_INTERVAL_SECS` (21600; 0=off), `TESELA_BACKUP_ON_START` (1), `TESELA_BACKUP_STARTUP_DELAY_SECS` (15), `TESELA_BACKUP_KEEP_DAILY/WEEKLY/MONTHLY` (7/4/6). Shutdown hook + scheduler share one `run_configured_backup` (same destination/encryption policy every trigger). `.backup.lock` already serializes concurrent triggers.
4. **Provable:** `GET /backup/status` = newest on-disk manifest (path/size/validated/`includes_loro_state`/`includes_sync_identity` + per-category contents summary) + scheduler state (last run/error, `next_scheduled_at`, cadence/retention) + `auto_on_quit`. On-disk manifest is the truth source (covers manual/shutdown/pre-restart backups), scheduler state rides alongside.
5. **Restore drill = the certainty artifact** (`tesela-server/tests/restore_drill.rs`): backup live engine → offsite-copy → `rm -rf` mosaic → restore → reopen: byte-identical encoded Loro version vector (a reseed would fail this), identical rendered+materialized content, identical device/group identity, writes continue the same lineage. Also proven end-to-end through the spawned server binary + HTTP.
6. **Not fixed here (still open from the audit):** `POST /backups/{name}/restore` unpacks into the RUNNING mosaic while the engine holds pre-restore state (next materialize can clobber). Restore stays a stopped-engine/CLI operation until that's redesigned.

---

### 2026-06-09 — Ultracode audit + product review: two-stream plan, relay topology, Reminders containment, full testing program

**Context:** full-repo multi-agent audit (12 bug finders + adversarial verifier per finding; 7 arch lenses + fact-checkers; 169 agents). 91 confirmed findings (3 distinct criticals), 42 fact-checked recommendations. Report + per-claim file:line evidence: `~/.harness/reports/tesela/20260609-bugbash-arch-review/`. Execution spec: `phases/2026-06-09-audit-hardening-spec.md`. Taylor decided (structured product review):

1. **Now = FULL hardening batch, two PARALLEL streams.** Stream A (Rust/iOS): relay seq black hole + iOS `.relay` write gates + cursor-past-failure family + scoped cursors + auth_key + data-corruption batch (mojibake/PUT-200/note_tree). Stream B (web): Graphite cutover — 7 parity bugs → flip /g default → delete v4/v5 behind a parity checklist. Rationale: streams barely overlap; the held product test gates only on A.
2. **Relay topology:** the relay is a zero-knowledge **mailbox**, never an authority — CRDT convergence needs no mediator, so LAN P2P (step 6) is orthogonal upside, not an alternative. **HA carries sync now → the CF Worker becomes the ONE canonical production spine** once deployed/proven; the Rust relay is then conformance-frozen as a self-host option (two production implementations already diverged: the seq bug + 3-way body-limit mismatch were Rust-only). Mac-hub WS retires as a *device* transport in Milestone 3 (stays for web UI).
3. **Apple Reminders (+ recur-bump): disable now, re-route next.** Both write around the Loro engine (`store.update`), so the engine reverts their writebacks and they never sync; auto-sync also self-retriggers every 30s and the conflict gate fails open. Auto-sync flips default-OFF in Stream A; the engine re-route is the first item of Milestone 3. General principle adopted: **every note mutation goes through the engine** (tag-rewrite, versions, etc. queue behind the same rule).
4. **Testing = FULL program:** CI green (one `cargo fmt --all` — red since 2026-04-14 because fmt fails before clippy/tests ever run) + CI gates (workspace tests, svelte-check, web e2e, relay conformance on BOTH impls, iOS compile smoke) + cross-process relay convergence harness (built during Stream A so the fixes land locked) + iOS unit target gated in the TestFlight script + FFI regenerate-and-diff drift check.
5. **Milestone 3 = finish the sync spine:** CF deploy (with its 1 MiB body-cap config + registration limits BEFORE public) + minimum key/pairing model (wrapped/passphrase-derived key; group keys → iOS Keychain) + HA→CF cursor migration + WS-hub demotion + Reminders re-route + NoteDelete tombstone design (deletes currently have NO wire form — never propagate, resurrect from snapshots).
6. **Hygiene:** repo-root cleanup approved (stray tesela.db/zips/movs/screenshots); **push-at-session-end** adopted (agent reminds; Taylor pushes). RELEASE.md history purge + auto-release retirement explicitly PARKED (73 MB blob, ~87% of tracked bytes — revisit later).

---

### 2026-06-08 — App Store export compliance: standard crypto = EXEMPT, but EXCLUDE FRANCE before any PUBLIC release

**Fact (verified `crates/tesela-sync/src/crypto/`):** the iOS build links the Rust sync FFI, which implements app-layer encryption beyond Apple's OS — **ChaCha20-Poly1305** (AEAD) for end-to-end sync-envelope encryption with the group key, **HKDF + HMAC-SHA-256** (relay auth / KDF), **BLAKE3** hashing, **rustls** TLS. All standard published algorithms (RFC 8439 / FIPS 180 / RFC 5869). So the app DOES contain encryption — "your app uses no encryption" is false.

**Classification:** standard published algorithms used only for Tesela's own data sync (not a cryptographic product) → qualifies for the **standard-cryptography / mass-market exemption** (US EAR §740.17(b)(1)). Uses encryption, but EXEMPT.

**TestFlight answers (2026-06-08, the exempt low-friction path):** algorithm type → "None of the algorithms mentioned above"; "available in France?" → **No**. Clears TestFlight with zero paperwork. (Strictly-accurate alternative = "Standard encryption algorithms" + "qualifies for exemption: Yes" — same exempt outcome.)

**⚠⚠ OBLIGATIONS BEFORE ANY PUBLIC APP STORE RELEASE — do NOT publish without doing these (TestFlight/internal is fine; PUBLIC distribution is the trigger):**
1. **France — EXCLUDE it** from App Store availability (or file France's encryption-import declaration). Apple's export flow explicitly flags France; we answered "not available in France", so a public release MUST keep France deselected in App Store Connect → Pricing and Availability, or we're in breach. This is the headline reminder.
2. **US BIS self-classification report** — the §740.17 exemption requires a one-time/annual report (encryption@bis.doc.gov + enc@nsa.gov) once the app is publicly exported. File it.
3. **Other restricted markets** — review encryption-import rules for any other target market (Russia/China have crypto regimes; Apple often auto-handles, but verify). Most of the EU besides France allows standard crypto without a declaration — don't blanket-exclude the EU, just France (+ anything the review flags).
4. **Info.plist** — declare `ITSAppUsesNonExemptEncryption` (match the exempt path) to stop the per-submission encryption dialog.

See also [[project-ios-release-convention]] (memory).

---

### 2026-06-08 — Task properties: priority p1/p2/p3 flags + Todoist "detect-inline, lift-below" display (Model B)

**Decision (Taylor, harness-deck mock-ups `tesela/20260607-task-property-ux`):**
- **Priority = `p1`/`p2`/`p3`/`p4` flags** (not low/med/high, not a generic "Priority: critical" chip). Colors: **P1 red, P2 amber, P3 blue, P4 default (no flag)**.
- **Display model B — Todoist smart-add:** as you type, detected parts (`p1`, dates like "tomorrow"/"fri"/"!jun 9", `#tags`) highlight **inline**; on commit they **lift out of the text into a quiet property strip BELOW the block**. Properties do NOT render as right-edge chips. (Rejected A = below-only, no detection; C = right-edge.)

**Detection gating — per-tag, NOT per-token markers (decided 2026-06-08, supersedes the marker/trailing/anywhere question in `tesela/20260607-date-detect`):**
- Inline NLP detection runs ONLY on blocks carrying a **detect-enabled tag**. Configurable per-tag via a `detect_tokens` flag on the tag page frontmatter; seeded **on for `Task`**, off elsewhere.
- **Gate = the block's DIRECT tags only** (`ParsedBlock.tags` = own `tags::` + inline `#tags`), NEVER `inherited_tags`. Children that merely inherit `#Task` from a parent do NOT get NLP — they must be directly tagged / make-tasked. (Both the lift and the cm highlight operate on each block's own text, so inheritance can't leak in.)
- **Inside an enabled block, detection is fully aggressive** (bare multi-word dates like `next tuesday`, `in 3 days`): bare date → `scheduled`; `due`/`deadline` keyword → `deadline` (reuses `parseDateAndRecurrenceInput`/`extractField`); `p1`–`p3` → priority. No per-token marker — markers can't express multi-word dates, which is the reason this approach won.
- **⌘↵ make-task = "tag it AND parse it"** — retroactively lifts already-typed tokens (typed `do dishes tom p1`, then ⌘↵ → Task tag + scheduled tom + P1).
- **Retrofit:** Part 2a's priority lift (currently ungated) gets gated on this flag.

**Build is phased (milestone-sized):**
- **Part 1 (foundation) — display:** p1/p2/p3 flags + extend `BlockDateRow` into a below-block property strip (priority + scheduled + deadline) + **dedup**: drop priority/scheduled/deadline from the right-edge `DisplayChip` path (`displayChipsFor`) — they were double-rendering (chip + row). Priority set via existing mechanisms (`/p`, property editor) until Part 2.
- **Part 2 — inline detection:** an NLP-ish parser detects `p1`/dates/`#tags` while typing (cm decoration highlight) + lifts them to structured props on commit. The novel/harder layer.

**Defaults locked (were open in the scoping):** priority `choices` → `[p1,p2,p3,p4]` (replace low/med/high in ALL seed sources: live `logseq` mosaic, repo `notes/`, fixtures); the priority RENDERER normalizes legacy values (critical→P1, high→P2, medium→P3, low→P4) so existing `priority::` data still shows a flag (no destructive data migration). Per-value color = a priority special-case in the renderer (not a general `value_colors` schema yet). Tag pills stay separate (the right-edge colored pills from 2026-06-07).

---

### 2026-06-07 — Tag/chip redesign: colored per-tag pills (right-edge) + ↵/⌘↵ commit gesture

**Decision (Taylor, via harness-deck mock-ups `tesela/20260607-tag-chip-redesign`):**
- **Look = colored per-tag pills** (option C). Keep the right-edge pill placement, but each tag pill is COLORED per-tag (color from the tag's page), with a small color dot — scannable across a list. (Rejected: inline pills, plain coral right-edge pills, stay-literal.)
- **Commit gesture = yes** (Logseq-style): while the `#tag` autocomplete popup is open, **↵ commits the tag to a chip**, **⌘↵ leaves it as literal `#text`**. Scoped to the popup so it does NOT clash with ⌘↵ = cycle-status/make-task on an already-committed block.

**Why mock-ups (not chat ASCII):** Taylor found ASCII previews hard to evaluate and asked for rendered harness-deck mock-ups in the real theme — now a standing preference ([[feedback-visual-mockups-harness-deck]]).

**Open mechanic to resolve in the build (do NOT guess):** today EVERY `#tag` auto-lifts to a pill, so there's no "literal tag" state. The ⌘↵-kept literal `#tag` needs a persistence + render distinction from a lifted chip (e.g. an escape/marker the parser leaves as text). Also needs a tag→color source (read the tag page's `color` frontmatter; deterministic palette fallback). Build = milestone-sized; likely increment it (colored pills first, then the gesture/literal-state).

---

### 2026-06-06 — /g splits via a Graphite-native pane renderer (GrLayoutTree), NOT by adopting v5 BufferShell

**Decision (Taylor, presented the fork):** make vsplit/hsplit render on `/g` by building a Graphite-native recursive pane-tree renderer (`graphite/shell/GrLayoutTree.svelte` + `GrLeaf.svelte`) that tiles the EXISTING `Gr*` views (GrDaily/GrPage/GrInbox/GrAgenda) across `tab.layout`. Do NOT take the handoff's literal "swap the single-pane `view` conditional for `<LayoutTree>`" path.

**Why:** a 5-agent mapping workflow proved the literal swap is not the clean change the A3 note assumed — `LayoutTree` mounts the v5 `BufferShell`, which renders the v5 NoteRenderer/ambient registry, NOT the Graphite views. That swap would (a) regress the default daily (empty pageId → BufferShell "empty pane" placeholder, today's journal lost), (b) drop GrPage's References/Properties **side pane** + title head, (c) replace GrAgenda's Mon–Fri time-grid with the v5 day-list, (d) change GrInbox's look + lose Process-all/snooze, and (e) need `--v4-*` tokens re-scoped into `.gr-root` (BufferShell is styled in v4 tokens absent under Graphite) + a `.gr-main` root-leaf flex rule + a shared default-today seed that also alters /v4. That trips A3's own "REVERT if any view regresses" guardrail. The Graphite-native renderer reuses the already-wired split state (`vsplit`/`hsplit`/`setRatio`/`moveFocus` already mutate `tab.layout`; the leader/⌘K/`:` already reach them — only the renderer was missing) so it's contained to `/g` with zero shared-state/token changes and zero view regressions.

**Shape:** `GrLayoutTree` mirrors `components/v5/LayoutTree.svelte`'s split/resizer/drag algebra (Graphite tokens) but mounts `GrLeaf` per leaf; `GrLeaf` runs the same per-buffer view routing the shell used (empty pageId → daily, so the empty-pane regression is structurally avoided). Focus accent is split-only (`showFocus`) so a lone pane is pixel-identical to before; click focuses the leaf; Ctrl-W h/j/k/l ports v4's `moveFocus` so splits are keyboard-usable. Browser-QA'd all four views + vsplit/hsplit/nested/close/focus-nav on a fresh mosaic, no console errors.

**Open follow-up (intentionally NOT done):** unifying `/g` onto the v5 BufferShell architecture (dropping the bespoke `Gr*` views) remains a deliberate future call — this preserves the Graphite presentation for now.

---

### 2026-06-06 — Tasks query stays tag-strict; existing tasks get a one-time #Task backfill (not a query widen)

**Decision (Taylor, product-tested):** the Tasks query keeps its strict definition `kind:block tag:Task -status:done` (`system-widgets.ts:50`). Do NOT widen it to `tag:Task OR has:status`.

**Why:** widening makes "any block that ever got a status" a task, flooding the Tasks view with blocks never meant as tasks (and depends on `OR`/`has:` query-grammar support that's unverified). Strict + explicit #Task is the cleaner semantic.

**Consequence:** existing status-bearing blocks predating the auto-tag logic lack #Task and won't appear. Remedy = a one-time #Task BACKFILL (scoped in current-state): scan the mosaic, add `tags:: Task` to any block with a `status::` but no Task tag, dry-run first, Taylor runs it on his real mosaic. Going forward, the `hasTask` auto-tag fix (`8d02625`) tags new status-cycles. Separately, `displayChipsFor` now falls back to `tag_properties` (`d9d30ee`) so priority/deadline render as chips without per-tag `display_chips`.

---

### 2026-06-05 (b) — Loro container-overwrite hazard: nested property containers must be seeded into shared history

**Finding (surfaced during P1.4 implementation by an adversarial test that correctly failed first):** Loro derives a child container's id from the op that creates it. Two peers that each create a nested container — a multi-value `LoroList` or a text `LoroText` property, **or** the per-block `props` map / `prop_keys` list itself — at the SAME map key, concurrently, for the FIRST time, mint RIVAL container ids → on merge one branch OVERWRITES the other (the loser's contents are lost). Union / char-merge only holds once the container already exists in SHARED history before the peers diverge.

**Impact:** a genuine multi-device data-loss vector for the exact case this milestone targets — two devices first-adding a tag, first-setting a text property, or first-setting ANY property on the same block before either has synced. Scalar property VALUES are safe as long as the `props` MAP is shared (per-key `insert` is LWW). The architectural review (2026-06-05) assumed nested containers merge; this is the one place that assumption is false.

**Decision (direction, to be finalized in the P1.9b convergence-design pass):** eagerly seed `props` + `prop_keys` on a block node at CREATION (and the page-root containers at note init) so the common path operates on a shared map — fixes the scalar + "any first property" case. The narrower per-key list/text first-touch hazard + the migrate-on-apply case (P1.6 creates props containers on EXISTING blocks → two devices migrating one block concurrently mint rivals) are resolved together: candidates are deterministic/seed-on-define container ids, an authoritative single-writer migration, and/or a rival-container reconcile folded into the disjoint-twin heal (P1.9). **P1.9b gates P1.6.** Until resolved, tests honestly seed the container on a shared base to prove union (they do NOT claim first-touch union).

---

### 2026-06-05 — Properties + types milestone: structured-first typed property containers

**Product decisions (brainstormed w/ Taylor; spec `phases/2026-06-05-properties-types-spec.md` + arch-review addendum):**
- Scope = the full Logseq-DB/AnyType property/type system, phased; **foundation-first** order.
- Editing = **Hybrid**: properties are CRDT data, edited as `key:: value` text OR chips/`/p` (the text line is a *view* over the container, mirroring block-text↔`text_seq`). Plus a **new-entity confirmation guard** (did-you-mean near-match) so a typo'd Enter / missed autocomplete stops minting junk properties/tags/pages. Globally toggleable.
- Config UI = all three surfaces (entity page canonical · inline drawer gear · ⌘K modal) over one shared registry foundation.
- **Multi-value AND node-references both ship** this milestone — supersedes the earlier "defer multi-value" note in `project_structured_first_crdt_truth`; multi-value also fixes the cross-device tag-merge LWW clobber.

**Architectural resolutions (7-lens code-verified review):**
- **Dedicated property ops** `BlockPropertySet`/`PagePropertySet` (`PropOp = SetScalar|SetText|AddToList|RemoveFromList|Clear`), NOT a `BlockUpsert.properties` field — a field still rides the stale-base whole-block text-diff → per-key LWW, defeating the multi-value union. `PropScalar = String|i64|f64|bool` (plain Rust, not `LoroValue` — the wire stays decoupled from the CRDT lib version).
- **Container topology:** `props` LoroMap — scalars = **primitive** `LoroValue` (zero sub-containers, snapshot-budget critical); text → nested `LoroText`; multi/node → nested `LoroList` — plus a **mandatory `prop_keys` LoroList** for deterministic materialization (LoroMap key order is unspecified). Always `get_or_create_container`, never `insert_container` at an existing key.
- **Failure policy = coerce-and-keep, surface-in-UI, NEVER reject** at write/index. Forced by CRDT-is-truth: peers exchange opaque deltas, so a server reject is unenforceable and would desync. Validation is a view.
- **Migrate-on-APPLY** (not just read): strip `key:: value` from an incoming `BlockUpsert.text` into `props`, write prose-only `text_seq`, one idempotent commit. Flag-gated **default-OFF**, flipped only after the WHOLE fleet (incl. old iOS FFI) is read-capable — an old build imports the new containers without error but renders them away (highest-severity loss). Keep emitting `key:: value` lines in the materialized view during transition; dual-read forever.
- Page-prop indexing NOT in Phase 1 (index stays downstream of materialized markdown).

**Why it matters:** the review caught that the disjoint-twin heal, the block pruner (`prune_bare_leaf_blocks`), the NoteUpsert reseed, and the set-property route would each have re-introduced the very data-loss this milestone exists to fix — all folded into the spec's 14 blocking issues before any code was written.

---

### 2026-06-02 — Block text is a nested LoroText (not a map register); discriminator scoped to disjoint twins

**Decision:** Store each block's text as a nested **`LoroText`** sequence CRDT (key `"text_seq"` on the tree node's meta map), written via `get_or_create_container` + `LoroText::update(whole_text)`. Clients keep sending WHOLE block text; `OpPayload::BlockUpsert.text` stays a `String`; diff.rs / FFI / note_tree / web / iOS / relay are all UNCHANGED. The engine alone converts whole-text → splices via `update()` (Myers diff). Lazy migrate-on-write: a new key, dual-read (`read_block_text` prefers `text_seq`, falls back to the legacy `text` register), legacy register never written again.

**Why:** This was the 4th distinct multi-device data-loss vector — a block's text being a Loro **LWW map register** meant two peers editing the SAME block concurrently lost one side (higher-(lamport,peer) whole-text write wins). A LoroText merges concurrent splices, so the WS/relay path merges text "for free." Approach (b) — engine-only, whole-text→splice server-side — was chosen over (c) (clients emit real character splices) because it sidesteps the hard constraint that iOS `record_note_diff` re-authors whole blocks from markdown and has no per-keystroke delta at the FFI. (c) is deferred for cursor-accurate same-region merges.

**Discriminator scoping (the subtle part):** the WS-apply Part-C discriminator (`peer_genuine_block_changes`) used to scan `JsonMapOp::Insert{key:"text"}` ops — dead once text is a Text container. Key realization: on a SHARED Loro lineage the LoroText merge makes raw-import SAFE (the old "stale re-assertion clobber", case a, is obviated — a peer's frame can't delete the server's newer inserts). So the discriminator + heal are now scoped to **disjoint TreeID twins only** (gated `twin_bids.is_empty()` early-return; the `server_block_text_history` op-replay runs only when a twin exists). Shared-lineage blocks defer entirely to Loro's merge and are never force-healed.

**Necessary-not-sufficient:** true char-merge only holds on a SHARED base lineage. Disjoint twins hold two independent LoroTexts Loro can't merge — so this fix sits on top of the shared-base bootstrap (D/#149). Migration hazard: an OLD-FFI device writing the legacy `text` register is shadowed once the server migrates a block to `text_seq` → devices must update before resuming cross-device edits. Spec: `phases/2026-06-02-block-text-crdt-spec.md`. Built subagent-driven, two-stage reviewed (spec✅+quality-APPROVE), proven by engine convergence + FFI round-trip + e2e real-socket merge tests.

---

### 2026-05-30 — Defer the HA-relay sync redesign until after Loro/RTC; bypass it locally for now

**Decision:** Do NOT keep patching the current relay path. Park a full sync-relay redesign until the Loro migration + real-time-collab (RTC) work is done — at which point we'll likely need an RTC server/proxy anyway and would redesign the transport regardless. For now, **bypass the relay** so the Graphite redesign can be tested locally: relay disabled in the Mac mosaic's `config.toml` (`[sync.relay]` commented out; backup at `config.toml.relay-bak`), making the Mac a standalone local server. Verified: a PUT persists, survives past the old 5s poll window (no inbound-clobber), and hits disk.

**Why:** A real bug surfaced while installing the Graphite build on the iPhone — cross-device edits reverted on both web + iOS. Root cause: `ai-business` (1.3 MB markdown → ~5 MB Loro snapshot ≈ 7 MB on the wire) exceeds the HA relay add-on's `max_body`, so every outbound PUT 413'd while the Mac kept pulling stale inbound ops over fresh edits. We fixed the *code* (binary `--max-body` default → 16 MiB, client chunk budget realigned, first-broadcast ships a compact snapshot instead of full deleted-history; commits `08e941b`, `0c97b92`). The live HA add-on still enforces its saved 1 MiB until its Configuration-tab `max_body` is raised — and rather than chase that, Taylor chose to stop investing in this relay shape. A single Loro doc can't be split across envelopes; the proper long-term answer (intra-doc chunking, or an RTC-aware transport) belongs in the post-Loro redesign, not a patch.

**Trade-off:** No cross-device sync while bypassed — the phone won't see Mac edits and vice-versa. Fine for now: testing the Graphite redesign only needs one device + persistence, which the standalone local server gives. Re-enable by restoring `config.toml.relay-bak` (or raising the HA add-on `max_body` to 16777216 in its Configuration tab) and restarting the server.

**Status of the deferred work:** Code fixes are committed and correct (relay binary + deploy configs all default to 16 MiB now). The remaining items — raising the live HA add-on limit, the coordinated live-data reseed + iPhone re-bootstrap, and intra-doc chunking — fold into the future relay/RTC redesign. See [project_relay_413_blocks_sync](../../.claude/projects/-Users-tfinklea-git-tesela/memory/project_relay_413_blocks_sync.md).

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

## 2026-05-28 — Loro authoritative-writer architecture (relay-payload + flag work)

- **Authoritative mode = bare `LoroEngine` as `AppState.sync_engine`** (no DualEngine, no SqliteEngine). Rationale: reads go through `FsNoteStore` (disk), NOT the engine, so once LoroEngine materializes `<mosaic>/notes/<slug>.md`, the web read path works unchanged — no reader-swap, no SqliteEngine-write suppression needed. This is also the flag-day end-state (SqliteEngine deleted), so we build toward it directly rather than threading a suppress-flag through DualEngine.
- **Relay payload v2 = 4-byte magic `TLR2` + postcard(Vec<LoroDocUpdate{doc,update_bytes}>)**, NOT a 1-byte version. A 1-byte version collides with the legacy bare `postcard(Vec<EncodedOp>)` (a 2-op batch starts with varint `0x02`). The magic is collision-proof: a v1 payload decodes to `None` on the v2 path and is skipped, never mis-applied. Index doc is NOT broadcast — each peer rebuilds it locally (self-healing index).
- **Engine selects the relay payload format via `SyncEngine::uses_loro_relay_payload()`** (trait method, default false; LoroEngine returns true when `materialize_dir` is set i.e. authoritative). The relay `tick()` branches on it: Loro path uses `produce_relay_updates`/`apply_relay_updates`; legacy path unchanged. DualEngine/SqliteEngine keep the v1 path.
- **Broadcast cursors persisted inside LoroEngine's snapshot dir** (`_broadcast.bin`), not in RelayState — keeps relay-state and tick code untouched and the cursor concern encapsulated. Re-broadcasting full state after a lost cursor is idempotent (Loro merge), so this is an optimization not a correctness requirement.
- **Multi-device bootstrap**: independent disk-reseed on each device mints non-merging Loro nodes (the flashing-reintroduction trap). So exactly ONE device reseeds from disk (canonical); others bootstrap by importing full state from the relay (empty broadcast_cursor → produce exports full state). Two-Mac test: Mac A reseeds from disk + authoritative; Mac B starts empty + imports from relay.

## 2026-05-29 — Cutover adversarial review dispositions

3-reviewer + per-finding-verification workflow on the relay-payload + authoritative diff (`b7e3c0f..HEAD`) confirmed 8 findings. Fixed 3 common-path (commit `1c64d52`→fix commit): cursor-before-send (lost delta on failed PUT), decode-Err stalling inbound, NoteDelete-orphan when display_alias=None. 

**Deferred (known v1 limitation): slug-rename orphans/duplicates (#7, #8).** note_id = `blake3(slug)[..16]` everywhere (server `stable_uuid_from_slug`, reseed, snapshots — verified identical). So `reseed_from_disk` recomputes the SAME id as any existing snapshot → reconciles, no duplicate; and NoteDelete's op-carried slug matches the file. These only break when a note's **slug changes** (rename): reseed would mint a new id for the renamed file, and NoteUpsert materializes the new `<slug>.md` without removing the old one. Tesela has no rename flow exercised in the cutover and the common bootstrap is correct (verified live on 512 notes). Post-cutover fix: track prior slug per note in root meta, remove the old file on slug change; reseed should resolve id via a slug→id index rather than recompute. Not flag-day-blocking.

## 2026-05-29 — Blank blocks + headings dropped (Loro render policy)

Taylor: "I want blank blocks/headings dropped." So the flat-block CRDT does NOT preserve:
- **Headings / non-bullet body lines** — already absent (the flat-block model never captured them; this is why `2026-05-17`'s bare `# heading` drops on cutover — now confirmed DESIRED, not a regression). No heading-modeling will be added.
- **Blank blocks** — empty/whitespace-only bullets are transient editing artifacts; `note_tree_from_doc` (the single render chokepoint feeding materialization + the comparison surface) now skips `fb.text.trim().is_empty()`. The Loro tree may still hold a transient empty node mid-edit, but it never materializes to disk or syncs as content. Reverses the old Phase-2.2 "blank blocks survive symmetrically" behavior for the Loro era.

## 2026-05-29 — Web daily-editing bugs (post-authoritative-cutover)

Three symptoms (`:daily`→wrong day, empty days un-editable, click-to-add broken) had two root causes, both surfaced by the cutover:
1. **Blank-block drop** (the 2026-05-29 experiment) made empty days zero-block; the web outliner needs a trailing empty bullet as the editing surface (`JournalView.ensureTrailingEmpty`). REVERTED — blank bullets are kept (the editing surface, like Logseq). Headings still drop (flat-block model never modeled them).
2. **Reseed clobbered file mtimes** — the authoritative reseed rewrote all 513 files at boot → all mtimes ~equal. `FsNoteStore::list` sorted by mtime then `limit`, so the journal's `limit:60` daily query returned the wrong 60 → recent days (with content) rendered as false "empty day · click" synthetics. FIX: `list` sorts by **title (date) descending**; reseed-proof; only the journal's bounded query is affected (other callers fetch all + re-sort).

Also: `ensureTrailingEmpty` dedup regex didn't account for the stamped bid marker (`- <!-- bid:… -->`) → appended a fresh empty bullet every mount (accumulation); fixed by stripping the bid before the empty-bullet test. Daily template now seeds `- ` (blank block) not `# heading`.

**Known remaining**: genuine gap days (date never created, no file) still show "empty day · click to add an entry"; rendering them as a no-click blank block needs create-on-focus (PUT doesn't upsert). Keyboard j/k nav into them already creates+focuses. Per "every daily should just have a blank block" this is the next piece. (Also: a stray note like 2026-05-26 can lack `tags: [daily]` in frontmatter → excluded from the daily list = a per-note data quirk, not the general bug.)

## 2026-05-29 — Loro flag-day: sole engine, op-wire deleted, LAN P2P retired

The cutover's destructive finish (`471d619`, `8ef366e`, `c626d25`). Decisions:

- **Loro is the sole sync engine; no fallback.** Deleted SqliteEngine, DualEngine, the dual-write path, the v1 op-wire (`encode/decode_op_batch` + `Vec<EncodedOp>`), and the `TESELA_LORO_DUAL_WRITE`/`AUTHORITATIVE` flags (Loro is unconditional; `TESELA_LORO_RESEED` kept for one-time canonical bootstrap). ~3.6k lines deleted. Convergence was already proven at 4 levels (engine, +wire, +AEAD+HTTP-relay, live web↔iPhone) before deleting the fallback.
- **Kept the `SyncEngine` trait (slimmed), did not drop it.** Single impl (LoroEngine) now, but the trait is the boundary the server's `Arc<dyn SyncEngine>` + the iOS FFI hold; keeping it is lower-risk than concretizing every call site, and leaves room for a test mock / future engine. Removed only the op-replay methods (`apply_changes`/`produce_changes_since`/`produce_local_authored_since`/`uses_loro_relay_payload`/`ProducedBatch`).
- **LAN P2P (peer_sync) data-plane RETIRED, not migrated.** Its op-replay pull ("give me your ops since cursor X") is fundamentally incompatible with Loro (no per-device op log to replay from an HLC cursor — Loro's unit is a per-note version-vector update) AND fully redundant with the relay spine for correctness (the relay broadcasts every update to every peer, so disabling LAN P2P loses no convergence — it was a pure latency optimization). `produce`/`receive_envelope` return 501 (loud, not a silent empty-sync); the daemon is a no-op; pairing/discovery stay live. Reimplementing LAN P2P over the Loro relay-update protocol is a deferred optimization (the transport/pairing scaffolding is kept for it). Matches the relay-as-spine / P2P-as-LAN-optimization decision.
- **ai-business dedup = frontmatter-only root meta.** LoroEngine stored the full markdown on root `content`, duplicating the body (already in the tree) and doubling every snapshot — a 1.3 MB page → 5.13 MB snapshot, over the 5 MB relay limit. Now stores only the verbatim frontmatter; full markdown is reconstructed on read (`doc_full_markdown` = frontmatter + rendered body, == what materialization writes). Backward-compatible (pre-dedup docs fall back to their stored `content`). **The size win lands only on FRESH docs** — Loro snapshots are cumulative (a delete is a tombstone, not a reclaim), so existing docs keep the bloat until a reseed rebuilds them. DR drill measured 5.13 MB → 2.58 MB after a fresh reseed.
- **DR procedure (canonical):** the mosaic's `notes/*.md` ARE the source of truth; `.tesela/loro/` is a derived cache. Recovery = restore `notes/` → boot one device with `TESELA_LORO_RESEED=1` → Loro rebuilds. Validated on an isolated copy (514 notes, no relay config → no live contact).
- **Live data reset deferred (user-coordinated).** Making the dedup land in production requires wiping `.tesela/loro/` + reseed AND wiping/re-bootstrapping the iPhone's local docs (fresh-identity docs would otherwise merge-duplicate against the iPhone's old docs). Needs the device present; not done unilaterally. Until then the server runs on existing docs via the backward-compat fallback (ai-business stays unsynced, as before).

## 2026-05-31 — Multi-device convergence: shared-base bootstrap + dedup heal (the real RTC fix)

The "flashing"/revert the Loro migration was meant to kill came BACK in multi-device testing (iPhone+iPad+web). Root-caused (deterministic repro `crates/tesela-sync/tests/disjoint_history_revert.rs`): **Loro tree node identity is the internal `TreeID` (peer+counter), not our `block_id`.** iOS `recordNoteDiff` re-authors blocks from its own markdown into a per-note doc that never imported the server's doc as a base → each peer mints a DIFFERENT TreeID for the same bid → on merge Loro UNIONS the twins → `note_tree_from_doc` rendered both, and the web block-diff save updated one twin in nondeterministic FxHashMap order, leaving a stale ghost = "revert".

Decisions:
- **The real fix is a SHARED BASE, not better merging.** iOS now imports the server's per-note Loro snapshot (`GET /loro/notes/{id}/snapshot` → FFI `import_note_snapshot`) BEFORE its first local author of that note (`RelayTicker.bootstrapNoteIfNeeded`, gated on `noteVersion!=nil` so it runs once). After import, `recordNoteDiff`'s BlockUpserts resolve to the EXISTING server nodes (no rival TreeID) → true convergence. This is the activation of the long-planned VV catch-up, done as a simple HTTP snapshot pull rather than a new /ws req/resp protocol (simpler; reuses get_loro_index's shape).
- **dedup-by-block_id is a LOSSY heal, kept as defense-in-depth.** `dedup_twins_by_block_id` (render) + `tombstone_duplicate_twins` (import) collapse twins deterministically by **min-TreeID** — loro 1.12 exposes no per-text-update recency (block text is a LWW map register; `get_last_editor` returns only a PeerID, `get_last_move_id` tracks structural ops not text), so the survivor is stable but NOT necessarily the latest edit. This stops the visible duplication and the nondeterministic ghost, and retroactively heals on-disk corruption — but it can drop a concurrent edit on the losing twin. Acceptable because once the shared base lands, twins stop forming; dedup only matters for legacy-corrupted docs. **Implication for testing/ops: devices must start from a CLEAN sandbox** (bootstrap skips already-resident pre-fix disjoint docs; only the lossy tombstone touches those).
- **iOS relay coordinator GATED in hub mode (`RelayTicker.hubMode`), not deleted.** The cached pairing code kept the phones syncing to each other through the HA relay (shared engine handle) and re-injecting stale foreign-history ops — so disabling the relay on the Mac alone didn't isolate them. `hubMode` skips the coordinator while the /ws hub path is active; the cache is NOT cleared, so it's reversible.
- **Did NOT make TreeID deterministic from the bid.** Cleanest in theory (convergence by construction) but loro 1.12's public `LoroTree` API forbids caller-chosen TreeIDs (`create`/`create_at` mint `txn.next_id()`; target-id methods are `pub(crate)`). Forking loro = rejected (maintenance + risk).
- **WS frame cap was silently dropping big snapshots.** Full-snapshot-per-keystroke (pre-existing) could exceed iOS `URLSessionWebSocketTask.maximumMessageSize` (default 1 MiB) → silent drop + reconnect. Raised to 64 MiB. The real follow-up (#150) is to ship deltas not snapshots now that the base is shared.

Spec: `phases/2026-05-31-multidevice-converge-spec.md`. Built subagent-driven, two-stage review per task (E1/E2+B/D), repro red→green. Server rebuilt+restarted on the fix; Roshar reinstalled clean. Live multi-device round-trip = user's step (Sel/iPad pending connection).

- **Verify proxy-dependent browser paths IN A BROWSER, not just headless.** C2.3 collab editing shipped with a passing headless converge-check yet was 100% broken at runtime: `NoteDoc` bootstrapped its snapshot from `/loro/...` but vite dev only proxies `/api/*`→tesela-server (rewriting `/api` off), so the browser fetch hit the SPA 404 → empty doc → no binding → every edit silently fell back to the whole-text HTTP clobber path. The headless node check used an ABSOLUTE base (`http://127.0.0.1:7474`), bypassing the proxy entirely, so it never exercised the real path; and the `/loro/...snapshot` GETs in the server log were the iOS devices hitting :7474 directly (a decoy that made the path look exercised). Lesson: when a client path depends on the dev-server proxy/rewrite, a headless test with an absolute base is NOT evidence the in-app path works — drive it through the actual origin (two browser tabs via Chrome DevTools MCP). Also: the web client's server base is `/api` (`api-client.ts` `BASE_URL`); any new fetch must use that prefix. Fixed in `4c92d6a`.

- **Graphite shell is now the iOS default (`20920b7`), legacy behind `-legacy`.** The redesign owns the daily-driver views AND the entire collaborative-editing path (C1 splice editor + C1-inbound live-apply); the legacy `AppShell`/`DailyView` has none of it. The app had been defaulting to legacy with Graphite gated behind a `-graphite` launch arg / `tesela.useGraphiteShell` default that nothing in code ever set and no UI toggled — so tapping the icon gave the no-collab legacy shell. This silently invalidated the first sim verification pass (every C1 hook read nil because legacy was running). Flipped `TeselaApp` to default Graphite; legacy kept reachable via `-legacy` / `tesela.useLegacyShell` until the cutover removes it. Lesson for device/sim testing: confirm WHICH shell is running before trusting a collab test — legacy looks similar but has no collab wiring.

- **The web collab Loro binding must live in the route the USER runs — `/g` (Graphite), not `/v4` (legacy).** C2.2/C2.3 wired `openActiveNoteDoc` into `/v4/+layout.svelte`; my web↔web verification passed there. But the user runs the Graphite web shell at `/g` (`GraphiteShell`), which never opened the active NoteDoc → its editors fell back to HTTP block-ops and only saw changes on a full refetch (the "web drift" the user hit: iOS edits landed on the server but the web didn't live-apply until refresh). Fix `a930142` ports the `openActiveNoteDoc(focusedSlug)` effect into `GraphiteShell` (same buffer state + Loro-bound BlockOutliner/BlockEditor it already uses). Exact mirror of the iOS shell-split lesson: the collab wiring went into the redesign shell's sibling, and the default/used surface was the other one. Whenever wiring a client-side feature, confirm WHICH route/shell the user actually runs before declaring it verified.

## 2026-06-03 — Cloudflare Worker relay: conformance-as-shared-contract; structural per-group isolation

Built the always-on cloud endpoint of the encrypted-replica spine (`cloudflare-relay/`, commits `397fc30` + `348603a`). Decisions:

- **One conformance suite gates BOTH implementations.** Rather than write Worker-specific tests, `crates/tesela-relay/tests/conformance.rs` `spawn_relay()` now honors `TESELA_RELAY_CONFORMANCE_URL` — unset it spawns the in-process Rust relay; set, it runs the same pure-HTTP tests against any URL (`wrangler dev`). This makes the Rust suite the canonical wire contract for every relay implementation (the file header always intended this). **Wire parity is then proven empirically, not by inspection:** the Rust client signs each request and the Worker verifies the MAC — if the canonical-request format, body-hash, or status codes diverged by one byte, every gated test would 401. 19/19 green on both = byte-identical on the wire.
- **Extended the pre-existing committed scaffold, did not rewrite.** A May-25 scaffold existed (pre-Phase-1: ack-triggered GC, no snapshots, no rate limit, 5 MiB cap). Brought it to the current protocol rather than greenfielding — respects prior work, smaller diff, and the crypto layer was already Rust-correct.
- **One Durable Object per group (`idFromName(group_id_hex)`) → isolation is structural.** Each group's state (DO-SQLite + in-memory nonce LRU + IP rate counter) lives in its own DO instance, so cross-group isolation is free (different group = different SQLite, no shared table to leak across). Consequence: rate-limiting + nonce-dedupe are **per-DO (per-group)**, where the Rust relay's are global-per-process. For the conformance suite (single-group bursts) this is equivalent; for production it means an attacker spreading load across many group IDs isn't globally throttled by the in-DO limiter — CF's platform-level DDoS/rate protection is the backstop, and a global limit (native Rate Limiting binding) is a deploy-hardening follow-up. Documented in the Worker README.
- **Zero-knowledge preserved by storing opaque strings/bytes, never decoding.** The Worker stores `payload_b64` / `stream_id_b64` as opaque BLOBs and only ever echoes them; it never parses, transforms, or logs ciphertext or keys. The only crypto it performs is HMAC-SHA256 verify (the request MAC) + SHA-256 (body hash) — both native WebCrypto, no library. It never derives keys (the client deposits the `auth_key` at register) and never sees the `group_key`.
- **Body cap defaults to 1 MiB, overridable to 16 MiB for production** (`TESELA_RELAY_MAX_BODY`) — mirrors EXACTLY how the Rust relay is operated (default 1 MiB; production runs `=16777216` for large per-note Loro snapshots, per the relay-413 fix). The conformance harness uses 1 MiB (test_08 sends 2 MiB expecting 413), so the dev/test default matches; production deploy must set the override.
- **AUTOINCREMENT (Worker) vs MAX(seq)+1 (Rust) — the Worker is correct, and this surfaced a latent Rust data-loss bug.** The Worker's `ops.seq` is `INTEGER PRIMARY KEY AUTOINCREMENT` (never reuses a seq, even after compaction deletes rows). The Rust relay's `COALESCE(MAX(seq),0)+1` RESETS to 1 after a FULL compaction (all ops deleted), while the compaction watermark stays high — so a device fetching `since=watermark` would miss the resurrected low-seq op. Adversarial review caught this; the Worker is right, the Rust relay needs `MAX(MAX(seq), compaction_seq)+1` (task #195). Lesson: porting to a second implementation is itself a review pass — the divergence exposed the original's bug.
- **Conformance proves the happy/error paths; an adversarial review covers the rest.** The black-box suite can't see silent-coercion or unbounded-growth bugs. A skeptical review found four real Worker issues (fromHex NaN→0x00 device corruption; un-capped /ack DoS; MAC gate over-requiring device/group headers; unbounded nonce map) — all fixed, and the two with observable HTTP behavior (non-hex → 400, over-cap ack → 413) were locked into the shared suite (test_14/15) so neither implementation can regress.

## 2026-06-04 — Desktop app: Tauri-wrap `/g`, not a fresh SwiftUI Mac app

The hinge decision of the product roadmap (step 2). Settled after a two-Claude discussion + Taylor's own usage. Spec: `phases/2026-06-03-tauri-desktop-spec.md`.

- **Tauri-wrap the SvelteKit `/g` UI, NOT a native SwiftUI Mac app.** The real axis isn't "native feel vs reuse" — it's *which platform family the Mac joins and whether the web client is canonical*. `/g` is the most mature, hardest-won surface (CodeMirror+vim, ⌘K, leader, the Loro collab editor that took a whole marathon to get right). Tauri reuses it 100% → roadmap step 3 (markdown render, code blocks, vim, properties, widgets — the largest phase) builds ONCE for web+desktop. A SwiftUI Mac would extend the *iOS* app (SwiftUI+UniFFI), which is *behind* `/g` (no Loro collab, no tag system, no v5 chrome) → it's "build the web shell's feature set twice, starting from the surface that's behind." The native-feel cost is ≈0 for a vim user living in a controlled CodeMirror surface (the original March "go AppKit for the native text system" rationale is moot — the shell stopped leaning on browser-native text behavior). Taylor daily-drives the web shell → web is de-facto canonical, which answers the flip-question. Reversible: the FFI/iOS path is untouched; a native SwiftUI Mac shell stays shelved as a possible premium-native tier once "native" has a concrete definition.
- **Architecture: native window + a child `tesela-server` bound to LOOPBACK that serves BOTH the API and the static `/g` UI.** The webview loads `http://127.0.0.1:<port>/g` → API + UI same-origin → no CORS, and the UI's existing `window.location.host`-derived WS just works. Chosen over (a) Tauri-serves-frontend + cross-origin API (needs CORS + a WS-base injection) and (b) Tauri `invoke` IPC (rewrites the whole api-client). The only web change is the API base prefix, resolved at runtime via `runtime-base.ts` `apiBase()` = `window.__TESELA_API_BASE__ ?? "/api"`; the Tauri shell injects `""` (same-origin). Enabled by two facts: the web is a pure SPA (zero server routes → trivial `adapter-static`), and `tesela-server` is a self-contained Axum server (now with an optional `TESELA_STATIC_DIR` SPA fallback).
- **The embedded server is a LOOPBACK Loro-replica NODE, not a hub.** Binds 127.0.0.1 only; mDNS + relay + LAN-peer-sync all disabled in the embed (`TESELA_DISABLE_MDNS/RELAY/PEER_SYNC`). Cross-device sync will flow through the spine (relay/LAN), the same transport as iOS — this is synergistic with the spine, which is *trying* to demote the Mac from hub to equal client. The webview↔server HTTP is local UI plumbing, not a sync seam. This forces the right posture instead of fighting it.
- **Single-writer is the load-bearing data-safety invariant, and it must be ENFORCED, not documented.** Two `tesela-server`s writing one mosaic = Loro corruption (rival device_ids / HLC). The orphan-prevention design (parent-death watchdog, graceful reaping) only covers the embed's own child; the front door (double-launch, or app + standalone) was open. Enforced with an exclusive `flock` on `<mosaic>/.tesela/server.lock` held for the process lifetime (mirrors `tesela-backup`'s lock) — a second server fails fast with EWOULDBLOCK. Verified: a 2nd server on the same mosaic is rejected; the lock releases on death (even SIGKILL, via the OS). This was the adversarial review's CRITICAL finding — the watchdog/loopback design was sound but the invariant itself was unenforced.
- **Porting to a second deployment is itself a review pass (again).** Just as the CF Worker port surfaced a latent Rust-relay bug, the Tauri review surfaced: a parent-death watchdog spawn-race (fixed by passing `TESELA_PARENT_PID` + `kill(pid,0)==ESRCH`, not just getppid-change), permissive CORS on the now-same-origin embed (gated off when embedded — DNS-rebinding vector), six frontend fetches hardcoding `/api` (broke voice/delete under the injected `""` base — routed through `apiBase()`), and two false "`/server/restart` doesn't inherit env" doc comments that invited a `0.0.0.0` bind regression (corrected). All fixed before commit.

## 2026-06-10 — Block deletes are explicit-only; NoteUpsert can never remove OR resurrect a block

Product-test failure (iOS `.relay` deletes reverting on the deleting device + never reaching the desktop) traced to engine seams, not the diff path — `record_note_diff` was already emitting explicit `BlockDelete`s. Fixes `ddc84ba` (Rust) + `54e80ab` (iOS). Decisions:

- **NoteUpsert apply is now a NON-destructive per-bid reconcile on every engine** (`loro_engine::reconcile_tree_to_blocks`): in-place text/indent/parent heals (lineage + containers preserved), create only never-seen bids, **deleted-wins** (a bid with a tombstoned node is never re-created by a whole-content upsert — authors never reuse bids, so a genuine re-add always has a fresh id), absent live blocks untouched. The old destructive clear+reseed was gated "server-only" on `materialize_dir`, but post-cutover EVERY engine has it set → the gate was vacuous and any stale full-content NoteUpsert resurrected deleted blocks, deleted peer blocks (vector #2 reopened), and re-minted TreeIDs (the disjoint-twin factory). Block REMOVAL now flows ONLY through explicit `BlockDelete`. Consequence: `TESELA_LORO_RESEED` boot reseed no longer removes doc-only stale blocks (acceptable for a recovery tool; deletes-by-file-absence were the clobber vector).
- **BlockDelete is bid-level: it tombstones EVERY live node carrying the bid.** Wild docs hold same-bid twins (disjoint-lineage residue) the renderer hides via `dedup_twins_by_block_id`; deleting only the first match left the survivor rendering → the delete silently reverted on the next materialize.
- **`record_note_diff` first-author path records a full NoteUpsert** (prev content empty → the note was never materialized on this device). A doc created purely from block ops has no `root.slug` → "cannot materialize — no slug" → no file, refresh shows nothing, peers get a slug-less doc. Safe now that NoteUpsert is non-destructive; also heals resident-but-slugless docs on the next writeback.
- **iOS never AUTHORS a placeholder-only daily** (`MockMosaicService.shouldSuppressPlaceholderAuthoring`): the editable-row bare-empty block, pushed as a fresh daily's first synced state, unioned ABOVE the peer's content in the morning race (the "extra empty block at top of Today"). While today's daily has no local file, splices route through the whole-content writeback (a splice can't CREATE a block; an engine-miss splice silently dropped the keystroke).
- **Why web→iOS deletes worked while iOS→web didn't:** web deletes hit the server's explicit `DELETE /notes/{id}/blocks/{bid}` (a direct BlockDelete); iOS deletes ride the whole-content writeback, whose reverts were exactly the seams above.
- **Leader which-key: a command must NOT be a leaf at a bucket-root key** (e.g. chord `[","]` while siblings use `[",", x]`). `buildChordTree`'s leaf+branches path (`leader-tree.svelte.ts`) then emits TWO sibling nodes with the SAME `key` (a leaf + its `…` subtree), and GrLeaderOverlay's keyed `{#each (node.key)}` throws Svelte `each_key_duplicate` → the ENTIRE leader overlay renders nothing on every open. Found by browser-QA 2026-06-16 after Phase B homed the settings panes under `,` but left `general: [","]` (fix `bffcf05d`: general→`[",", "g"]`, a pure `,` config bucket). Bucket keys (g/w/b/n/i/p/v/a/t/,) are PURE buckets — every command lives at depth ≥2 under one. Defense added: GrLeaderOverlay keys its each by `key+label` so a future collision degrades instead of crashing. **The node unit tests CANNOT catch this** — `leader-tree-real.test.mjs` can't import `v4/commands.ts` ($lib aliases, KNOWN_LIB_FAILURES) so the real settings chords never enter the registry under test; svelte-check can't see a render-time keyed-each duplicate. Only a live leader-open surfaces it → browser-QA is mandatory for leader/chord changes.
- **Keyed `{#each}` over command/chord trees must use a RENDER-UNIQUE key, never label- or key-alone.** The leader fix (bffcf05d) was only the first instance; the pre-merge sweep found the same `each_key_duplicate` crash class on ChordMenu's slash value-picker (`ChordMenu.svelte:451`, keyed by `path+label` which collapses to label in filterMode) — duplicate user-authored `select`/`multi-select` choices (`status:: [todo, todo]`, or case variants `[Open, open]`) crash the picker. Fixed (d8b26944): filterMode each → `idx + key + label` (idx guarantees uniqueness); the sibling non-filter each → `key + label` (mirrors the leader overlay). Rule for any future chord/verb/property menu: key by something the data cannot duplicate (include the array index).
- **The slash `Ctrl/⌘+letter` accelerator is CASE-SENSITIVE** (`ChordMenu.svelte:305`): `⌘t` → Task (slashKey `t`), `⌘⇧t` → Tag picker (slashKey `T`). The original case-folded match shadowed the second of any case-distinct sibling pair (Task/Tag, Status/Scheduled). Exact-case match first, case-insensitive fallback only when there's no exact hit — mirroring the leader's case-sensitive chord match (chord keys are deliberately case-distinct per chord-keys.ts).
- **`:peek <renderer>` arg-drop + open→toggle is INTENTIONAL, not a regression.** Phase D folded the hardcoded peek/graph colon builtins into the registry; the registered `peek` command toggles (consistent with ⌘I) and ignores a renderer arg. The redesign plan L821/L822 anticipated both deltas ("confirm acceptable in browser-QA"; arg-drop "likely fine — no UI advertised it") and they were accepted. The old `:peek <kind>` affordance was undocumented (no argPrompt, no UI, no test). Don't "fix" it back without a deliberate decision.
- **OPEN — leader `i`/`p` buckets can't run editor verbs (no editor context in the shell).** GraphiteShell's `commandCtx` has no `editor` field, so leader-dispatched editor commands no-op (`Space→i→h` silent; `Space→p` doesn't render because editor.property is the only p-chord and it's surface:'editor'→slash-only). Slash handles insert/properties fully. Not a regression (these weren't on the leader on main). Decision pending from Taylor (harness-deck `leader-ip-bucket` ask): wire leader→editor (dispatch to the focused BlockEditor's ctx) vs. make insert/properties slash-only and trim the i/p buckets.
- **Leader→editor wiring = a document-event bridge, NOT a shared editor ctx (2026-06-17, `9c7983ba`).** `Space → i/p` editor verbs run on the focused block via `tesela:run-editor-command{id}` (leader-tree `leafAction` dispatches for any `category:'editor'` leaf), handled by the focused BlockEditor — mirroring the shipped `g f` follow-wiki bridge. The shell can't build a `SlashContext`; it only carries `ctx.editorFocused` (a presence bool from the new `focused-editor` store) so `available()` keeps editor commands in the leader tree. surfacesFor adds 'leader' to editor commands with a chord. Do NOT try to thread a live editor ctx through the shell — the event bridge is the pattern.
- **Leader→editor handler MUST gate on `view.hasFocus`, never the `focused` PROP (2026-06-17).** The `focused` prop is PER-OUTLINER (`focusedIndex === vi`); a journal stack / split / drawer has MULTIPLE `focused===true` BlockEditors at once (each outliner auto-focuses index 0 on mount — same race that moved vimCtx onto DOM focus at BlockEditor ~1553). A `focused`-gated bridge → one `Space→i→h` mutated block 0 of EVERY stacked day. DOM focus is singular, and cm keeps it while the (capture-phase, non-input) leader overlay is open → `view.hasFocus` uniquely picks the block the user is in. Caught by 8-agent adversarial review + browser-QA (heading landed on the wrong pane's block). The `focused-editor` STORE is likewise driven by the cm focus/blur DOM handlers, not the `focused` prop (the prop is stale-after-click-away — `onblur` is a no-op + focusedIndex isn't nulled).
- **The leader editor context is WHOLE-BLOCK (before = entire block, after = ""), not caret-split (2026-06-17).** The editor verbs assume the slash invariant: the caret is pinned at the `/` trigger end, so `before` = full pre-trigger text and `after` = "". They then do `before.trim() + after` (heading) or `before.trimEnd() + "\n<propline>\n" + after` (collection/query/property). From the leader the caret can be MID-BLOCK; a caret-split context (`before=slice(0,cursor)`, `after=slice(cursor)`) merged words (`"hello world"`@6 → `"# helloworld"`) and spliced property lines into the middle of the block. Fix: `buildSlashContext(leaderMode=true)` sets `before = doc`, `after = ""` → every verb is a clean caret-agnostic prepend/append. (First attempt — `after=slice(cursor)` to "preserve the tail" — preserved text but introduced the mid-caret splice; the correct fix restores the verbs' `after===""` invariant.) Caught by adversarial review, not unit tests (which used end-of-block carets).

## 2026-06-19/20 — APNs instant-sync: the non-obvious gotchas

Built content-available APNs silent-push so an edit on one device wakes the others (P3; instant cross-device sync). The code was right early; everything *around* it was the work. Durable learnings:

- **A TestFlight build's APNs token is PRODUCTION, regardless of the `aps-environment` string in the entitlements file.** `scripts/ios-testflight.sh`'s `ExportOptions.plist` uses `method: app-store-connect`, which RE-SIGNS the dev-signed archive with the Apple **Distribution** cert → the shipped build is `aps-environment=production`. So the relay's `APNS_HOST` must be **unset = production** (`api.push.apple.com`); only an Xcode-direct dev/ad-hoc install needs the sandbox host. I got this backwards first (told Taylor "set sandbox") → would have been guaranteed `BadDeviceToken`. Verify the SHIPPED entitlement with `codesign -d --entitlements :- <archive>/…/Tesela.app`, not the `.entitlements` file.
- **The CF Worker relay MUST set `TESELA_RELAY_MAX_BODY = "16777216"` (16 MiB) in `wrangler.toml`.** The 1 MiB default `413 Payload Too Large`s real deposits (single-note Loro snapshots reach ~7 MiB — the ai-business note). This is the SAME relay-413 jam the Rust self-host was already bumped for; a freshly-deployed relay silently reverts to 1 MiB. The 413 also blocks APNs downstream (the failed tick never reaches token registration) — so "push never arrives" can really be a body-cap problem. Trust the device's `Sync error` text over theories.
- **The iOS devices use `pairing.relayUrl` (a self-host `tesela-relay` / HA add-on), NOT the CF Worker by default.** `desktop.toml relay_url` → `TESELA_EMBED_RELAY_URL` → the embedded server → handed to iOS at pairing. To switch relays: edit `desktop.toml relay_url`, restart desktop, **re-pair** iOS. A `wrangler tail` that's SILENT during a deposit means the devices aren't on that Worker. (This is why the HA-relay APNs port `#74` exists — but the **decision is CF Worker NOW** (deployed, public HTTPS, zero-knowledge-verified); HA parked, port + add-on committed and ready.)
- **The Graphite shell presents `GrSettingsView`, not the legacy `SyncSettingsView`.** A diagnostic added to the latter is invisible. (Burned a build cutting the "APNs push" status row into the wrong view.) Same trap as any iOS settings change — the live shell is Graphite.
- **Prune dead APNs tokens.** A reinstalled device gets a new `device_id` → a new row, leaving its old token pushed (and `BadDeviceToken`-failing) on every deposit forever. `sendApnsBackgroundPush` returns delivered/dead/failed; the deposit path deletes `dead` (HTTP 410 / BadDeviceToken). Both relays.
- **Relay is zero-knowledge — verified adversarially** (`relay-zk-verify`): note content is XChaCha20-Poly1305 sealed client-side under the GroupKey (which never leaves devices); the relay holds only the HKDF-derived MAC auth_key (one-way, can't decrypt) + ciphertext. The push carries no content. Residual: the snapshot `stream_id` is a stable per-note hash (relay can count notes / cadence, not read them) — a known `TODO(privacy)`.

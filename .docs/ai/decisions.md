# Architecture Decision Records

Concise log of non-obvious decisions. Newest first.

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

# 2026-07-14 — Adversarial audit report (Fable orchestrating; Sol/Terra/Qwen/Opus/Sonnet fleet)

Spec: `2026-07-14-adversarial-audit-spec.md`. Read-only audit; no source changed. All findings verified against source before filing.

## Method + what it cost
6 Sonnet mappers (subsystem dossiers) → 5 peer reviewers on inlined read-only dossiers (Sol@max architecture, Terra@xhigh sync, Qwen@xhigh ×3 core/iOS/web, GLM stalled) → Opus deep-dive on relocation + Sonnet audits (server silent-failures, iOS verification, properties/widgets) → Fable verified every claim against source, deduped vs 112 open beads.

**The verify pass earned its keep**: 3 confident findings REFUTED (Terra's batch-framing overflow; Qwen's iOS JQL "match-everything"; a background-flush race). The iOS JQL one was a **stale doc comment** (`LocalQueryEngine.swift:36-41`) that misled *two* independent reviewers AND my own mapper — full JQL parity has been implemented since 2026-06-15 and is conformance-pinned. Killing that comment is filed (tesela-9xd).

## The 6 findings that matter most (all verified, all new)

1. **tesela-73b (P0) — relocation boot recovery is fail-CLOSED: one unrecoverable intent permanently bricks engine open.** `loro_engine.rs:586` calls `recover_persisted_relocations().await?` inside engine open; that fn `?`-propagates the first failing intent (`relocation.rs:2429`); and `relocated_bids_have_only_allowed_owners` returns `false` for a bid owned by *nobody* (`.unwrap_or(false)`, `:1345`). So a failed move whose blocks are later deleted ⇒ the engine never opens again. On iOS the record is in the app sandbox ⇒ **delete the app, lose unsynced data**. This is the *only* fail-closed path in an engine whose every other risky step fails soft (poison probe SKIPS; import plan degrades). Zero test coverage of an unrecoverable boot.
2. **tesela-fdd (P1) — the relay can cross-write your notes.** Snapshots are sealed under a **group-only AAD** (`transport/relay.rs:521-543`); `stream_id` travels verbatim and is never bound in. `catchup_from_snapshots` (`sync_relay.rs:1084-1094`) routes purely on the relay-supplied stream_id and ignores `snapshot_seq`. A buggy or hostile relay (Cloudflare = a third party) can serve note A's snapshot as note B; the AEAD tag verifies and B imports A's content. **This violates the stated topology lock** ("relay = zero-knowledge mailbox, never an authority"): today E2E gives confidentiality, not integrity-of-routing. Cheap fix — the opener *knows* the stream_id it asked for.
3. **tesela-zip (P1) — the "recovery path of last resort" is unreachable.** On terminal apply failure the tick preserves the envelope from GC (`catchup_since_seq`) *and advances the cursor past it*; grep proves nothing ever re-reads it — recovery is snapshot-only. Non-adversarial trigger: `put_snapshots_chunked` **skips** any single stream too big to deposit (`skipped_streams`) — i.e. exactly the big-note class — so a big note + one transient 5×-failed apply = permanently stale on that device, with the data sitting on the relay, un-GC'd and unreachable.
4. **tesela-9ut (P1) — the relocation splice guard is dead code.** `spliceNoteBlock` checks `isReserved(slug)` but the barrier's only two `reserve()` call sites key it by **note UUID**. The domains never intersect ⇒ the guard can never fire ⇒ keystrokes splice the CRDT doc *during an active move*. Compounds with **tesela-flr**: the retry/boot-recovery path then judges the destination "incomplete" and re-authors it from the captured snapshot, **destroying the keystroke**.
5. **tesela-h8m (P1) — iOS tick timeout wedges sync.** The 25s watchdog abandons the tick and drops the coordinator but **never releases the admission lease**; there is no expiry and no preempt anywhere in `RelayOperationAdmission`. Every later tick fails `tryAcquire` → the loop spins making no progress. Bigger than first reported: `activateEngine` can't preempt either, so a stuck lease **also blocks mosaic switching**.
6. **tesela-d33 (P1) — recurring-task rolls are silently dropped.** `persist_lifecycle_rolls` warns-and-continues on `record_local` failure while the *identical* call 80 lines away is fatal-and-regression-tested. User completes a recurring task, sees success, task never rolls.

## The structural verdict (Sol + Terra converged independently)
Both Lead-tier reviewers, working from different dossiers, named **the relay's snapshot/compaction durability model** as the weakest structural link — which is exactly where Terra's two verified bugs live. Sol's ranked weaknesses:
1. Relay truncation safety is **implicit, not executable** (group-wide note-blind compaction; safety delegated to a caller-computed `covers_seq`; `gqd` open; #195 seq-reset unresolved). → **tesela-d0e**
2. **"Sole writer" is policy, not an enforced boundary** (TUI knowingly reverted; MCP/importer bypass; safety asserted in comments). → myh/ewj, with a stronger acceptance bar.
3. **iOS sync lifecycle ownership is uncontrolled** (abandon-not-cancel; orphan holds handles; cursors cross the FFI boundary; errors swallowed; never built in CI). → tesela-h8m + tesela-6hu
4. **Web has three mutation protocols with no stated total order.** → **tesela-q3p** (and tesela-9ut is a live instance of the gap)
5. **Fork recovery is deterministic *destruction*, not convergence** — max-TreeID stops oscillation but discards the loser's unique edits. `65f` is a **durability** item, not cleanup.

Notably, the audit also *prevented* work: the server's raw `unwrap`/`panic` counts looked alarming but are **test code**; production sync paths are genuinely well-hardened (explicit `Lagged` handling, retry budgets, clobber guards, catch-up queues; no note-content write behind a swallowed Result). **Do not open a broad error-handling epic.**

## RTC: how "full build in scope" survives contact with the audit
Sol's judgment, which I endorse: *Taylor is right about product priority and wrong only if "full build now" means **default-on** regardless of gates.* Build the whole lane now, behind a kill switch; do not point it at the daily-driver corpus until the durability gates pass. Encoded as:
- **tesela-hx8** — RTC production-enable gate (kill switch + 6-rung rollout ladder + 72h physical 3-device soak). Veto conditions: `gqd`, iOS generation fencing, the compaction model, the save-path ordering contract.
- **tesela-d0e / tesela-q3p** — the two XL preconditions, wired as `blocked-by` on `680.5`.
- Safe to start immediately inside 680: **680.1** (DO design), **680.3** (latency harness — measure the *paths* separately, not one end-to-end number), **680.4** (make "cursor UX" into real sync-health diagnostics).
- **680.8** must follow the durable-append contract — optimizing broadcast before defining commit order is backwards.

## Blind spots this audit could not cover (Sol §C — filed as tesela-h9f)
Static review systematically misses failures that live across process death, OS lifecycle, and component boundaries. Four experiments, one of which (**Byzantine relay**) would have independently found tesela-fdd:
randomized kill/restart durability campaign · Byzantine relay (dup/reorder/omit/replay/stale-snapshot/seq-reset/compact-while-offline) · a real DR drill under write load (this is what would settle whether the separate `.backup.lock` is safe) · an end-to-end semantic differential across both parsers × every writer × all three query runtimes.

## Beads filed (19 new) — see roadmap Now
P0 ×1 · P1 ×10 · P2 ×5 · P3 ×3. All carry `tier_floor`/`complexity`/`verify_cmd` metadata. `tesela-gqd` raised P2→P1 (Sol's #1 item; same code as tesela-fdd). `tesela-myh` gained an addendum: `prune_bare_leaf_blocks` is a **second** ungated parse→serialize writer on the ordinary write path.

## Model notes (full entries in ~/.claude/model-scorecard.md)
- **gpt-5.6-terra 5/5** — best sync review on record; 3 of 4 findings verified true, including the AAD gap.
- **gpt-5.6-sol 5/5** — the architecture verdict + the RTC gating synthesis; its §C blind-spot list is the most valuable single artifact of the session.
- **qwen3.7-max 5/5 ×3** — carried three dossiers (core/iOS/web) after GLM and MiniMax both failed on the same recipe.
- **opus-4.8 5/5** — found the P0 nobody else did.
- **glm-5.2 1/5, minimax-m3 2/5** — both produced nothing usable on large (167–181KB) inlined dossiers, *contradicting* the 2026-07-13 "no-tools + pre-digested context works" lesson. Correction: that recipe works for **short** briefs; these lanes have a large-context ceiling. Route big-dossier review to qwen/terra/sol.

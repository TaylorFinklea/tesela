# 2026-06-09 audit → two-stream plan (relay hardening + Graphite cutover)

Source: ultracode bug bash + arch review (169 agents, adversarially verified).
**Full evidence with file:line + verifier notes:** `~/.harness/reports/tesela/20260609-bugbash-arch-review/artifacts/all-findings.json` (91 confirmed findings) + `l0–l6.json`/`lens-fact-checks.json` (42 recs, fact-checked). Read the artifact entry for an item BEFORE implementing — every claim cites exact lines and the verifier notes correct finder inaccuracies.

Decisions (Taylor, product review 2026-06-09 — also in decisions.md):
full hardening batch now · cutover runs PARALLEL · Reminders disable-now/re-route-after ·
relay topology = HA now → CF becomes the one canonical spine (HA → conformance-frozen self-host option; LAN P2P stays step 6) ·
FULL testing program · repo-root cleanup + push-at-session-end (RELEASE.md purge parked) ·
Milestone 3 = finish sync spine (CF deploy + key/pairing min slice + cursor migration + demote Mac-hub WS).

## Stream A — relay hardening + data integrity (Rust/iOS; gates the product test)

Sync-blocking:
- **A1 relay seq fix**: `crates/tesela-relay/src/store.rs` `insert_op` must allocate `MAX(MAX(seq), compaction_seq)+1` (join `relay_group_meta`; mirrors the CF Worker's AUTOINCREMENT). Add a conformance-suite case: deposit-compact-then-put must deliver to a caught-up cursor. Bump HA add-on → 0.2.1 (gate on verified GHCR publish). Verify: `cargo test -p tesela-relay` + conformance suite vs BOTH impls.
- **A2 auth_key leak**: stop serving `auth_key` from open `GET /registration` (check what the client actually consumes from that response first). Conformance case.
- **A3 poison envelope**: `RelayClient::poll` must skip+log a per-row decode/AEAD failure, not abort the batch (one bad envelope currently wedges every consumer forever).
- **A4 cursor-past-failure family** (server `sync_relay.rs` + `tesela-sync-ffi` + engine): per-note apply results surfaced; don't ack/advance past a failed envelope (bounded retry); snapshot bootstrap only advances cursor when ALL imports succeeded (retry failed stream_ids on later ticks); route relay applies through `apply_doc_update_status` so Loro `pending` triggers snapshot catch-up instead of silently freezing the note.
- **A5 scoped cursors**: cursor keys scoped per (relay, group) on BOTH platforms (iOS UserDefaults keys are global today; server state file too) + cleared on re-pair. Prerequisite for the HA→CF migration.
- **A6 iOS .relay writes**: the three write gates (`MockMosaicService.swift` `spliceTodayBlock:351`, `pushPage:987`, `scheduleWriteback:1553`) accept `.relay`; `applyRemoteChange` accepts `.relay` (inbound refresh); don't delete the cached pairing code on transient rebuild errors; clear the Mock seed on `attach(.relay)`.
- **A7 honest sync status**: FFI `tick_outbound` must propagate PUT failures (currently reports success); surface last-error in iOS sync UI. Same for `sendDelta()` success-on-unconnected-socket.

Data corruption (real-mosaic risk today):
- **A8 mojibake**: `crates/tesela-core/src/tag_rewrite.rs:73,198,232` — byte-as-char copy corrupts all non-ASCII on tag rename/delete (twice on rename). Fix with `char_indices()`/str-slice copies + a non-ASCII round-trip test.
- **A9 silent write failures**: `PUT /notes/{id}` returns 200 when `record_local` fails (file saved, sync op dropped, engine reverts the file next materialization) → propagate. `note_tree` parse→serialize drops non-bullet body content and `stamp_existing_notes` runs it at startup → preserve or gate.
- **A10 Reminders containment**: auto-sync default-OFF (stops the 30s self-retrigger loop + EK fail-open clobbers); recur-bump acknowledged same-disease (`store.update` bypass). Full engine re-route = first item of Milestone 3.

Testing program (FULL, per decision — build DURING the stream so fixes land locked):
- **A11 CI green + gates**: one `cargo fmt --all` commit unblocks ci.yml (red since 2026-04-14; fmt step fails first so clippy/tests never ran). Then: workspace tests + svelte-check + web e2e .mjs + relay conformance (BOTH impls) + iOS compile smoke in CI; alert on relay-container failures.
- **A12 convergence harness**: cross-process two-engines + real-relay harness asserting convergence through compaction/restart cycles (would have caught A1/A4/A6). CI-gated.
- **A13 iOS unit target** (RelayTicker tick/bootstrap/cursor, pairing adoption, backend-mode routing) gated in `scripts/ios-testflight.sh`; **FFI regenerate-and-diff** drift check (script currently rebuilds the .a but never regenerates/validates bindings).
- **A14 ship + test**: HA add-on 0.2.1 + new TestFlight build; replace the held harness-deck product test (`20260609-relay-sync-rollout`).

## Stream B — Graphite cutover (web; parallel)

- **B1 the 7 confirmed /g parity bugs**: /v4-chrome eject on wiki-link/date/row drills · dead `tesela:open-leader-at` (g leader) · Cmd+K can't close palette (double-handle) · `:` line styled with out-of-scope `--v4-*` tokens · Peek no-op (PeekPopover only mounted in /v4 layout) · phantom advertised shortcuts (Cmd+W) · orphaned localStorage widgets (rail Pinned/Today, recents boost). Artifact has file:line for each.
- **B2 flip default chrome to /g** (route `/` → /g shell).
- **B3 parity checklist → delete v4/v5** chromes, PRESERVE the reused `lib/v4`+`lib/v5` behavior modules (per the Graphite spec). Rescope the stale "/v4 route removal" backlog item into this.
- **B4 web-editor invariant fixes** (survive cutover, fix in the new chrome's path): inbox triage t/d/x + attach-to-project → engine container ops (today: base-less whole-note PUT + text-line property writes); BlockOpsSaver kind-blind coalescing (move drops pending upsert text); applyRemoteTextEvent multi-run coordinate mapping; NORMAL-mode Enter/Backspace vim guard; JournalView future-dated-dailies hang.

## Deferred to Milestone 3 (finish the sync spine)
CF Worker `wrangler deploy` (+ its 1 MiB body-cap config + registration limits/replay durability BEFORE public) · minimum key/pairing model (wrapped/passphrase-derived key, iOS Keychain for group keys) · cursor migration HA→CF · demote/retire Mac-hub WS as device transport · Reminders + recur-bump engine re-route · NoteDelete tombstone design (wire-format mini-spec; deletes currently never propagate + resurrect).

## Parked / Later
RELEASE.md history purge + auto-release retirement (Taylor deferred) · MockMosaicService decomposition · one-shared-relay-tick-driver in tesela-sync · server decomposition before multi-mosaic · Loro-derived history replacing note_versions · structured block-tree FFI surface · medium/low findings not listed here (all in the artifact, severity-tagged).

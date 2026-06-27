# Spec: multi-device live presence + cursors (the collab north star)

Status: DESIGN (2026-06-27), researched via the `design-multidevice-presence`
workflow (5 agents, adversarial-verified). Implementation NOT started. This is
the north-star arc (`project_emacs2_northstar` RTC; `project_savanne_collaborator`).

## Goal

See your other devices (and eventually collaborators) live: who's online, where
their cursor/selection is, edits streaming in real time. Tesela already has CRDT
co-editing (LoroText interleave) once docs converge; this adds the *presence*
layer on top.

## Primitives — loro 1.13.6 gives us BOTH for free (verified)

- **Presence: `EphemeralStore`** (`loro::EphemeralStore`, re-exported from
  `loro_internal::awareness`; `Awareness` is deprecated → use this). A
  timeout'd, last-write-wins key-value store for transient per-peer state that
  lives OUTSIDE the doc (never persisted to the oplog). API: `new(timeout_ms)`,
  `set/get/delete`, `encode/encode_all` (postcard, skips expired), `apply(bytes)`
  (LWW import), `subscribe_local_updates` (raw bytes to transmit on local
  change), `subscribe` (merged updates: Local|Import|Timeout + changed keys),
  `remove_outdated()` (must be called explicitly in Rust — runs automatically
  only in WASM). → store each peer's cursor/selection/name/color here.
- **Stable position: `Cursor`** (`loro::Cursor`, from `loro_internal::cursor`).
  Anchors a position to op-IDs so it survives concurrent edits (not an index).
  Mint via `LoroText::get_cursor(pos, side)`; resolve via
  `LoroDoc::get_cursor_pos(&cursor) → AbsolutePosition { pos, side }`;
  encode/decode (postcard). Works on Text/List/MovableList. → a cursor on a
  block's `text_seq` LoroText survives the peer's concurrent typing.
- **NOT in `tesela-sync-ffi` yet** — both are Rust-reachable but the FFI only
  exposes delta/snapshot sync. Phase 1 wraps them.

## Cursor representation (portable across devices)

A presence cursor = `{ peer/device id, name, color, block_id (bid),
loro Cursor (on that block's text_seq), [selection: anchor+head Cursors] }`.
Encode the loro `Cursor` (it carries the container + op-id anchor); on the
receiver, `get_cursor_pos` → the utf16 offset within that block's text → render.
- **Capture (local):** on caret move, find the focused block bid + offset →
  `get_cursor` → `EphemeralStore.set("cursor", encoded)`.
- **Render (remote):** on `EphemeralStore` import, for each peer resolve its
  Cursor → block + offset → draw.
  - WEB: a CodeMirror decoration/widget (the `.cm-*` editor; mirror the existing
    decoration machinery in `web/src/lib/cm-decorations.ts`). ⚠ the
    `editor-cursor-model` research agent failed to return — CONFIRM the exact CM
    caret-read + remote-widget API + the iOS caret/overlay before building.
  - iOS: an overlay caret in `BlockRow`/the Graphite editor.

## Transport — the architectural crux

There is NO ephemeral/broadcast path today; everything is persisted Loro updates.
- **Web ↔ server (desktop):** the `/ws` socket already has an in-memory tokio
  `broadcast` fan-out (`ws_delta_tx` for binary deltas, origin-tagged for
  echo-suppression). Add a SEPARATE presence frame (distinct from the `TLR2`
  magic so the dispatcher can tell them apart), broadcast in-memory, NOT applied
  to the engine, NOT persisted. Real-time, easy. ✅
- **Cross-device / iOS (the hard part):** the **CF relay is store-and-poll
  only** — a zero-knowledge mailbox (PUT ops / GET since=N / APNs
  content-available wake). It CANNOT broadcast ephemeral presence, and APNs
  carries no payload. So real-time presence does NOT work over the relay as-is.
  Options (decision needed):
  1. **CF Durable Object WebSocket** (RECOMMENDED). CF DOs support the
     hibernatable WebSocket API — the GroupDO can hold device WS connections and
     broadcast encrypted presence frames in real time, ALONGSIDE the unchanged
     store-poll ops path. This is the durable answer: presence = a new WS channel
     on the relay; ops stay store-poll. Presence frames are AEAD-sealed with the
     group key (zero-knowledge preserved).
  2. **Hub-mode only:** iOS opens a WS to the Mac's `tesela-server` directly
     (works only when the Mac is on + reachable on the tailnet). Cheap first
     step; not "works with the Mac off".
  3. Poll a lightweight presence endpoint — defeats the point (laggy).

## Hard dependency: layer-2 convergence FIRST

A cursor is addressed by block identity (bid → TreeID + text offset). With
DISJOINT TWINS (two TreeIDs for one bid — the June 25/26 residue), a remote
cursor's anchor TreeID gets tombstoned by the dedup → the cursor goes
stale/invalid. So meaningful cross-device cursors REQUIRE stable shared lineage:
- **Land layer-2 (rebase-on-relay-inbound + mergeable containers, see
  `2026-06-26-mergeable-containers-spec.md`) FIRST**, OR
- degrade gracefully: anchor to `(bid, offset)`, resolve to the LIVE survivor
  node each render, and suppress a cursor whose block currently has unresolved
  twins. (Good enough for v1 same-user multi-device; collab wants layer-2.)

## Phasing

- **Phase 0 (prereq): layer-2 convergence** — stable lineage. The foundation;
  also what fixes the June 25/26 stuck notes. Convergence-critical — own spec.
- **Phase 1: FFI** — wrap loro `Cursor` (get_cursor / get_cursor_pos /
  encode-decode) + `EphemeralStore` (set/encode/apply/subscribe + a
  remove_outdated tick) in `tesela-sync-ffi`; Swift + the web/desktop engine
  seam.
- **Phase 2: desktop presence over WS** — presence frame + in-memory broadcast +
  echo-suppression; render remote cursors in CodeMirror. Prove with two desktop
  windows / web tabs on one mosaic (online dot + live caret). No relay needed.
- **Phase 3: iOS presence** — hub-mode WS first; then the CF DO WebSocket
  channel for relay-mode (the real cross-device path).
- **Phase 4: collab polish** — selection ranges, peer names/colors, "follow",
  presence in the sidebar, multi-user-within-mosaic (Savanne).

## Risks / open questions

- The CF DO WebSocket addition (option 1) is the biggest new surface — needs its
  own design (connection lifecycle, hibernation, auth via group key, rate
  limits). Decision: do it, or ship desktop+hub-mode first?
- `EphemeralStore.remove_outdated()` must be driven on a timer in Rust.
- Confirm the editor caret-read / remote-cursor-render APIs (the failed research
  agent) before Phase 2.
- Cursor stability under twin tombstoning until layer-2 lands (degrade path).

## Verify (per phase)

P1: a Rust/FFI round-trip test (mint cursor → encode → apply on a 2nd engine →
resolve to the same offset after a concurrent edit). P2: a Playwright e2e (two
pages, one moves caret → the other shows a remote cursor at the right block) —
reuse the `tests/e2e` harness. P3: sim + desktop live presence.

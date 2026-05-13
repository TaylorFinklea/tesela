# Phase 1.5 sync smoke test

How to verify multi-device sync works end-to-end on a single machine. Two
`tesela-server` instances on different ports against different mosaic
directories, paired manually, and a note created on one should appear on
the other within the sync interval (default 5 seconds).

## Prerequisites

- Rebuild the binary after pulling latest: `cargo install --path crates/tesela-server --bin tesela-server`.

## Steps

1. Create two empty mosaic directories. The presence of `.tesela/` is
   the marker that `find_mosaic` looks for.

   ```sh
   mkdir -p /tmp/sync-smoke-a/.tesela /tmp/sync-smoke-a/notes
   mkdir -p /tmp/sync-smoke-b/.tesela /tmp/sync-smoke-b/notes
   ```

2. Start two servers, each pinned to its own mosaic via `--mosaic`.

   ```sh
   TESELA_SERVER_BIND=127.0.0.1:7474 \
     tesela-server --mosaic /tmp/sync-smoke-a > /tmp/sync-smoke-a.log 2>&1 &

   TESELA_SERVER_BIND=127.0.0.1:7475 \
     tesela-server --mosaic /tmp/sync-smoke-b > /tmp/sync-smoke-b.log 2>&1 &
   ```

3. Confirm each server got its own device id.

   ```sh
   curl -s http://127.0.0.1:7474/sync/peer/device
   curl -s http://127.0.0.1:7475/sync/peer/device
   ```

   Should print two distinct 32-char hex device ids.

4. Pair A with B, and B with A. Substitute the actual device ids
   returned above.

   ```sh
   A_ID=...    # 32 hex chars from /sync/peer/device on 7474
   B_ID=...    # 32 hex chars from /sync/peer/device on 7475

   curl -s -X POST http://127.0.0.1:7474/sync/peer/peers \
     -H "Content-Type: application/json" \
     -d "{\"device_id_hex\":\"$B_ID\",\"url\":\"http://127.0.0.1:7475\",\"display_name\":\"server-B\"}"

   curl -s -X POST http://127.0.0.1:7475/sync/peer/peers \
     -H "Content-Type: application/json" \
     -d "{\"device_id_hex\":\"$A_ID\",\"url\":\"http://127.0.0.1:7474\",\"display_name\":\"server-A\"}"
   ```

5. Create a note on A.

   ```sh
   curl -s -X POST http://127.0.0.1:7474/notes \
     -H "Content-Type: application/json" \
     -d '{"title":"Hello from A","content":"---\ntitle: \"Hello from A\"\ntags: []\n---\n- Sync should propagate this.\n"}'
   ```

6. Wait the sync interval (default 5 seconds) plus a small margin,
   then verify the note appears on B.

   ```sh
   sleep 8
   ls /tmp/sync-smoke-b/notes/hello-from-a.md
   curl -s http://127.0.0.1:7475/notes/hello-from-a
   ```

7. (Optional) Edit on B and verify the change syncs back to A.

   ```sh
   curl -s -X PUT http://127.0.0.1:7475/notes/hello-from-a \
     -H "Content-Type: application/json" \
     -d '{"content":"---\ntitle: \"Hello from A\"\n---\n- Edited on B.\n"}'

   sleep 8
   cat /tmp/sync-smoke-a/notes/hello-from-a.md
   ```

## What's happening under the hood

- Each server runs an `SqliteEngine` (see `crates/tesela-sync`) over the
  mosaic's `tesela.db`. Migration `004_sync_substrate` created
  `oplog`, `peer_cursors`, `parked_ops`, `device_self`, `group_members`,
  `group_keys` in every mosaic database when you started the new binary.
- On every local note write (`POST /notes`, `PUT /notes/{id}`,
  `DELETE /notes/{id}`), `tesela-server` calls `engine.record_local`
  which stamps an HLC, computes a content hash, and appends an op to
  `oplog`.
- Every `TESELA_SYNC_INTERVAL_SECS` seconds (default 5), the sync
  daemon loops over each paired peer and POSTs
  `/sync/peer/produce { peer_device: <my_id>, since_hlc_ntp: <cursor> }`.
  The peer replies with a postcard-encoded `ProduceResponse` containing
  the ops it has that we haven't seen.
- `engine.apply_changes` decodes the ops, dedups against `content_hash`,
  appends them to the local oplog, and (because we passed
  `Some(mosaic_dir)` to `open_with_mosaic`) writes each NoteUpsert's
  content to `{mosaic}/notes/{slug}.md`.
- The existing `Indexer` file-watcher notices the new file, reindexes
  it through `FsNoteStore` and `SqliteIndex`, and broadcasts a
  WebSocket `NoteCreated` / `NoteUpdated` event. The web client picks
  it up live.

## Useful endpoints for inspection

- `GET  /sync/peer/device`   this device's id (hex)
- `GET  /sync/peer/peers`    list paired peers
- `GET  /sync/peer/status`   per-peer cursor and url
- `POST /sync/peer/now`      force one immediate sync attempt with every peer
- `POST /sync/peer/peers`    `{ device_id_hex, url, display_name }` add a peer
- `DELETE /sync/peer/peers/{device_id_hex}` remove a peer

## Known limits in Phase 1.5

- Sync grain is the whole note blob (NoteUpsert carries the full
  markdown). Concurrent edits to the same note in the same window
  resolve by HLC last-writer-wins; the loser's content is still in the
  oplog for inspection but not on disk. Block-level sync is planned in
  `plan/block-level-sync.md`.
- Wire format is cleartext postcard. Crypto (XChaCha20-Poly1305 AEAD)
  arrives in Phase 2 alongside the LAN transport and pairing UI.
- Transport is HTTP-only and requires the peer to be reachable directly
  (no mDNS discovery yet, no relay). LAN mDNS and a thin WebSocket
  relay are Phase 2/3 work.
- Stable note_id is derived deterministically from the slug
  (`blake3(slug)[..16]`) so two devices creating the same slug see it
  as a single note. UUID-v7 note identity arrives with the Mutation
  refactor.

## Fixed since Phase 1.5 initial cut

- `--mosaic <PATH>` is now a real CLI flag (used above). Takes
  precedence over `TESELA_DEFAULT_MOSAIC`.
- NoteDelete now unlinks the file on the receiving peer. The slug is
  carried in the op payload so receivers can locate the target file.

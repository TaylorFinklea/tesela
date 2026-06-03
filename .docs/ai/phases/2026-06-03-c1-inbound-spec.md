# C1-inbound — iOS live-apply of remote splices into the open editor

**Goal:** when a remote character splice lands in the engine while the iOS user is editing the SAME block, live-update the open `UITextView` (minimal diff + caret remap) instead of only refreshing on blur. Closes the iOS half of same-line collab (web↔web already verified, commit `4c92d6a`).

## Root cause (from the understand workflow `wks8c2qzd`)
`CollabTextView.updateUIView` is gated on `!uiView.isFirstResponder`, so the FOCUSED block's text view never updates while editing. Remote splices apply to the engine + materialize, but the open editor only catches up on blur (deferred full refresh via `isEditingBlock`). No per-block live-apply path exists.

## Design (engine is source of truth; editor reconciles to it)
iOS cannot recover a remote splice's offsets from the opaque delta, so after `applyInboundDelta` it READS the merged block text from the engine and reconciles the open editor to it. Own-echo is free: reconcile is a no-op when the view already equals the engine text.

### Rust (new minimal FFI)
- `engine/mod.rs`: add `async fn read_block_text(&self, note_id, block_id) -> Option<String>` to `SyncEngine` (default `None`).
- `engine/loro_engine.rs`: inherent `read_block_text` (read-only `docs.read()`, `find_node_by_block_id`, free `read_block_text(tree,node)`) + trait impl forwarding to it.
- `tesela-sync-ffi/lib.rs`: `pub async fn read_block_text(&self, slug, block_id_hex) -> Result<Option<String>, FfiSyncError>` (mirror `splice_block_text`: `stable_uuid_from_slug` + `parse_block_id_hex`).
- Rebuild `aarch64-apple-ios-sim` release `.a` + regen uniffi bindings into `app/Tesela-iOS/Generated/`.

### Swift
- `CollabTextView.CollabTextInserter.reconcile(toEngineText:)` — common UTF-16 prefix/suffix diff → remap `selectedRange` → set `tv.text` (programmatic, no `shouldChangeTextIn` → no echo) → sync the binding (`coordinator.parent.text`). No-op when equal; skip while `markedTextRange != nil` (IME).
- `BlockRow`: new `onActiveCollabInserter` callback; `collabEditField.onAppear` registers `inserter`.
- `GrDailyView`: set `mosaic.editingBlockId = newValue` in the `editingBlockId` onChange; pass `onActiveCollabInserter: { mosaic.openBlockInserter = $0 }`.
- `MockMosaicService`: `editingBlockId`, weak `openBlockInserter`, `readEngineBlockText` closure; `applyRemoteChange`'s `isEditingBlock` branch calls `reconcileOpenBlockLive()` (reads engine text via the closure, updates the in-memory mirror via `splitTrailingTags`, calls `inserter.reconcile`). Gate on `editingBlockId != nil`, slug = `serverDailyId`.
- `RelayTicker.readBlockText(slug:blockIdHex:)` → `engine.readBlockText`.
- `GrAppShell`: wire `mosaic.readEngineBlockText = { await relayTicker?.readBlockText(...) }`.

## Verify
- `cargo test -p tesela-sync -p tesela-sync-ffi` green.
- xcodebuild sim build green; install on a sim.
- Drive web (browser) + iOS (sim) editing the SAME today block concurrently → both contributions interleave/merge live, no clobber, caret stays sane.

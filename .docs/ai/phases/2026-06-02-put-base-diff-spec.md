# Whole-body PUT base-diff (last clobber vector) — spec (2026-06-02)

> Kills the FINAL concurrent-edit data-loss path. User chose: server base-diff
> AND eliminate the remaining web PUT fallbacks. Build subagent-driven,
> two-stage review, deterministic repro + SIM verification (Claude-driven).

## Confirmed root cause (live repro, ~10 timing variants)
Block-granular writes already closed the in-place/insert/merge/delete clobber (all survived). The LAST vector is the **whole-body `PUT /notes/{id}`**: the server diffs the PUT body against `prev_content` = **the CURRENT server file** (`record_sync_update`, notes.rs:1194-1213, `old_tree = parse_note(prev_content)` where `prev_content = note.content` read fresh at notes.rs:287). When the PUT author is STALE — its body carries the old text of a block a peer concurrently edited — the diff sees that block's text differ from the server's newer text and emits a `BlockUpsert` re-asserting the AUTHOR's stale copy (diff.rs:99-113), **silently overwriting the peer's edit on the authoritative file**. Repro measured `web_edit_on_server == false`. Made worse by silent display masking: the editor's focused-block merge keeps the lost edit on screen until reload, so the user sees no error.

Whole-body PUT is still used by: frontmatter/title edits, note CREATE, and web FALLBACKS (new-block-insert-before-canonical, undo/redo `saveBlocksImmediate`, null-op batch, POST failure). ANY of these, sent stale, clobbers a concurrent peer block edit.

`emit_deletes:false` (notes.rs:1208) already prevents *delete* stomps but NOT *stale-text re-assertion* (an upsert with old text). The base-diff closes that.

## The fix — two parts

### Part 1 (PRIMARY, server): diff the author's BASE → new, not server-file → new
The PUT must apply only the blocks the AUTHOR actually changed, mirroring the block-ops "untouched block ⇒ no op ⇒ peer edit survives" invariant.
- **Contract:** the client sends `base_content` (the full note body it last loaded/last sent — the version it started this edit from) alongside `content` (its new body). `UpdateNoteReq` gains `base_content: Option<String>`.
- **Server:** in `update_note`/`record_sync_update`, when `base_content` is present, diff `base → new` (the author's real edits) instead of `server_file → new`. Emit ops ONLY for blocks the author changed (text differs base→new) or added. Apply those ops to the engine (which already holds the peer's concurrent edit) → Loro merges → both survive. A block identical in base and new = NO op = never re-asserted = peer's edit untouched.
- **emit_deletes with a trustworthy base:** with a real author base, "present in base, absent in new" IS a genuine author delete (not a stale-view artifact) — so deletes COULD be emitted safely on the base-diff path. BUT to stay conservative + consistent with the block-ops delete endpoint, keep `emit_deletes:false` for v1 and let explicit deletes flow through `DELETE /blocks/{bid}` (web already does block-granular deletes). Decide + document; do NOT silently enable deletes without a test.
- **Backward compat:** when `base_content` is absent (older client, or a true whole-note rewrite like create), fall back to TODAY's behavior (diff server-file → new). So the change is additive + safe; clients opt in by sending the base.
- **Frontmatter-only path:** the `ops.is_empty()` → `NoteUpsert` fallback (notes.rs:1215-1237) handles frontmatter/title/non-bullet changes. With base-diff, a frontmatter-only edit still yields empty block ops → NoteUpsert. CONFIRM NoteUpsert doesn't itself re-assert stale blocks: NoteUpsert carries the whole `content` and the engine reconciles the block tree to it (loro_engine `tree_matches_blocks` → reseed). **This is a stale-whole-body re-assert in disguise** — a frontmatter edit sent stale could reseed the block tree over a peer's edit. VERIFY: does the NoteUpsert apply path re-assert block text from `content`, and if so, must the frontmatter-only path ALSO be made base-aware (e.g. apply frontmatter/page-props only, not the body)? This is the subtle one — trace `apply_payload_inner` NoteUpsert (loro_engine.rs ~1652) and decide. If NoteUpsert reseeds the body, the frontmatter-only path needs a body-preserving variant (update root meta + page_props, skip the block-tree reconcile) OR the client must send base so even frontmatter-only edits diff correctly.

### Part 2 (web): eliminate the remaining PUT fallbacks + send base on the PUTs that remain
- Migrate the remaining web whole-body-PUT triggers to block ops where they're genuinely block edits: new-block-insert-before-canonical, undo/redo (`saveBlocksImmediate` ~969), null-op batch fallback (`saveBlocksViaOps` ~874/878), POST-failure fallback (~949). After this, the web PUT is used ONLY for true whole-note writes: frontmatter/title + note-create.
- For the PUTs that REMAIN (frontmatter/title/create), send `base_content` = the body the client last loaded (track it; `lastExternalBody`/`lastSentBody` in BlockOutliner is the candidate base) so even those go through the server base-diff safely.
- Keep the own-echo `recordLocalSave` on every path. Don't regress S1/S2/S3/S4 or the debounce/coalesce.

## Invariants
1. A whole-body PUT (with base) NEVER re-asserts a block the author didn't change → a concurrent peer edit to another block ALWAYS survives on the server.
2. A frontmatter/title edit sent while stale does NOT clobber a peer's concurrent block edit (the frontmatter-only path is body-preserving or base-aware).
3. Backward compat: a PUT WITHOUT base behaves exactly as today (no regression for non-updated clients).
4. The author's own real edits (including deletes via the block endpoint) still land.
5. No silent display masking regression: after the fix, the editor's on-screen state matches the server (don't rely on the focused-block merge to hide a loss — there's no loss to hide).

## Deterministic repro (the spine — write FIRST, red→green)
Rust test (tesela-server or tesela-sync) mirroring the live repro:
- Seed a note with blocks alpha+beta. Capture `base` = that body.
- Peer edits beta → "beta PEER" applied to the server engine (server file now has beta PEER).
- A STALE author PUTs: base=alpha+beta(old), new=alpha CHANGED + beta(old). 
- OLD behavior (diff server-file→new): emits BlockUpsert beta="beta"(stale) → clobbers. (Document as the bug, like concurrent_whole_body_clobber.rs.)
- NEW behavior (diff base→new): author only changed alpha → emits ONLY BlockUpsert alpha → apply → server render has "alpha CHANGED" AND "beta PEER". GREEN.
Add a frontmatter-only variant: stale author changes only the title while peer edits beta → assert beta PEER survives (invariant 2).

## Tasks (subagent-driven, two-stage review)
1. **Repro test** (red→green spine): the base-diff scenario above + frontmatter variant. Rust.
2. **Server base-diff** (Part 1): `UpdateNoteReq.base_content: Option<String>`; `update_note`/`record_sync_update` diff base→new when present, fallback to server-file→new when absent. Resolve the frontmatter-only NoteUpsert stale-body question (Part 1) with a test. Rust.
3. **Web: send base + eliminate fallbacks** (Part 2): send `base_content` on the remaining PUTs; migrate the block-edit fallbacks to ops. Web.
4. **SIM verification (Claude-driven)**: iPad sim + web, reproduce the exact user scenario (web edit committed, then a stale whole-body write from the peer) → assert web's edit survives on server + display; frontmatter edit during a concurrent block edit → both survive. Use /tmp/tesela-srv.log. Per [[feedback-drive-sims-not-devices]] — drive the sim myself.

## Risks
- **The frontmatter-only NoteUpsert path is the sneaky one** — if NoteUpsert reseeds the block tree from `content`, a stale frontmatter edit still clobbers. Must be verified + handled (invariant 2). Do NOT declare done until a frontmatter-while-peer-edits test is green.
- **What base does the client actually have?** `lastSentBody`/`lastExternalBody` may not equal the true server base if the client is mid-divergence. The base only needs to be "what the author started THIS edit from" — good enough for the author-change diff. Document the chosen base source.
- **Create path**: a brand-new note has no base — that's a true whole-note write, fine (no peer to clobber yet).
- Don't regress: block-granular S0-S4, the debounce/coalesce (a7c3924), S2 re-settle, T7.
- Atomicity, dual-path one-save invariant: preserved (PUT and POST still mutually exclusive per save).

## Acceptance
1. Repro test green (base-diff preserves both; frontmatter variant preserves peer edit).
2. `cargo test -p tesela-server -p tesela-sync` green; web `npm run test:unit` green; svelte-check clean; xcodebuild (if iOS touched — likely not) SUCCEEDED.
3. SIM-verified: the user's exact scenario (concurrent edit, web victim) → web edit survives on server AND display, both directions; frontmatter edit concurrent with a block edit → both survive.
4. No regression in the prior block-granular live test (different-block + same-block concurrent edits still converge).

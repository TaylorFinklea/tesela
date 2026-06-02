# iOS engine-rendered display (drop mtime pick) — spec (2026-06-02)

> Fix the dominant multi-device "new block reverts / iOS never updates" bug.
> User chose: **render from the engine, drop the mtime pick entirely.**
> Build subagent-driven, two-stage review. Verify on a STABLE connected device.

## Root cause (confirmed live + in code)
`MockMosaicService.preferLocalIfNewer` (app/Tesela-iOS/Sources/Data/MockMosaicService.swift:1555, called :801 + :938) compares the iOS sandbox `<id>.md` file mtime vs the server's `modified_at`; if local is strictly newer it **returns the whole local body instead of the server body**. That is last-writer-wins between WHOLE documents, which defeats the block-level Loro merge:
- Web adds a new block → server has it. iOS's local daily is mtime-newer (materialized on bootstrap/edit) → iOS keeps its local body lacking the web block → "iOS never updates".
- Existing-block TEXT edits arrive as block-level relay/WS ops that merge into the same bid in the engine → survive ("show in 1s"). That asymmetry is the tell.
Server never deletes the new block (`emit_deletes:false`, notes.rs); the loss is purely client-side display reconciliation.

## Key architecture facts (verified)
- **There is NO `render` FFI.** The iOS engine (LoroEngine, `materialize_dir = <sandbox>/notes`) **materializes each note to `<sandbox>/notes/<id>.md` on every apply** (`import_doc_update` loro_engine.rs:418; `apply_relay_updates` :481). So **the local `.md` file IS the engine's merged output** — rendering "from the engine" = reading that file.
- Inbound WS delta → `applyInboundDelta` (RelayTicker) → engine apply (rewrites file) → `onAppliedChanges` → `applyRemoteChange()` → re-read. Path is sound.
- `bootstrapNoteIfNeeded(slug:)` currently **skips when the doc is resident** (`noteVersion != nil`). So a resident-but-divergent daily NEVER pulls the server's newer ops — it just stays locally stale. This is why "render local" alone would show stale data; catch-up must be real.
- FFI available: `noteVersion(slug) -> Data?` (the local VV), `produceNoteDelta(slug, sinceVv) -> Data?`, `applyDeltaFrame(frame)`, `importNoteSnapshot(slug, bytes)`. Server: `GET /loro/notes/{id}/snapshot` (full) — and `export_doc_update(note, Some(vv))` exists server-side but is NOT yet exposed as an HTTP "give me what I'm missing past THIS vv" endpoint.

## The fix (two coupled changes)

### 1. Display from the engine-materialized file; never mtime-pick the HTTP body
- **Delete `preferLocalIfNewer` usage** at MockMosaicService.swift:938 (today's daily) and :801 (page load). Render the note body from the **engine-materialized local file** (`readLocalNote`/`applyLocalRefreshFallback` already read `<sandbox>/notes/<id>.md`).
- HTTP refresh keeps its OTHER jobs: discover the list of notes, tags, metadata, past-dailies, `serverDailyId`, and `snapshotNotesToSandbox` for notes the engine hasn't materialized. But the **body of a note the engine has resident** comes from the engine file, not the HTTP body.
- For a note the engine has NOT yet materialized (never opened/bootstrapped), the HTTP body is the only source — use it (and trigger catch-up so the engine takes over). No regression for first-view notes.
- Net: the displayed note body is always the engine's merged state. An offline iOS edit (in the engine/file, not yet shipped) is preserved BECAUSE it's in the engine — not because of an mtime override. A web edit shows once the engine applies it (change 2).

### 2. Real catch-up on open (replace skip-if-resident)
`bootstrapNoteIfNeeded` must, for the visible note, ensure the engine holds the server's latest even when resident:
- Resident → send our `noteVersion(slug)` to the server and import the ops we lack. MINIMAL path without a new endpoint: fetch `GET /loro/notes/{id}/snapshot` and `importNoteSnapshot` (idempotent merge — importing a full snapshot into a resident doc merges, doesn't clobber; the engine already de-dups/merges). Heavier on bytes but correct and simple. PREFERRED if cheap enough for the daily.
- If snapshot-merge proves too heavy per open, add a server `POST /loro/notes/{id}/catchup` taking the client VV → `export_doc_update(note, Some(vv))` (the precise delta). Implement only if the snapshot path is too costly — measure first.
- Keep it best-effort + debounced (don't catch-up on every keystroke; on note-open + on reconnect + on an inbound-delta-for-visible-note miss).
- Preserve the suppression guards (`isEditingBlock`/`suppressRemoteUntil`) so catch-up doesn't reseed mid-typing — but catch-up MERGES into the engine, so even if it lands mid-edit the user's in-engine edit isn't lost (verify).

## Invariants
1. Displayed note body == engine-materialized file content for any resident note (no HTTP-body override, no mtime pick).
2. A web-authored new block on the visible note appears on iOS within one catch-up cycle (open/reconnect/inbound), WITHOUT the user editing on iOS.
3. An iOS offline edit not yet shipped is preserved across an HTTP refresh (it's in the engine/file; refresh no longer overrides it).
4. Concurrent edits to DIFFERENT blocks of the same note (web + iOS) both survive (block merge), no whole-body LWW.
5. `preferLocalIfNewer` is removed (or reduced to a no-op that returns serverNote) — no remaining mtime-based whole-body pick on the display path.

## Tasks (subagent-driven, two-stage review)
1. **Catch-up-on-open** — make `bootstrapNoteIfNeeded` (RelayTicker) ALSO catch up a resident note: fetch snapshot + `importNoteSnapshot` (or VV path), debounced, best-effort. Verify importing into a resident doc merges (doesn't lose local ops) with a Rust test if not already covered. Swift (+ maybe a server endpoint if VV path chosen).
2. **Engine-render display** — in `MockMosaicService.refresh` (and the page path ~801), stop calling `preferLocalIfNewer`; source resident-note bodies from the engine file; keep HTTP for list/meta/discovery + first-view bodies. Remove `preferLocalIfNewer` (invariant 5). Swift.
3. **Tests** — Rust: importing a server snapshot into a resident doc with a local-only edit keeps BOTH (merge, no clobber). Confirm `cargo test -p tesela-sync`. iOS: `xcodebuild` build succeeds.
4. **SIM/device QA** — reproduce the revert (stable connected device or sim with a second writer), confirm: web new block → appears on iOS without iOS editing; iOS new block → survives (no revert); concurrent different-block edits both survive. Use the LIVE log `/tmp/tesela-server-sync-live-debug.log` (NOT the stale ones). Keep the device FOREGROUNDED (Roshar idles when backgrounded → no polling → false "pass").

## Risks / notes
- **Don't reintroduce the storm**: catch-up-on-open must be debounced (the T4 coalescer + T6 web coalescer stay). Snapshot-fetch-per-open on a big note (ai-business) could be heavy — scope catch-up to the VISIBLE note, not all loaded pages; measure on the daily first.
- **The offline-edit case `preferLocalIfNewer` protected is real** — preserve it via the engine holding the edit, and verify: make an offline iOS edit (engine/file updated, relay down), then HTTP refresh with a stale server body → the edit must remain (now because we render the engine file, not because of mtime). Add this to QA.
- Engine must be the materializer for the displayed note — if a note is shown that the engine hasn't opened, it won't have a file; handle that (HTTP body + trigger open) so non-bootstrapped notes still render.
- Live server runs from worktree `.worktrees/sync-live-debug` (log `/tmp/tesela-server-sync-live-debug.log`). Roshar build has the Mac Tailscale IP (100.112.34.59) + http-mode baked in (Graphite has no Settings UI — task #156). Date is 2026-06-02.
- Engine-render is the architecturally correct end of the "HTTP-vs-engine split" flagged in the 2026-05-31 delivery-redesign spec.

# Tesela Architectural Review

**Model:** MiniMax M3 (`minimax-m3`)
**Date:** 2026-06-17T10:50:40Z
**Scope:** Whole workspace — Rust crates, SvelteKit web, iOS app, sync stack

---

## What this actually is

The architecture doc at `architecture.md` is significantly stale. Reality is much bigger:

- **13 Rust crates** (not 5): `tesela-core`, `tesela-cli`, `tesela-tui`, `tesela-mcp`, `tesela-plugins`, `tesela-server`, `tesela-backup`, `tesela-fixtures`, `tesela-fixtures-cli`, `tesela-sync`, `tesela-sync-ffi`, `tesela-relay`, `tesela-loro-spike` — plus a Tauri embed shell and an iOS app
- **Web is Svelte 5** (SvelteKit, runes, CodeMirror 6, TanStack Query, bits-ui), not Next.js as `AGENTS.md` claims
- ~9.4k lines in `tesela-core`, ~7k in `tesela-server` routes, ~3k in sync
- A Loro CRDT sync engine (`tesela-sync`) with separate `tesela-relay` server, `tesela-sync-ffi` for iOS UniFFI, encrypted group identity, mDNS LAN discovery, age-encrypted backups, Apple Reminders sync, scheduled/recurring block firing
- iOS app is a separate Swift codebase in `app/Tesela-iOS/`

The core principle ("files are truth, DB is cache") is intact. The shape of the system is correct. But the codebase has grown past the point where one person can hold it all in their head, and several structural patterns are now actively working against you.

---

## The biggest problems

### 1. The server's `lib.rs` is a 1,813-line god module

`crates/tesela-server/src/lib.rs` holds one function, `serve()`, that does everything: mosaic lock, config migration, system-widget seeding, block-id stamping, tag-page auto-creation, ~20 hardcoded built-in pages, indexer bring-up, sync-engine init, mDNS, relay bring-up, backup scheduler spawn, view seeding, parent-death watchdog, env-var resolution for ~11 different env vars, and shutdown. The `AppState` struct in the same module has **19 fields**, several of them `Option<...>`, with no encapsulation.

This isn't an exaggeration — `serve()` is a 600-line linear sequence of side effects. It's also where 8 of the 11 `TESELA_*` env vars get resolved, each with their own precedence rule (env > mosaic config > global config > default), scattered through different functions. You have a `ServeConfig` struct for *one* thing (the mosaic path); the rest is procedural.

The `_ = index.reindex(...)` swallow patterns in the boot sequence are also a smell — a tag-page create that fails gets a `tracing::info!` and a continue, which is a "best-effort" pattern that's hard to reason about.

**Fix:** break `serve()` into named subsystems with explicit start/stop methods. A `MosaicLock::acquire()`, a `BootContext` that holds paths/config handles, then per-subsystem structs: `IndexerSubsystem`, `SyncSubsystem`, `RelaySubsystem`, `BackupSubsystem`, `RemindersSubsystem`, each implementing a `start/stop` trait. The big linear sequence becomes a builder. The `AppState` shrinks to "things routes actually need at request time" vs. "things needed only at boot."

### 2. "Files are truth" and "block-level CRDT" are in an uneasy truce

This is your single biggest architectural risk. The file format and the Loro engine don't agree on what a note is:

- `note_tree.rs` (1,237 lines) does the parse/serialize round-trip and openly admits in its module docs: *"Non-bullet body content does NOT survive the round trip."* This is acknowledged as a known landmine (audit A9b). The `stamped_any` flag tells producers they may need to write back a stamped version.
- There's a *whole-body* write path (`PUT /notes/{id}`) and a *block-granular* write path (`POST /notes/{id}/blocks`). The whole-body path was retrofitted with `base_content` so the server can diff against what the client started from instead of against disk — and there's a test file `concurrent_whole_body_clobber.rs` that exists solely to pin down the race that this fix prevents. That race is still possible any time a client omits `base_content` (backward compat path).
- Loro is the sole sync engine, but Loro's "doc" model is being materialized *back to disk* on every change so the file watcher picks it up. That's Loro serving as cache + sync, with disk as the materialized view — which is fine until it isn't. You have no model for what happens if the Loro doc and the on-disk file disagree (you do, but it lives in `diff.rs` in `tesela-sync` rather than in any one place I can read to understand it).
- `web/src/lib/loro/` (a whole sibling tree: `note-doc.ts` 447 lines, `active-note-doc.svelte.ts` 185, `loro-client.ts` 103, `tlr2.ts` 199) is a *parallel* Loro implementation in TypeScript. The bidirectional WS hub forwards the exact applied bytes; it does not re-produce. That means the iOS app, web, and Rust server all carry their own Loro logic, and "the same wire bytes" is the only thing keeping them aligned.

**Fix:** the core invariant needs to be stated in one place: "disk is the source of truth for human-readable content; Loro is the source of truth for the *edit log* between any two disk snapshots; the round-trip is lossless only for the supported subset." Then audit which paths violate it. The two write paths need a clearer rule — pick one (block-granular) and migrate the whole-body path to it. The non-bullet round-trip gap needs a "what's the data model for prose?" answer, even if the answer is "we don't support prose yet" — right now it's implicit.

### 3. Cross-implementation duplication with no shared spec

You have three implementations of the query language:
- `crates/tesela-core/src/query.rs` (2,890 lines, the canonical)
- `web/src/lib/query-language.ts` (1,016 lines, the web client)
- (presumably) a Swift implementation in `app/Tesela-iOS/`

These are pinned together by `tests/fixtures/query-conformance.json`. That's a fixture, not a spec — you can find out they're *not* aligned, but the conformance suite only catches it at test time. Same story for the block parser (`block-parser.ts` 326 lines vs `note_tree.rs` 1237 lines, the parser portion), the `BlockOp` enum (server side vs TS client), and the Loro/TLR2 framing.

**Fix:** for the query DSL, generate the parser. You already have `ts-rs` generating types from Rust — extend the same machinery so the Rust parser produces an AST that's serialized, and the TS/Swift implementations become pure AST executors. That gives you one source of truth and removes the conformance-fixture-as-spec pattern. Same for the block parser. The Loro layer can't be auto-generated (Loro is Loro), but at minimum the `TLR2` framing should have a one-page spec document, not three implementations that agree on the wire bytes.

### 4. The trait layer in `tesela-core` is over-general for what's actually used

`NoteStore`, `SearchIndex`, `LinkGraph`, `Plugin` are all `Arc<dyn ...>` in the boot sequence, but in practice every production path uses `FsNoteStore` + `SqliteIndex` (which implements both `SearchIndex` and `LinkGraph`). The trait objects add a layer of indirection with no current second implementation. Worse, the `SyncEngine` trait is a *separate* abstraction that overlaps — a `LoroEngine` is also a `NoteStore` of sorts (it materializes notes to disk), but that relationship isn't expressed in the type system.

**Fix:** keep the traits (they're cheap and they document intent), but stop the code from pretending there's a second implementation in production. Drop the `Arc<dyn>` for `NoteStore` and `SearchIndex` in `AppState` and use the concrete types; leave the trait for `SyncEngine` where you actually have a real choice (Loro vs. legacy). Also: a `LoroEngine` should explicitly delegate to a `NoteStore` for materialization, not also implement it. Make the layering legible.

### 5. Configuration is scattered across env, mosaic config, global config, and code

- `tesela-core/src/config.rs` is 612 lines
- The `find_mosaic()` function in `lib.rs` has a 4-step precedence chain documented in 4 separate doc comments
- 11+ `TESELA_*` env vars, each resolved in a different function with different precedence
- Built-in tag/property pages (Task, Project, Person, Status, Priority, etc.) are hardcoded in `lib.rs::serve()` as a `Vec<(&str, &str)>` literal — adding a new built-in requires editing the server
- The `types.toml` system lives in `tesela-core/src/types.rs` (285 lines) with its own `default_types()` function returning 3 hardcoded types — which then conflict with the 18 hardcoded built-in pages in `lib.rs`

So we have *three* sources of truth for "what types exist": `types.toml` (loaded at boot), `default_types()` in core (used as fallback), and the hardcoded `Vec` in `lib.rs`. The first two were apparently once one system; the third was added to add more types. This is debt.

**Fix:** one source of truth. Either the on-disk `types.toml` (and delete the hardcoded `Vec`) or a Rust-registered type set (and delete `types.toml`). The env-var precedence chain should be a single `ResolutionContext` struct that takes a list of "where to look" and an explicit "what wins" matrix.

### 6. The route layer in `tesela-server` is just as monolithic

`routes/notes.rs` is 3,714 lines. `data_ops.rs` is 1,337 lines. `peer_sync.rs` is 590. These are the actual HTTP surfaces and they're not organized in any visible way — `notes.rs` contains list, create, get, update, delete, block-granular updates, tag operations, daily notes, versions, history, and probably more. The `BlockOp` enum, which is the API surface for block-granular writes, is *defined* in this file.

**Fix:** split by resource, not by HTTP verb. `routes/notes/`, `routes/blocks/`, `routes/tags/`, `routes/views/`, `routes/sync/`, `routes/backup/`, `routes/system/` (config, status, health). The `BlockOp` enum and the `NoteUpdateReq` belong in their own request-shape modules, not at the top of an 1,800-line file.

### 7. Observability is haphazard

- `tracing` is used everywhere but there's no visible subscriber config or sampling policy
- Background tasks log sync errors at `debug` level (`tracing::debug!("sync to {}: {}", peer.url, e);` in `sync_daemon_loop`, `tracing::debug!("relay tick: {e}")` in the relay tick). When sync is broken, it will be silent in production logs
- No metrics — no request count, no per-route latency, no Loro doc size, no broadcast channel lag
- The `WsEvent` over the broadcast channel can `Lagged` and silently drop a client; the WS handler `continue`s on lag. For a 64-capacity broadcast channel, under burst load this is going to drop events.

**Fix:** a single `tracing-subscriber` config in `tesela-server/src/main.rs` with sensible defaults (warn in production, info in dev, structured JSON for prod). Promote background-task errors to at least `warn`. Add `metrics` or `tracing` spans around the hot paths. The `Lagged` recovery in the WS hub should at minimum re-send a `WsEvent::Resync` so the client knows to refetch — the web client probably handles this by refetching on every event anyway, but the iOS app might not.

---

## Smaller things worth fixing

- **HLC writes in `note_tree.rs`** are being maintained in two places: the on-disk `<!-- bid:UUID -->` comment *and* the Loro doc. If those ever diverge (crash mid-write, partial disk write), there's no recovery path mentioned in the doc comments.
- **`auto_create_tag_pages`** in `lib.rs::serve()` is a `for note in &all_notes { for tag in &note.metadata.tags { ... } }` O(N×T) loop with no short-circuit. On a large corpus this is a slow boot.
- **`let _ = std::fs::write(...)`** for built-in pages in `lib.rs::serve()` swallows write errors. The user gets no signal that the page failed to create.
- **The `app-query-client.svelte.ts`** (TanStack Query client) is a single 23-line file with no global config — every query in the web app sets its own staleTime/cacheTime. There's no reason not to centralize the defaults.
- **The `cm-decorations.ts` file at 1,483 lines** is the single biggest web file I saw. That's the entire CodeMirror decoration system in one file — surely it has internal sub-domains.
- **`tesela-plugins` is "Lua working, WASM stub."** Per the architecture doc. If plugin support is real, ship WASM. If not, drop the stub and the dead code.
- **`tesela-loro-spike` is a workspace member.** Spike crates should not be workspace members in shipping code; they should live in a side branch. It's currently at parity with the rest of the workspace, which means it's not really a spike.

---

## What is actually working well — don't change

- **`ts-rs` for type sharing** is the right move. Keep extending it to cover the AST-shaped types (the query language AST, the block op shapes) rather than just primitives.
- **The trait surface in `tesela-core`** is well-named. The problem is overuse, not design.
- **The Elm-style TUI** (`handler.rs` 699 lines, but the *purity* matters more than the size) is the right shape. Keep it.
- **The "server-as-library" pattern** (`pub async fn serve(...)` callable from `main.rs` *or* in-process from the Tauri embed) is excellent. The parent-death watchdog for the embed is a real piece of work. Mirror this pattern in the other side: the iOS app should also be able to call into `tesela-sync` via `tesela-sync-ffi` without owning the full process.
- **The mosaic lock** (`flock` on `<mosaic>/.tesela/server.lock`) is a textbook-correct single-writer pattern with OS-level release.
- **The "comment-anchored decisions" culture** is great. The code is full of `// CAVEAT (resolved in L4 Phase B):` and `// Phase 12.3 — ...` breadcrumbs. Don't lose that. The cost is that decisions live in code rather than in `decisions.md` — but that file exists and is 94KB, so they're getting recorded.
- **The Loro migration decision** (Loro as the sole sync engine, delete the dual-write path, flag-day cutover) was the right call. The complexity that's left is the irreducible cost of a CRDT-based sync.

---

## Priority ordering

If I had to pick three to do first:

1. **Decompose `serve()` and shrink `AppState`.** A 600-line linear function with 19-field state is a maintenance cliff, and it makes every other improvement harder.
2. **Audit the file/CRDT boundary.** State the invariant in writing. Pick one write path (block-granular) and migrate the other. Decide what to do about non-bullet content.
3. **Unify the built-in pages/types system.** Pick one source of truth, delete the other two.

After those, the route split, the query-DSL generation, and the env-var resolution are all the same shape of work — turn implicit procedures into explicit named things.

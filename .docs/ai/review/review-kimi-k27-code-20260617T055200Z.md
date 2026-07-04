# Architectural Review — Tesela

**Date:** 2026-06-16  
**Author:** Claude Opus (orchestrator)  
**Scope:** Full repo — Rust workspace, web frontend, iOS/FFI, sync, CI, packaging  
**Context read:** `architecture.md`, `decisions.md`, `roadmap.md`, `current-state.md`, key trait files, `tesela-core`, `tesela-sync`, `tesela-server`, `web/src`, and iOS source tree.

---

## Executive summary

Tesela has a **strong spine**: Loro as the single source of truth, a unified command registry, a conformance-tested query language across Rust/web/iOS, and clear separation between headless core and thin frontends. The project’s biggest architectural debt is not design intent — it’s **accumulated triplication and god-modules** that grew while racing to daily-driver parity. The risk is that the next phase (collaboration / properties / polished mobile) will grind against 2,000–3,000-line files and three overlapping UI generations rather than against the data model.

My top-level recommendation: **finish the cutovers before adding major new surfaces**, and use that cutover momentum to break up the largest modules into services/traits.

---

## What’s strong

| Area | Why it’s solid |
|---|---|
| **Sync authority model** | `LoroEngine` is the sole writer; files are materialized exports; backups capture `.tesela/loro/` + identity. This is the right post-cutover shape (`decisions.md` 2026-06-10, 2026-05-29). |
| **Command registry** | `web/src/lib/command-registry.svelte.ts` is a real spine: palette, leader, slash, and colon all resolve into one registry with rebindable overrides. That’s the emacs-ness the north star demands. |
| **Query-language parity** | `tesela-core/src/query.rs` + `web/src/lib/query-language.ts` + iOS `LocalQueryEngine.swift` share a 182-case conformance fixture. Treating Rust as the source of truth and forcing web/iOS to match is the right contract. |
| **Single-writer safety** | `tesela-server/src/lib.rs` holds an `flock` for the mosaic lifetime; in-process Tauri embed shares the same guard. |
| **Test harness** | Relay conformance runs against both the Rust relay and the Cloudflare Worker in CI; restore drill is an actual `rm -rf → restore → reopen` test. |
| **Decision log** | `decisions.md` is excellent. Most projects lose this within months. |

---

## What I’d improve

### 1. Rust core: `query.rs` is doing too much

- **File:** `crates/tesela-core/src/query.rs` (~2,890 lines)
- **Issue:** It mixes grammar/AST, DSL parser, JQL parser, SQL prefilter, in-memory matcher, calendar/agenda expansion, recurrence, and sorting. That makes the query language hard to test in isolation and hard to port.
- **Fix:** Split into:
  - `query/ast.rs` — `BoolExpr`, `Predicate`, `QueryOp`, `Kind`
  - `query/parser.rs` — DSL + JQL recursive descent
  - `query/matcher.rs` — in-memory evaluation over `ParsedBlock`
  - `query/sql.rs` — `execute_block_query` / `apply_sort` prefilter
  - `query/agenda.rs` / `query/calendar.rs` — date/recurrence views
- **Acceptance:** `cargo test -p tesela-core --lib query` still passes; each submodule is <800 lines.

### 2. Sync engine: `LoroEngine` is a god object

- **File:** `crates/tesela-sync/src/engine/loro_engine.rs` (~9,400 lines)
- **Issue:** It handles doc lifecycle, block ops, property containers, index doc, views registry, materialization, snapshots, relay cursors, twin healing, and broadcast cursors. That’s a lot of critical code in one file.
- **Fix:** Decompose into private modules inside `tesela-sync/src/engine/loro/`:
  - `docs.rs` — per-note doc load/save/export
  - `apply.rs` — `OpPayload` → doc application
  - `materialize.rs` — render to `.md`
  - `index.rs` — index doc + `index_entries`
  - `views.rs` — saved-views registry
  - `relay.rs` — broadcast cursor + `produce/apply_relay_updates`
  - `heal.rs` — twin/dedup/disjoint-lineage logic
- **Acceptance:** `cargo test --workspace` passes; no public API change.

### 3. Server routes have become the business layer

- **Files:** `crates/tesela-server/src/lib.rs` (~1,800 lines), `crates/tesela-server/src/routes/notes.rs` (~3,700 lines)
- **Issue:** The server boot sequence does auto-tag-page creation, built-in page seeding, indexer wiring, reminders, notifications, backup scheduling, and config migration. Route handlers call `store.create`, `index.reindex`, `record_sync_create`, `ensure_tag_pages`, and WebSocket broadcast inline. This violates the project’s own convention (“No business logic in CLI/TUI”) and makes the server hard to unit-test.
- **Fix:** Introduce `tesela-server/src/services/` (or push down to `tesela-core/src/services/`):
  - `NoteLifecycleService` — create/update/delete with tag-page backfill + sync op + WS event
  - `DailyNoteService` — lazy create + index + sync
  - `BootstrapService` — system widgets + built-in pages + block-id stamping
  - `TagService` — rename/cleanup/resolve
- **Acceptance:** HTTP tests still pass; handlers are <100 lines and delegate.

### 4. `SyncEngine` trait is becoming a grab bag

- **File:** `crates/tesela-sync/src/engine/mod.rs`
- **Issue:** The trait has ~30 methods, many with default no-ops, covering docs, views, relay, block text, snapshots, and authoritative rebase. A trait that broad makes it hard to see what an implementation must actually do.
- **Fix:** Split into focused traits that `LoroEngine` implements:
  - `DocEngine` — doc load/export/import/splice
  - `ViewRegistry` — CRUD on saved views
  - `RelayEngine` — produce/apply relay updates
  - `BlockTextEngine` — `splice_block_text`, `read_block_text`
  - `SyncEngine` becomes a marker aggregate or just the parts the server/FFI actually need.
- **Acceptance:** `cargo check --workspace` passes; `AppState` holds `Arc<dyn DocEngine + RelayEngine + …>`.

### 5. Web frontend: three generations are still alive

- **Files:** `web/src/lib/v4/`, `web/src/lib/v5/`, `web/src/lib/graphite/` (~5,240 lines combined)
- **Issue:** Graphite is the current design, but v4/v5 shells and behavior modules are still imported. This is acknowledged in the roadmap as Stream B, but it’s the biggest drag on frontend velocity.
- **Fix:** Finish the Graphite cutover (the roadmap’s Stream B), then delete `routes/v4/`, `lib/v4/`, and `lib/v5/` components that are no longer routed. Keep only genuinely shared behavior modules (commands, query, api-client) under `lib/`.
- **Acceptance:** `pnpm --dir web check` and `pnpm --dir web test:unit` pass; `/g` is the only shell.

### 6. Web components are too large

- **Files:** `web/src/lib/components/BlockOutliner.svelte` (2,392 lines), `BlockEditor.svelte` (2,352 lines)
- **Issue:** These two files contain navigation, editing, vim key handling, save orchestration, property editing, slash/colon/leader dispatch, and CM6 decoration logic. They’re hard to test and easy to regress.
- **Fix:** Extract:
  - `lib/editor/BlockNavigator.ts` — j/k/Enter/Backspace/page-jump logic
  - `lib/editor/SaveOrchestrator.ts` — debounce/flush/own-echo handling
  - `lib/editor/VimController.ts` — vim-mode key routing
  - `lib/editor/SlashController.ts`, `ColonController.ts`
  - Keep `BlockOutliner`/`BlockEditor` as thin wiring shells.
- **Acceptance:** Unit tests cover the extracted modules; no change in keyboard behavior.

### 7. State/store proliferation

- **Files:** `web/src/lib/stores/*` (~20+ stores)
- **Issue:** pane-state, active-pane-nav, fullscreen-overlay, station, peek, journey, recents, etc. are each small but the relationships between them are implicit. That makes the URL ↔ pane ↔ editor state machine hard to reason about.
- **Fix:** Document the state graph. Consider a single `WorkspaceState` rune that owns pane tree, active pane, back-context, and drawer state, with derived stores for consumers.
- **Acceptance:** A markdown doc in `web/docs/state.md` plus a refactor that reduces the number of stores that mutate `gotoNote`/`goBack`.

### 8. iOS / FFI: large surface files

- **Files:** `crates/tesela-sync-ffi/src/lib.rs` (~2,897 lines), `app/Tesela-iOS/Sources/Data/MockMosaicService.swift` (~3,818 lines), `RelayTicker.swift` (~1,330 lines), `LocalQueryEngine.swift` (~1,326 lines)
- **Issue:** The FFI is one file; the mock service is bigger than many real services; `RelayTicker` mixes networking, state machine, and UI-facing status.
- **Fix:**
  - Split the FFI into `ffi/note.rs`, `ffi/sync.rs`, `ffi/views.rs`, `ffi/crypto.rs`.
  - Split `MockMosaicService` into smaller protocol extensions or separate mock engines.
  - Make `RelayTicker` a pure state machine with a separate `RelayTransport` layer.
- **Acceptance:** `check-ffi-drift.sh` still passes; iOS unit tests pass.

### 9. Query parity cost is high

- **Files:** `tesela-core/src/query.rs`, `web/src/lib/query-language.ts`, `app/Tesela-iOS/Sources/Data/LocalQueryEngine.swift`
- **Issue:** Three hand-ported parsers is a maintenance tax. Every new operator (e.g., `BETWEEN`, `IS NULL`) requires three implementations.
- **Fix:** Long term, generate the parser from the Rust grammar (e.g., via a small grammar file + code generator) or compile the Rust parser to WASM for web. Short term, keep the conformance fixture as the contract and add property-based/fuzz cases.
- **Acceptance:** A fuzz target generates 1,000 random queries and asserts Rust/web/iOS agree.

### 10. `architecture.md` is stale

- **File:** `architecture.md`
- **Issue:** It still says “Files are truth, database is cache” and lists a `SqliteEngine`/dual-write model. Loro is now the authority.
- **Fix:** Rewrite the authority/data-flow section to match `decisions.md` 2026-05-29 and 2026-06-10. Delete Slint GUI mention; add Tauri desktop.
- **Acceptance:** A new reader can understand the Loro-first model from `architecture.md` alone.

### 11. CI still has advisory gates

- **File:** `.github/workflows/ci.yml`
- **Issue:** `cargo clippy --workspace` and `pnpm --dir web check` are `continue-on-error`. That means new warnings accumulate.
- **Fix:** Burn down the existing ~26 clippy warnings and the one v4 `VoiceCaptureButton` svelte-check error, then make both gates blocking.
- **Acceptance:** Remove `continue-on-error: true` from CI; green build.

### 12. Sync transport matrix is complex

- **Files:** `crates/tesela-server/src/sync_relay.rs`, `routes/ws.rs`, `routes/peer_sync.rs`, `crates/tesela-sync/src/transport/relay.rs`
- **Issue:** You have WebSocket binary deltas, HTTP relay polls, LAN mDNS peers, and iOS FFI sync. Each has slightly different cursor/ack/echo models. This is necessary for the product, but the abstraction boundaries could be tighter.
- **Fix:** Define a `SyncTransport` trait with `send_delta`, `recv_delta`, and `peer_liveness`. Implement it for `WsTransport`, `RelayTransport`, `LanTransport`, and `FfiTransport`. Keep the cursor logic in one place.
- **Acceptance:** New transport can be added without touching `LoroEngine`.

### 13. Property/type system is mid-flight

- **Files:** `crates/tesela-core/src/property.rs`, `crates/tesela-sync/src/engine/loro_engine.rs` property sections, `web/src/lib/components/PropertyTypeConfig.svelte`
- **Issue:** The foundation is good (`PropScalar`, `PropOp`, typed containers), but it’s still flag-gated and not all surfaces render it consistently.
- **Fix:** Complete the convergence-design pass for nested-container rival hazards (`decisions.md` 2026-06-05b) before flipping migrate-on-write default ON. Don’t ship the type system publicly until old iOS builds can read it safely.
- **Acceptance:** A property created on device A concurrently with device B survives merge without container overwrite.

### 14. Security / identity next steps are well-identified but not implemented

- **Issue:** Pairing uses a shared group key in files; Ed25519 device identity and passphrase-derived keys are in the roadmap but not landed.
- **Fix:** Treat this as the next architectural milestone after the CF relay deploy. Do not open public signup/sharing before device identity exists.
- **Acceptance:** Each device has a stable Ed25519 key; group key can be rotated.

---

## Suggested sequence

1. **Cutover cleanup** (Stream B): delete v4/v5 web shells, finish Graphite as the only shell. *This is the highest-leverage cleanup.*
2. **Break up god modules**: `query.rs`, `loro_engine.rs`, `notes.rs` route handlers. Do this while the code is fresh.
3. **Server service layer**: extract `NoteLifecycleService`, `BootstrapService`, etc.
4. **Make CI gates blocking**: clippy + svelte-check.
5. **Property system convergence**: resolve rival-container hazard, flip migration default.
6. **Sync identity + CF relay deploy**: the next real architecture milestone.

---

## One thing I would not change

The decision to **own the command system in Svelte instead of forking Zed** (`decisions.md` 2026-06-12) is correct. Zed would not solve the block-outliner data model or iOS unification, and the unified command registry is already paying off. Keep investing there.

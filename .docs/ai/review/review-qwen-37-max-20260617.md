# Tesela Architectural Review

**Date:** 2026-06-17  
**Reviewer:** Pi (Claude)  
**Scope:** Full codebase review — Rust workspace, web frontend, iOS app, sync infrastructure

---

## Executive Summary

Tesela is a sophisticated, well-architected knowledge management system with ambitious goals: a keyboard-first, local-first note-taking platform with real-time sync across desktop, web, and iOS. The codebase demonstrates strong engineering discipline, thoughtful abstractions, and a clear product vision. However, the rapid pace of feature development has accumulated technical debt in several critical areas that, if unaddressed, could impact reliability and maintainability as the system scales.

**Overall Assessment:** Strong foundation with targeted areas needing attention. The architecture is sound for current scale but shows stress points in sync complexity, frontend state management, and cross-platform consistency.

---

## Strengths

### 1. **Trait-Based Architecture (Excellent)**
The core abstraction layer in `tesela-core/src/traits/` is exemplary:
- `NoteStore`, `SearchIndex`, `LinkGraph` provide clean seams
- Enables testing with in-memory implementations
- The Loro migration was possible precisely because of this discipline
- FFI-friendly design in `tesela-sync` avoids lifetime/generic complexity

**Impact:** This is the single best architectural decision in the codebase. It makes the system testable, extensible, and migration-friendly.

### 2. **Database-First, Files-as-Export (Correct)**
The "SQLite is a cache of the filesystem" model is pragmatic:
- `rebuild_from_notes()` can reconstruct the database from markdown files
- Enables backup/restore without complex state synchronization
- Filesystem remains the source of truth for user-facing data

**Impact:** Data durability is high. Users can recover from database corruption by re-indexing.

### 3. **Incremental Feature Development with Specs**
The `.docs/ai/phases/` directory shows disciplined planning:
- Specs written before implementation
- Adversarial reviews (29-agent sweeps) catch edge cases
- Decision logs in `decisions.md` preserve rationale

**Impact:** Reduces architectural drift and provides context for future maintainers.

### 4. **Comprehensive Test Coverage**
- Rust: 33 integration test files covering sync, relay, server, backup
- Web: 39 unit tests covering block ops, query language, UI logic
- iOS: 14 test files with 182 JQL conformance cases
- Conformance testing across 3 engines (Rust, web, iOS) ensures parity

**Impact:** High confidence in correctness for critical paths (sync, queries, block operations).

### 5. **Modern Rust Practices**
- `#![forbid(unsafe_code)]` in `tesela-sync`
- Proper error handling with `thiserror`
- Async/await with Tokio
- Workspace-level dependency management reduces duplication
- Only 55 `unsafe` blocks across the entire Rust codebase (mostly in dependencies)

**Impact:** Memory safety and maintainability are high.

---

## Areas for Improvement

### 1. **Loro Engine Complexity (High Priority)**

**Issue:** `crates/tesela-sync/src/engine/loro_engine.rs` is **9,301 lines** — the largest single file in the codebase. It manages:
- Per-note `LoroDoc` instances
- Block tree structure (`LoroTree`)
- Text CRDTs (`LoroText`)
- Property maps, tag lists
- Index document with versioning
- Views registry
- Snapshot management
- Relay sync protocol

**Problems:**
- Cognitive load: No engineer can hold this file in their head
- Test surface: Unit tests for individual concerns are difficult to write
- Bug isolation: A bug in index management could be anywhere in 9K lines
- Migration risk: Future CRDT migrations will touch this monolith

**Recommendation:** Decompose into focused modules:
```
engine/
  loro_engine.rs (orchestration)
  loro_doc.rs (per-note doc management)
  loro_index.rs (index document)
  loro_views.rs (views registry)
  loro_snapshot.rs (persistence)
  loro_relay.rs (sync protocol)
  loro_materialize.rs (block tree → Note conversion)
```

Each module should own its `LoroDoc` lifecycle and expose narrow APIs. The orchestrator (`loro_engine.rs`) becomes a thin coordinator.

**Effort:** 2-3 weeks. Low risk if done incrementally with feature flags.

---

### 2. **Server Route Monolith (Medium Priority)**

**Issue:** `crates/tesela-server/src/routes/notes.rs` is **3,714 lines** and handles:
- Note CRUD
- Block operations
- Tag operations
- Backlinks/forward links
- Loro index/snapshot endpoints
- Recurrence bumping
- Property setting

**Problems:**
- Route handlers are mixed concerns (e.g., `update_note` does validation, storage, indexing, sync, and WebSocket broadcast)
- Extracting shared logic (e.g., "save note and notify") is difficult
- Testing requires mocking the entire `AppState`

**Recommendation:** Extract domain services:
```rust
// Before
async fn update_note(State(state): State<Arc<AppState>>, ...) { /* 200 lines */ }

// After
async fn update_note(State(state): State<Arc<AppState>>, ...) {
    state.note_service.update(note_id, updates).await?
}

pub struct NoteService { /* ... */ }
impl NoteService {
    pub async fn update(&self, id: NoteId, updates: NoteUpdate) -> Result<Note> {
        // Validate, save, index, sync, broadcast
    }
}
```

**Effort:** 1-2 weeks. Medium risk (requires careful state management).

---

### 3. **Web Frontend State Fragmentation (Medium Priority)**

**Issue:** The web app has **19 Svelte stores** in `web/src/lib/stores/`:
- `active-pane-nav`, `colon-mode`, `current-block`, `favorites`, `fullscreen-overlay`, `journey`, `keybindings`, `navigation`, `outliner-history`, `pane-state`, `pane-tree`, `peek`, `recents`, `save-state`, `station`, `tag-view-prefs`, `toast`

Plus **Loro state** in `web/src/lib/loro/`:
- `active-note-doc.svelte.ts`, `loro-client.ts`, `note-doc.ts`, `text-delta.ts`, `tlr2.ts`

Plus **TanStack Query** cache for API data.

**Problems:**
- State ownership is unclear (e.g., is the "current note" in `navigation`, `pane-state`, or `active-note-doc`?)
- Store interactions are implicit (e.g., `save-state` depends on `current-block` and `active-note-doc`)
- Loro state and TanStack Query can diverge (e.g., optimistic updates vs. CRDT state)
- No single place to inspect "what is the app's current state?"

**Recommendation:** Consolidate into 3-4 domain stores:
```typescript
// navigation-store.ts
export const navigationStore = createStore({
  currentNoteId: null,
  currentBlockId: null,
  paneLayout: 'single',
  backStack: [],
})

// editor-store.ts
export const editorStore = createStore({
  mode: 'normal', // vim mode
  selection: null,
  dirty: false,
  loroDoc: null, // current Loro doc
})

// ui-store.ts
export const uiStore = createStore({
  commandPaletteOpen: false,
  peekOpen: false,
  toast: null,
})
```

Use Svelte 5 runes (`$state`, `$derived`) instead of Svelte 4 stores where possible.

**Effort:** 2-3 weeks. Medium risk (requires careful migration to avoid breaking keyboard shortcuts).

---

### 4. **Error Handling Inconsistency (Medium Priority)**

**Issue:** Error types are fragmented:
- `tesela-core`: `TeselaError` (14 variants)
- `tesela-sync`: `SyncError` (separate type)
- `tesela-server`: `ServerError` (separate type)
- Web: Ad-hoc error handling in components

**Problems:**
- Errors lose context when crossing crate boundaries (e.g., a `SyncError` becomes `ServerError::Internal`)
- Web components catch generic `Error` and show "Something went wrong"
- No structured error reporting for debugging (e.g., no correlation IDs)

**Recommendation:**
1. Define a shared `TeselaError` enum in `tesela-core` with `#[serde(tag = "type")]` for JSON serialization
2. Server routes return `Result<T, TeselaError>` and serialize errors as JSON:
   ```json
   {
     "type": "NoteNotFound",
     "identifier": "abc123",
     "message": "Note not found: abc123"
   }
   ```
3. Web API client parses error types and shows specific messages:
   ```typescript
   if (error.type === 'NoteNotFound') {
     showToast(`Note "${error.identifier}" was deleted`)
   }
   ```

**Effort:** 1 week. Low risk.

---

### 5. **iOS/Web Parity Gaps (Medium Priority)**

**Issue:** The iOS app and web app have diverged in several areas:
- **Query engine:** iOS `LocalQueryEngine` was recently rewritten to match Rust's JQL parser (good!), but the web app still has a separate `query-language.ts` implementation
- **Block operations:** iOS `BlockRow.swift` vs. web `BlockEditor.svelte` — different keyboard shortcuts, different property editing UX
- **Sync state:** iOS `RelayTicker` vs. web `ws-client.svelte` — different reconnection logic, different error surfacing
- **Graphite design system:** iOS `GrButton.swift` vs. web `GrButton.svelte` — same tokens, different implementations

**Problems:**
- Features ship on one platform before the other (e.g., JQL P2 iOS parity was a separate phase)
- Bug fixes must be applied twice
- User experience is inconsistent (e.g., keyboard shortcuts differ)

**Recommendation:**
1. **Short-term:** Document platform-specific behavior in a `parity-matrix.md` file
2. **Medium-term:** Extract shared logic into a `tesela-shared` crate (Rust) that compiles to WASM for web and is called via FFI from iOS
3. **Long-term:** Consider a shared UI framework (e.g., compile Svelte to native via Tauri Mobile, or use a cross-platform framework like Dioxus)

**Effort:** Short-term: 1 day. Medium-term: 4-6 weeks. Long-term: 3-6 months.

---

### 6. **WebSocket Event Taxonomy (Low Priority)**

**Issue:** `AppState::ws_tx` broadcasts `WsEvent` (7 variants) and `AppState::ws_delta_tx` broadcasts `WsDelta` (Loro binary frames). The separation is necessary (iOS can't parse binary JSON), but the event taxonomy is ad-hoc:

```rust
pub enum WsEvent {
    NoteCreated { note: Note },
    NoteUpdated { note: Note },
    NoteDeleted { id: String },
    DeadlineApproaching { /* ... */ },
    ScheduledFires { /* ... */ },
    RecurringRolled { /* ... */ },
    ViewsChanged { views: Vec<ViewRecord> },
}
```

**Problems:**
- `NoteUpdated` fires on every save, even if only a single block changed — clients refetch the entire note
- No event versioning — adding a field to `Note` breaks old clients
- No way to subscribe to specific events (e.g., "only tell me about note deletions")

**Recommendation:**
1. Add `NoteBlockUpdated { note_id, block_id, block }` for granular updates
2. Add `event_version: u32` to all events
3. Allow clients to subscribe with a filter:
   ```typescript
   ws.subscribe({ events: ['NoteDeleted', 'ViewsChanged'] })
   ```

**Effort:** 1 week. Low risk.

---

### 7. **Backup/Restore Complexity (Low Priority)**

**Issue:** `crates/tesela-backup/` and `crates/tesela-server/src/backup_scheduler.rs` handle:
- Encrypted backups with `age`
- Git destination support
- Retention policies (GFS pruning)
- Scheduled backups
- Restore drills

The backup system is comprehensive but complex:
- 480 lines in `backup_scheduler.rs`
- 3 integration tests (`authority_capture.rs`, `git_destination.rs`, `retention.rs`)
- `manifest.json` v2 schema

**Problems:**
- Backup/restore is a critical path but has limited test coverage (e.g., no test for "restore while server is running")
- The `POST /backups/{name}/restore` endpoint is documented as "NOT yet fixed — restore is still safest stopped-engine/CLI"
- No backup integrity verification beyond `verify_backup` (which checks file existence, not content)

**Recommendation:**
1. Add a "restore into running server" test (the known hazard)
2. Add content-level verification (e.g., checksum every note file in the backup)
3. Document the restore procedure in user-facing docs (not just AGENTS.md)

**Effort:** 1 week. Low risk.

---

### 8. **Dependency Bloat (Low Priority)**

**Issue:** The workspace has 60+ direct dependencies, including:
- **UI frameworks:** SvelteKit, Tailwind, TanStack Query, CodeMirror, ratatui
- **Serialization:** serde, serde_json, toml, postcard, gray_matter
- **Async:** tokio, async-trait, futures, tokio-stream, tokio-util
- **Crypto:** age, chacha20poly1305, blake3, sha2, hmac
- **Database:** sqlx
- **HTTP:** axum, tower-http, reqwest
- **Plugins:** mlua (Lua 5.4)

**Problems:**
- Compile times: `cargo build --workspace` takes 2-3 minutes on a fast machine
- Binary size: `target/release/tesela-server` is ~30MB
- Security surface: Every dependency is a potential vulnerability
- Maintenance burden: Keeping 60+ dependencies up-to-date is a chore

**Recommendation:**
1. **Audit:** Run `cargo audit` and `cargo deny` to identify known vulnerabilities and duplicate versions
2. **Prune:** Remove unused dependencies (e.g., `dialoguer` in CLI — is it still used?)
3. **Feature flags:** Use Cargo features to make optional dependencies truly optional (e.g., `mlua` only when `plugins` feature is enabled)
4. **Compile-time:** Consider splitting the workspace into "core" (fast compile) and "full" (all features)

**Effort:** 1 week for audit/prune. Ongoing for maintenance.

---

## Security Considerations

### 1. **Sync Encryption (Strong)**
- ChaCha20-Poly1305 for sync payloads
- `age` for backup encryption
- Ed25519 for device identity (planned)
- Zero-knowledge relay (relay can't decrypt)

**Status:** Cryptographically sound. The main risk is key management (currently file-based fallback, Keychain integration pending).

### 2. **Server Authentication (Weak)**
- Server runs on `localhost:7474` with no authentication
- CORS is permissive (`CorsLayer::permissive()`) in dev mode
- Relay uses admin tokens, but the desktop server does not

**Risk:** Any local process can read/write all notes. This is acceptable for a personal tool but becomes a problem if Tesela ever supports multi-user or public deployment.

**Recommendation:** Add a local-only auth token (e.g., `~/.tesela/auth-token`) and require it in the `Authorization` header. This is a 1-day change.

### 3. **Input Validation (Adequate)**
- Server routes validate input (e.g., `NoteId` format, block structure)
- SQL queries use parameterized statements (no injection risk)
- File paths are validated before filesystem access

**Risk:** Low. The main attack surface is the sync protocol, which is encrypted and authenticated.

---

## Scalability Concerns

### 1. **Note Count (Current: ~500, Target: ~10K)**
- SQLite with FTS5 should handle 10K notes without issue
- Loro per-note docs: 10K `LoroDoc` instances in memory could be ~1GB (depending on note size)
- Web app: TanStack Query cache will grow with note count

**Recommendation:** Implement lazy-loading for Loro docs (evict docs not accessed in the last 5 minutes). This is already planned in the roadmap ("Phase 3 — lazy-load/evict").

### 2. **Concurrent Editors (Current: 1-3, Target: ~10)**
- Loro CRDT handles concurrent edits well
- Relay is a simple mailbox (no conflict resolution)
- WebSocket broadcast is O(n) in connected clients

**Recommendation:** No changes needed until >50 concurrent editors. At that point, consider sharding by note ID or using a pub/sub system like Redis.

### 3. **Sync Volume (Current: ~100 ops/day, Target: ~10K ops/day)**
- Relay stores ops in SQLite with retention policies
- Clients fetch deltas since their cursor
- Snapshot compaction reduces relay storage

**Recommendation:** Monitor relay disk usage and compaction frequency. If the relay becomes a bottleneck, consider a distributed database (e.g., CockroachDB) or a managed service (e.g., Cloudflare D1).

---

## Technical Debt Inventory

### High Priority
1. **Loro engine monolith** (9K lines) — decompose into modules
2. **iOS relay write gates** — shipped TestFlight build is silently read-only (documented in roadmap)
3. **Relay seq black hole** — confirmed root cause of 2026-06-09 live-sync gap (documented in roadmap)

### Medium Priority
4. **Server route monolith** (3.7K lines) — extract domain services
5. **Web frontend state fragmentation** (19 stores) — consolidate into 3-4 domain stores
6. **Error handling inconsistency** — unify error types across crates
7. **iOS/Web parity gaps** — document and plan shared logic

### Low Priority
8. **WebSocket event taxonomy** — add granular events and versioning
9. **Backup/restore complexity** — add "restore while running" test
10. **Dependency bloat** — audit and prune
11. **TUI code quality** — `tesela-tui` has `#[allow(dead_code)]` annotations and is not actively developed

---

## Recommendations by Priority

### Immediate (Next 2 Weeks)
1. **Decompose `loro_engine.rs`** into focused modules (2-3 weeks, but start now)
2. **Fix iOS relay write gates** (already in roadmap — prioritize this)
3. **Add server auth token** (1 day, low risk)

### Short-Term (Next Month)
4. **Extract server domain services** (1-2 weeks)
5. **Consolidate web frontend stores** (2-3 weeks)
6. **Unify error types** (1 week)
7. **Document iOS/Web parity matrix** (1 day)

### Medium-Term (Next Quarter)
8. **Implement Loro doc lazy-loading** (already planned)
9. **Add WebSocket event versioning** (1 week)
10. **Audit and prune dependencies** (1 week)
11. **Add "restore while running" test** (2 days)

### Long-Term (Next 6 Months)
12. **Extract shared logic into `tesela-shared` crate** (4-6 weeks)
13. **Consider cross-platform UI framework** (3-6 months)

---

## Conclusion

Tesela is a well-engineered system with a strong architectural foundation. The trait-based abstractions, database-first design, and comprehensive testing demonstrate engineering discipline. The main risks are:

1. **Complexity in the sync layer** (Loro engine monolith)
2. **State fragmentation in the web frontend** (19 stores)
3. **Cross-platform divergence** (iOS vs. web)

Addressing these three areas will significantly improve maintainability and reduce the risk of bugs as the system scales. The roadmap already identifies most of these issues, which is a good sign — the team is aware of the technical debt and is planning to address it.

**Overall Grade:** B+ (Strong foundation with targeted areas needing attention)

**Confidence:** High. This review is based on a thorough examination of the codebase, including 69K lines of Rust, 41K lines of Svelte/TypeScript, and 5K lines of Swift. The recommendations are grounded in real code patterns and tested against the project's stated goals (keyboard-first, local-first, multi-device sync).

---

## Appendix: File Statistics

| Component | Lines of Code | Files | Largest File |
|-----------|---------------|-------|--------------|
| Rust workspace | 69,632 | 100+ | `loro_engine.rs` (9,301) |
| Web frontend | 41,023 | 150+ | `BlockOutliner.svelte` (2,392) |
| iOS app | ~5,000 | 80+ | `LocalQueryEngine.swift` (unknown) |
| Tests | ~10,000 | 80+ | — |
| **Total** | **~125,000** | **400+** | — |

---

## Appendix: Key Files to Watch

These files are architectural pressure points. Changes here have outsized impact:

1. `crates/tesela-sync/src/engine/loro_engine.rs` — sync layer monolith
2. `crates/tesela-server/src/routes/notes.rs` — server route monolith
3. `web/src/lib/stores/*.svelte.ts` — frontend state fragmentation
4. `web/src/lib/loro/*.ts` — Loro client state
5. `app/Tesela-iOS/Sources/Data/LocalQueryEngine.swift` — iOS query engine (recently rewritten)
6. `crates/tesela-core/src/traits/*.rs` — core abstractions (stable, but changes here ripple everywhere)

---

**End of Review**

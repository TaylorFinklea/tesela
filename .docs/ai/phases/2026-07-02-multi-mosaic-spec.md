# One server hosting N mosaics — design spec

**Bead:** `tesela-mmos.1` (Lead design spec, SPEC-ONLY — no code). **Epic:** `tesela-mmos`.
**Status:** revised after architecture review (rev 2, 2026-07-02). Two reviewers: glm-5.2 approve-with-nits, gpt-5.5 reject ("direction right, not implementation-ready"). Every finding addressed — see the Review responses appendix.
**Locked direction:** decisions.md 2026-07-01b — *"Multi-mosaic end-state: ONE SERVER HOSTING N MOSAICS — committed"* (overrides the review's cheaper process-per-mosaic lean).
**Depends on:** `tesela-qql` (per-note lazy-load — LANDED) + `tesela-b8v` (per-note eviction — PENDING; mmos sequences AFTER it). Prerequisite mechanism (§3) not yet in tree.

> This document is design only; it prescribes no code blocks. Implementers read the named modules and mirror them. Helper signatures below are either **verified against the code** (cited with file) or marked **find-and-mirror**.

---

## 1. Problem + what changes

`tesela-server` today is **one process = one mosaic**. `serve(ServeConfig{mosaic}, …)` (`lib.rs:95`) boots exactly one `AppState` over one mosaic dir, takes one process-lifetime flock, spawns one set of daemons, and `routes::build` bakes that single `AppState` into the router via `.with_state(Arc::new(state))` (`routes/mod.rs:234`). "A second mosaic" today means "a second server on another port" (roadmap:165) — a non-starter for mobile-first daily-driving, and the desktop embed can only ever show its one `resolve_mosaic()` pick.

**End state:** one process holds a **registry** of N mosaics, resolves the target mosaic **per request**, opens/closes/evicts whole mosaics on demand, holds **one flock per open mosaic** (not per process), runs the WS hub + relay tick + backup scheduler **per mosaic**, and exposes the hosted set via `GET /mosaics` + a pair-handoff so a joining device imports them.

**Orthogonality to lazy-load/evict.** qql/b8v evict *per-note docs WITHIN one mosaic's engine* (`Inner.docs: RwLock<HashMap<[u8;16], LoroDoc>>`, `loro_engine.rs:251`). mmos evicts *whole mosaics* (an entire `MosaicState` = engine + store + index + daemons + flock). Two axes, composed: closing a mosaic drops its whole `docs` map (implicitly evicting every resident note); opening one starts with zero resident notes and fills via qql lazy-load. b8v caps per-mosaic RAM **before** mmos multiplies it by N — that is the sequencing reason.

## 2. Grounded current primitives (mirror these — do not reinvent)

- **`AppState`** (`state.rs:14`) is a flat struct: `mosaic_root`, `store: Arc<FsNoteStore>`, `index: Arc<SqliteIndex>`, `ws_tx`, `ws_delta_tx`, `ws_conn_seq: AtomicU64`, `type_registry`, `auto_sync`, `sync_engine: Arc<dyn SyncEngine>`, `lan_discovery`, `group_identity: Arc<RwLock<GroupIdentity>>`, `display_name`, `public_url` (`state.rs:59`), `relay_url`, `relay: Option<RelayHandle>`, `backup_status`. **Field scope is NOT uniform — do not treat "everything but `display_name`" as mosaic-scoped (the prior draft's error):**
  - **Mosaic-scoped** (→ `MosaicState`, §4.1): `mosaic_root`, `store`, `index`, `ws_tx`/`ws_delta_tx`/`ws_conn_seq`, `type_registry`, `sync_engine`, `group_identity`, `relay`, `backup_status`. `relay_url` is mosaic-scoped too — it is read from the mosaic's own `config.toml` `[sync.relay]`, so each mosaic keeps its own relay group (§7).
  - **Process-global** (→ `ServerState`, §4.1): `display_name` (device-global) **and `public_url`** — `public_url = build_public_url(&addr, bound_port)` (`lib.rs:356`, `:537`) is derived from the process bind address + bound port, NOT from any mosaic. It is one value for the whole server; pairing composes it with a *mosaic's* `group_id` (§7). `lan_discovery` is also process-global (§6). This is gpt-5.5's m3 field-split fix.
  - **Per-mosaic identity on disk but NOT the advert identity**: `device_id` lives at `<mosaic>/.tesela/device_id.hex` (one per mosaic), yet the single process-global mDNS advert cannot carry N device ids — see the host-identity split in §6 (gpt-5.5 m2). `auto_sync` is nominally per-mosaic in the struct but wraps a *device-global* OS resource (EventKit / Apple Reminders) — unresolved, tracked as an open question (§14, glm g3).
- **`serve()`** (`lib.rs:95`) is the whole boot: config-migrate → `acquire_mosaic_lock` → seed widgets → stamp bids → build store/index/type_registry → `LoroEngine::with_dirs(device, hlc, <mosaic>/.tesela/loro, Some(<mosaic>/notes))` (`lib.rs:308`) → `load_or_create_group_identity(mosaic)` → mDNS → `bring_up_relay_if_configured` → `backup_scheduler::start` → `notifications::start` → `reminders::auto::start_triggers` → `indexer.start()` → `axum::serve(…).with_graceful_shutdown`. On return: `indexer_handle.stop()`, auto-backup, flock dropped.
- **Flock** (`acquire_mosaic_lock(mosaic: &Path) -> Result<std::fs::File>`, `lib.rs:619`): `flock(LOCK_EX|LOCK_NB)` on `<mosaic>/.tesela/server.lock`, held by the `_mosaic_lock` local for the whole `serve` call; OS-released on exit (even SIGKILL) → no stale-lock hazard.
- **Daemon shutdown is the load-bearing gap.** `serve`'s own doc comment (`lib.rs:89-94`): *"the background daemons (sync, relay tick, reminders, notifications, backup scheduler) are detached `tokio::spawn` tasks with NO shutdown handle — they rely on the PROCESS ending to stop … an in-process embedder that calls `serve` more than once in a long-lived process would leak them. Cleanup would require a `CancellationToken` mechanism (not yet implemented)."* Confirmed: **zero** `tokio_util`/`CancellationToken`/`JoinSet` in the crate. Only the **indexer** has a real stop handle (`indexer_handle.stop()`); everything else is fire-and-forget. **mmos cannot close/evict a mosaic without first building this mechanism.**
- **Per-mosaic identity on disk** (all under `<mosaic>/.tesela/`): `server.lock`, `device_id.hex`, `group_id.hex` + `group_key.bin` (`load_or_create_group_identity`), `loro/` snapshots, `config.toml`. **No format change is needed** — the registry is an in-memory index over these existing dirs (§8).
- **Engine is mosaic-scoped.** `LoroEngine.Inner.snapshot_dir = Some(<mosaic>/.tesela/loro)`; one engine per mosaic. Residency-audit landmine (ADR-6, decisions.md:449): `produce_relay_updates` walks only resident docs (`loro_engine.rs`) — mmos inherits it at a coarser grain (§5).
- **Mosaic routes today** (`routes/mod.rs`): `GET /mosaics/current`, `GET /mosaics/discovered` (`data_ops::list_discovered_mosaics:1110` — scans `Config::mosaic_root_dir()` for `.tesela/` subdirs, always includes current, returns `DiscoveredMosaic{name, path, is_current, note_count, last_modified}` computed off-disk by `summarize_mosaic:1147`), `POST /mosaics` (create), `POST /mosaics/switch` (`switch_mosaic:1193` — only writes `cfg.general.default_mosaic`, restarts nothing), `POST /server/restart` (`restart_server:1229` — re-execs; **refuses under `TESELA_EMBEDDED`**).
- **Pairing handoff** (`peer_sync.rs`): `PairingCodePayload{code, display_name, device_id_hex, url, short_code, …}` (`:189`); `PairingCode::from_local(group_id?, s.public_url, s.relay_url)` (`get_pairing_code:262`). Carries **exactly one** mosaic's `(group_id, url, relay_url)`. `pair_with_code` adopts into `s.group_identity` + `s.mosaic_root` (`:380`).
- **Clients address the server by base + path.** Web: `apiBase()` (`runtime-base.ts:18`) = injected `window.__TESELA_API_BASE__` (`""` same-origin for desktop; `WebviewWindowBuilder … initialization_script("window.__TESELA_API_BASE__ = '';")` in `src-tauri/src/main.rs:475`) else `/api` (vite/proxy strips `/api` → server `/notes`). iOS: `BackendSettings.serverURL` (e.g. `http://mac-a:7474`) + `mode` (relay/http/mock) + a device-local `MosaicRegistry` (`app/…/Data/BackendSettings.swift`, `Components/MosaicChromeButton.swift`). **Both prepend a base to a path** — the seam a path prefix rides in.
- **Desktop embed** (`src-tauri/src/main.rs`): in-process `serve` on a leaked multi-thread runtime, single-instance plugin FIRST (only the primary takes the flock, #202), one window `"main"` → `http://127.0.0.1:<port>/g`, `EmbedHandle{shutdown, join}` drained ≤30s on Exit. `resolve_mosaic()` → `TESELA_MOSAIC` env or `~/Library/Application Support/tesela/logseq`. `desktop.toml` flat keys `remote_url` / `relay_url`. `set_embed_env` sets `TESELA_SERVER_BIND=127.0.0.1:0`, `TESELA_DISABLE_MDNS=1`, `TESELA_DISABLE_PEER_SYNC=1`, `TESELA_EMBEDDED=1`, `TESELA_DISABLE_RELAY` (unless a relay is configured).

## 3. PREREQUISITE (hard gate) — per-mosaic daemon shutdown

**Nothing in mmos's close/evict/switch story works until each mosaic's daemons can be stopped without ending the process.** This is a standalone bead that MUST land first (own Verify).

- Add `tokio_util::sync::CancellationToken` (`tokio-util` is **absent** from `tesela-server/Cargo.toml` today — verified — so this bead adds the dep) + a per-mosaic `JoinSet<()>` (or a `Vec<JoinHandle>`).
- Every daemon spawn in the per-mosaic boot (relay tick `lib.rs:851`, presence bridge `lib.rs:907`, backup scheduler, notifier, reminders auto-sync, the NoteEvent→WsEvent bridge `lib.rs:261`) must `select!` its loop against `token.cancelled()` **and hand its `JoinHandle` back to the caller** so the mosaic's `JoinSet` can drain it. **This is a signature change at each spawner, not a call-site wrap** (LOW-nit fix): e.g. `presence_relay::spawn(relay_base, group_id, device_id, group_key, ws_delta_tx)` (`presence_relay.rs:96`) today takes 5 args, calls `tokio::spawn` *internally*, and returns `()` — you cannot cancel or join it from the outside. It must grow a `CancellationToken` parameter (select the internal loop against it) and either return its `JoinHandle` or take an `&mut JoinSet` to spawn into. The same holds for every other fire-and-forget spawner listed; wrapping the existing call site does nothing. The indexer keeps its existing `stop()`.
- `close_mosaic` = `token.cancel()` → `join_set.join_all().await` (bounded timeout) → `indexer_handle.stop()` → final flush/backup → drop engine/store/index → drop flock `File`.
- Acceptance for THIS gate: open→close→re-open the same mosaic in one process N times with **no leaked tasks and no leaked fds** (flock re-acquires cleanly). This retires the `lib.rs:89-94` caveat.

## 4. AppState → registry; per-request resolution (path prefix, decided)

### 4.1 Split the state

Split `AppState` into two:

- **`ServerState`** (process-global, one instance, the router's `.with_state`): `registry: MosaicRegistry`, `display_name`, `public_url` (§2 — bind-derived, process-global), the process-global mDNS `LanDiscovery` + host identity (§6), and boot config (bind addr, static dir). **Find-and-mirror** the exact field set against `state.rs`.
- **`MosaicState`** = today's `AppState` minus the process-global fields (`display_name`, `public_url`, `lan_discovery`) per §2. One `Arc<MosaicState>` per **open** mosaic, owned by the registry.

`MosaicRegistry` = `RwLock<HashMap<MosaicId, MosaicSlot>>`. The registry keys internally on the canonical path; `MosaicId` is the stable public handle in URLs.

**`MosaicId`** = a URL-safe slug of the mosaic dir basename, de-duplicated by appending a short hash of the canonicalized path on collision. NOT the `group_id` (it rotates — decisions.md 2026-07-01/groupkey-rotation, and mmos never surfaces `group_id` in a URL). *(Open question §14.)*

### 4.1a Slot lifecycle state machine (converged-HIGH fix — both reviewers)

The prior two-variant `{ Resident, Known }` enum is racy: two concurrent first-requests to a *cold* mosaic both try to flock, the loser gets a nonsense **same-process 409**, and close/evict can release the flock while in-flight requests or WS writers still hold the engine. Replace it with an explicit five-state machine plus an admission **lease/refcount**.

```
MosaicSlot =
  | Closed(MosaicMeta)                         // registered, no flock/daemons/engine
  | Opening { meta, barrier: Arc<OpenBarrier> }// exactly one opener working; others await `barrier`
  | Open    { state: Arc<MosaicState>,
              handle: MosaicHandle,
              leases: LeaseCounter }           // serving; leases > 0 pins it
  | Closing { drain: Arc<CloseBarrier> }       // no new leases admitted; draining to 0 then releasing
```

- `MosaicMeta` = `{ id, path, name, is_default }` — pure metadata, no OS resources.
- `MosaicHandle` = `{ cancel: CancellationToken, tasks: JoinSet<()>, _flock: std::fs::File }` (§3).
- `OpenBarrier` / `CloseBarrier` = a shared awaitable the registry publishes so concurrent callers **wait on one transition instead of racing it** — a `tokio::sync::Notify` (or a `Shared<oneshot>` future) carrying the eventual `Result<Arc<MosaicState>>` / `()`. The registry lock is held only to read/swap the slot enum; the actual open/close work runs **outside** the lock (heavy work is `spawn_blocking`, §4.3 / g1).

**Transitions (who does what, what concurrent callers await):**

| From | Event | Action | Concurrent callers |
|---|---|---|---|
| `Closed` | first request / explicit open | CAS slot → `Opening{barrier}` under the registry write-lock (the CAS makes exactly ONE caller the opener), release lock, then do the heavy open off-lock | every other caller that finds `Opening` **awaits `barrier`** — they do NOT attempt their own flock, so there is no same-process 409 |
| `Opening` | open succeeds | swap → `Open{ state, handle, leases: 1-for-the-triggering-request }`, resolve `barrier` with `Ok(state)` | awaiters resume from `barrier`, each **acquires its own lease** on the resolved `state` |
| `Opening` | open fails (real cross-process flock conflict, or IO error) | swap → `Closed(meta)`, resolve `barrier` with `Err` | awaiters resume with the SAME `Err` (one flock attempt, one 409, shared) |
| `Open` | request / WS connect | `leases.acquire()` → `MosaicLease` guard (RAII, `Drop` decrements) | many concurrent leases coexist; a WS holds its lease for the socket's whole lifetime |
| `Open` | evict/close trigger | only if `leases == 0` may it go straight to teardown; if `leases > 0` swap → `Closing{drain}`, stop admitting new leases, and let outstanding guards drain | new requests that arrive during `Closing` **await `drain`, then re-open** (transition falls through to `Closed`→`Opening`), so a close racing a request re-opens transparently rather than 409-ing |
| `Closing` | last lease dropped | run the ordered teardown (§5): final flush → `cancel` → `JoinSet` drain → `indexer.stop()` → drop engine → **release flock LAST** → swap → `Closed(meta)`, resolve `drain` | awaiters on `drain` resume and re-open |

**Lease/refcount = the flock-release safety gate.** The flock `File` is dropped **only** in the `Closing → Closed` teardown, which cannot start until `leases == 0`. So an in-flight handler or a live WS writer always holds the engine + flock for the duration of its work; close/evict can never yank the engine out from under it (the concrete bug both reviewers flagged). `never_evict` (default/foreground) is a policy check *before* even entering `Closing` (§5). A WS reconnect after a `Closing` completed just re-opens.

**Extractor contract (ties into §4.3):** the `Mosaic` extractor resolves a slot to an `Arc<MosaicState>` **plus a `MosaicLease` guard it stashes in request extensions**, so the lease lives exactly as long as the request. For WS, the handler upgrades the lease to live for the socket. This is what makes "drain before flock release" mechanical rather than aspirational.

### 4.2 Selector = PATH PREFIX, not header

**Decision: path prefix `/m/{mosaic_id}/…`.** Rationale (why header loses):

1. **The selector must ride in a URL.** WebSocket (`/m/{id}/ws`), the SPA deep-link (`/g/m/{id}/…`), `<img src>`/attachment URLs, and any shareable link all carry a URL and cannot carry a custom header. A header (`X-Tesela-Mosaic`) is invisible to every browser-native URL context — it would force a parallel selector for WS/static and split the model.
2. **The proxy/base seam already path-rewrites.** Web's `apiBase()` is `/api` behind a rewrite that strips `/api`; extending to `/api/m/{id}/*` is mechanical. A header must be forwarded by every proxy hop + the vite dev rewrite — more surface, easy to drop.
3. **Clients already prepend a base to a path.** Web `apiBase()` and iOS `serverURL` both compose `base + path`; a prefix slots in with no new transport concept. iOS's per-serverURL `MosaicRegistry` collapses to one serverURL hosting many mosaics via prefix.

Rejected but door left ajar: the resolver MAY *additionally* honor an `X-Tesela-Mosaic` header as a secondary selector for a future non-URL caller — but v1 ships path only; do not build the header path.

### 4.3 Resolution mechanism (minimize handler churn)

**Verified counts (prior draft's "56" was inflated — LOW-nit fix):** `85` handler sites take `State(state): State<Arc<AppState>>` (`rg 'State<Arc<AppState>>'`), and `state.mosaic_root` appears on `34` route lines (`37` crate-wide, incl. `lib.rs`/tests) — `rg '\.mosaic_root\b'`. The extractor exists to avoid rewriting all 85 bodies:

- Introduce an axum extractor **`Mosaic(pub Arc<MosaicState>)`** implementing `FromRequestParts`. **There is NO existing custom extractor in this crate to mirror** (verified: zero `FromRequestParts`/`FromRequest` impls; handlers use only the built-in `State`/`Path`/`Query`/`Multipart`) — so implement it fresh per axum's documented `FromRequestParts` pattern, pulling `Arc<ServerState>` out via `FromRef` (the router's single `.with_state` becomes `ServerState`, and `Arc<ServerState>: FromRef<ServerState>` — or a manual `FromRef` — lets the extractor reach the registry).
- The extractor reads the `{mosaic_id}` `Path` param, looks it up in `ServerState.registry`, resolves the slot per the §4.1a state machine (**awaiting the `Opening`/`Closing` barrier rather than racing a flock**), **lazily opens on `Closed`** (§5), **acquires a `MosaicLease` and stashes it in request extensions** (§4.1a), and returns `Arc<MosaicState>` — or `404` (unknown id) / `409` (real cross-process flock conflict, shared via the barrier). Implement `Deref<Target = MosaicState>` so handler bodies that do `state.store` / `state.sync_engine` / `state.mosaic_root` change only their **parameter** (`State(state): State<Arc<AppState>>` → `mosaic: Mosaic`), not their bodies. Handlers needing global bits additionally take `State(server): State<Arc<ServerState>>`.
- **Heavy open work must NOT run inline in the `FromRequestParts` poll (glm g1).** The open path does blocking file IO + Loro snapshot decode + index open + flock acquire — mirror the crate's existing `tokio::task::spawn_blocking` pattern (e.g. `routes/data_ops.rs:44/123/…`, `reminders/darwin.rs:79`). Concretely: the ONE opener (the §4.1a CAS winner) runs the heavy boot inside `spawn_blocking` (or a dedicated open task) and resolves the barrier; every concurrent extractor just `await`s the barrier — no extractor blocks the async runtime, and the heavy work runs exactly once.
- This is a large but **mechanical** edit across the route modules — spec it as its own Senior lane, one module at a time, each compiling green.

### 4.4 Router shape + the default-mosaic alias (migration seam)

`routes::build` changes from `build(state: AppState) -> Router` (`routes/mod.rs:33`, `.with_state(Arc::new(state))` at `:234`) to `build(server: ServerState) -> Router`. Mount the mosaic sub-router **twice**:

- Nested: `.nest("/m/{mosaic_id}", mosaic_router())` — the extractor reads the path param.
- At root: `.merge(mosaic_router())` — the extractor finds **no** path param and resolves the registry's **default** mosaic (`Config::general.default_mosaic`).

**Absent `{mosaic_id}` is a first-class case, not an error (LOW-nit fix).** The root-mounted copy has no path segment for the id, so the extractor must read the param as **`Option<Path<String>>`** (axum yields `None` when the param is absent rather than failing the extraction) — `None` ⇒ resolve the default mosaic; `Some(id)` ⇒ resolve that id. Both paths then run the identical §4.1a resolve+lease logic. So `GET /notes` == `GET /m/{default}/notes` and every pre-mmos client keeps working unchanged (§8).

**The `/api` base seam already path-rewrites (claim grounded).** Web dev proxies `/api/*` to the server with the prefix stripped: `web/vite.config.ts:16` — `rewrite: (p) => p.replace(/^\/api/, '')` under the `server.proxy['/api']` block. Extending the client base to `/api/m/{id}` rides that same rewrite mechanically (§4.2). Process-global routes (`/health`, `/info`, `/mosaics*`, pair-handoff, static `/g` fallback) mount once, outside both. *(Static `/g` fallback and the SPA index stay process-global; the SPA reads `/m/{id}` from its own client route.)*

## 5. Lifecycle: open / close / evict WHOLE mosaics

Refactor `serve`'s per-mosaic body into `open_mosaic(path) -> Result<(Arc<MosaicState>, MosaicHandle)>` (find-and-mirror — it is `serve` minus arg-parsing, the listener, and `axum::serve`). A new `serve_multi(registry_config, shutdown, on_bound)` builds `ServerState`, seeds the registry (§7), builds the router **once**, and drives axum. The standalone `serve()` becomes a thin wrapper: open the one `--mosaic`, register it as default, call `serve_multi`.

The transitions below are the §4.1a state machine's teardown/bringup steps; the table is the *ordering* contract.

| Op | Trigger | Does | Notes |
|---|---|---|---|
| **open** | first request to a `Closed` mosaic (extractor lazy-open), or explicit `POST /mosaics/{id}/open` | CAS `Closed→Opening` → `open_mosaic` (acquire flock, engine w/ zero resident notes, store, index, group identity, WS channels, per-mosaic daemons under a fresh `CancellationToken`) → `Opening→Open{leases:1}` | real cross-process flock conflict → back to `Closed`, `409` shared via barrier (§4.1a/§6). Never opens eagerly at boot beyond the default. |
| **close** | explicit / process shutdown | `Open→Closing`, **drain leases to 0**, then: final flush (see gate) → `token.cancel()` → drain `JoinSet` → `indexer.stop()` → final backup → drop engine → **release flock LAST** → `Closing→Closed` | §3 is the enabling mechanism; §4.1a lease-drain is what makes flock-release safe. |
| **evict** | idle/LRU policy (mosaic-granularity, the b8v analog one level up) | same ordered teardown as close, policy-driven | leaves `Closed(meta)` so the next request re-opens transparently. |

**Interaction with qql/b8v (per-note):** closing/evicting a mosaic drops its entire `Inner.docs` map — a superset of what b8v evicts per note. Opening starts empty; qql fills on demand. The two policies are independent knobs (per-note RAM cap vs. per-mosaic resident count).

**Eviction safety (the produce-only-resident landmine, ADR-6).** `produce_relay_updates` broadcasts only resident docs; a *closed* mosaic runs **no** relay tick, so an evicted mosaic must not strand un-broadcast local ops. Rule: **do not evict a mosaic with a pending un-synced tail — run one final `sync_relay::tick` (flush) as a close step, and only evict when the broadcast tail is caught up** (or the mosaic has no relay). Never evict the default/foreground mosaic (a policy gate checked *before* entering `Closing`, §4.1a).

- **The "tail caught up" query is a NEW engine method — no queryable accessor exists today (glm g2 fix).** The prior draft cited `broadcast_cursor` as if it were readable; verified, `broadcast_cursor` is a **private field** (`RwLock<HashMap<[u8;16], Vec<u8>>>`, `crates/tesela-sync/src/engine/loro_engine.rs:293` — note the real path is `engine/loro_engine.rs`, not top-level), reachable only via the setter `commit_broadcast_cursors` (`:1234`) and the internal `produce_relay_updates` walk. There is **no** "is everything broadcast?" accessor. Add one on `LoroEngine`, sketch:
  ```rust
  /// True if any resident note's current version is ahead of its persisted
  /// broadcast cursor (i.e. local ops not yet handed to the relay producer).
  pub async fn has_unbroadcast_tail(&self) -> bool
  ```
  Implement it by comparing, per resident doc, the doc's current version-vector against the decoded `broadcast_cursor[bid]` (the same comparison `produce_relay_updates` already makes internally). The evict gate = run one final tick, then evict only when `!has_unbroadcast_tail()` (or `relay` is `None`). Mark this as a find-or-add on the engine, owned by the `mmos.lifecycle` lane, not assumed to exist.
- mmos-level `GET /mosaics` counts notes off-disk (`summarize_mosaic` already reads `.md` files) — it must **not** force-open every engine to answer (residency-audit discipline at the mosaic grain).

## 6. WS hub + relay tick + backup scheduler PER MOSAIC

- **WS hub is per-mosaic.** `ws_tx` / `ws_delta_tx` / `ws_conn_seq` live in `MosaicState`; `/m/{id}/ws` subscribes to that mosaic's channels. A client viewing mosaic X holds a WS to X; switching mosaics closes X's socket and opens Y's (or holds one socket per open mosaic in a multi-window desktop). `ws_conn_seq` echo-suppression ids only need uniqueness within a mosaic's fan-out — unchanged semantics.
- **Relay tick + presence bridge are per-mosaic.** Each open mosaic with a configured relay spawns its own `sync_relay::tick` loop bound to that mosaic's `(group_id, group_key, engine, RelayHandle)` under the mosaic's token. N open mosaics = N tick loops, each polling its own group. One relay group per mosaic is **unchanged** — mmos just runs N of them in one process.
- **Backup scheduler + notifier + reminders auto-sync are per-mosaic**, each under the mosaic's token; `/m/{id}/backup/status` reads that mosaic's `backup_status` handle.
- **mDNS/LAN discovery is PROCESS-GLOBAL, not per-mosaic** (deliberate divergence from "everything per mosaic"). One device advertises **one** service; the mosaic list is learned via `GET /mosaics` *after* connecting, not via N mDNS records. `LanDiscovery` moves to `ServerState`. (The embed keeps `TESELA_DISABLE_MDNS=1`.)
- **mDNS advert identity ≠ per-mosaic `DeviceId` (gpt-5.5 m2).** `device_id` is stored **per mosaic** (`<mosaic>/.tesela/device_id.hex`), so a single process-global advert cannot carry N of them — and it must NOT pick the default mosaic's id (that makes the default special and mis-identifies the host once the user is on another mosaic). Resolve by splitting identity into two layers:
  1. **Host identity** — a process-global id that the single mDNS advert carries ("this Tesela host is reachable at `IP:port`"). It is decoupled from any group; it identifies the *machine/server*, not a mosaic. (Persist it process-globally — e.g. at the `mosaic_root_dir()` level — see §14 open question; do not synthesize it from a mosaic's `device_id`.)
  2. **Per-mosaic sync `DeviceId`** — unchanged: stays the relay/pairing identity *within* a mosaic's group.
  **Peer filtering is by GROUP MEMBERSHIP over HTTP, not by mDNS.** The advert is a pure "host exists" beacon; a discovering device fetches `GET /mosaics`, and can only *adopt/sync* a mosaic whose group it already belongs to (holds the ContentKey) or is invited into via the §7 pair-bundle. So "which mosaics can this peer touch" is an authorization question answered by the key model (§7), never by the discovery layer — no mosaic is special at the mDNS tier.
- **Resource fairness / backpressure across N daemon loops (gpt-5.5 m1).** N mosaics × (relay tick loop + presence WS + backup timer + notifier timer + reminders auto-sync + indexer watcher + NoteEvent→Ws bridge) cannot each run unbounded. Scheduling/limits model for v1:
  - **One shared multi-thread tokio runtime** (already the case) cooperatively schedules all loops — no per-mosaic runtime.
  - **Cap resident mosaics** (foreground + a small LRU window); §5 eviction enforces the cap. This bounds the *number* of live loops directly.
  - **A process-global `Semaphore` gates heavy blocking ops** (backup encrypt, full index rebuild, snapshot flush) so N mosaics can't stampede the `spawn_blocking` pool simultaneously — heavy work acquires a permit first.
  - **Jitter/stagger per-mosaic timer starts** (backup/notifier/reminders) to avoid a thundering herd when several mosaics open together.
  - **Reminders `AutoSync` already serializes through a single `Mutex`** (`reminders/auto.rs`) — that becomes a natural process-global chokepoint (and see the device-global EventKit open question, §14/g3).
  - Relay ticks self-throttle via their existing backoff; no change beyond threading them through the cap.

## 7. `GET /mosaics` + pair-handoff discovery

- **`GET /mosaics`** (process-global, `ServerState`) — the registry is now the source of truth. Returns every registered mosaic `{id, name, path, is_default, open: bool (slot is Open per §4.1a, i.e. loaded), note_count, last_modified}`. It **supersedes** the fs-scan role: keep `GET /mosaics/discovered` as "dirs under the root you *could* register/add"; `GET /mosaics` is "mosaics this server *hosts*." CLI `init` **adds** to the registry (roadmap:162) rather than overwriting the single `--mosaic`.
- **Registry seed at boot (§5 `serve_multi`):** register `Config::general.default_mosaic` (open it — it is the foreground), and register every `.tesela/` dir under `Config::mosaic_root_dir()` as **`Closed(meta)`** (§4.1a — lazy, no flock, no daemons, no engine). Do not open them.

### 7.1 Identity-model reconciliation (gpt-5.5 HIGH — mosaic ↔ group ↔ phrase)

gpt-5.5 flagged that the pair-handoff was not coherent with the recovery-phrase identity model: a recovery phrase reads as ONE recovered `group_id`/key, yet N mosaics each need their own identity/roots/cursors — so *what authorizes exporting MULTIPLE group keys in one handoff?* Reconciled against **decisions.md 2026-07-02 (multi-user key hierarchy)** + **2026-06-30 (recovery phrase + QR)**:

**The mapping is 1:1:1 per mosaic — there is NO device-global recovered identity spanning N mosaics.**

| Layer | Cardinality | Source of truth |
|---|---|---|
| **Mosaic** | 1 | `<mosaic>/.tesela/` on disk; `MosaicId` in URLs |
| **Group + ContentKey** (today's `group_id` + `group_key`) | **1 per mosaic** | `<mosaic>/.tesela/group_id.hex` + `group_key.bin` (`load_or_create_group_identity`) |
| **BIP39 recovery phrase** | **1 per mosaic** — it renders *that mosaic's* ContentKey (per the ADR, the ContentKey stays "BIP39-renderable, random group_id") | derived from the mosaic's `group_key` |

So the phrase is **per-mosaic**, not per-device. A device hosting N mosaics holds N groups / N ContentKeys / N phrases. "Recover my identity from a phrase" recovers exactly **one mosaic's** group membership — this is the honest answer to gpt-5.5's "one recovered key vs N mosaics": there was never a single key to reconcile; recovery is per-mosaic-group. This is consistent with the §4.1 rule that `MosaicId ≠ group_id` (the id is stable; the group_id/phrase rotate per the groupkey-rotation model) and with the §13 non-goal "no collapsing mosaics into one relay group."

**What authorizes exporting multiple group keys in one handoff:** the handoff is a device-to-device transfer over a channel the QR/short-code **authenticates between two devices the same user controls**. The single QR proves the joining device is authorized for at least the default group; exporting *additional* mosaics' ContentKeys is gated on (a) the pairing device already holding them (it hosts them) and (b) the user **explicitly selecting** which to share. Authorization is per-mosaic, user-consented, over the authenticated channel — not "one key unlocks all."

**Forward-compat with the key-hierarchy ADR (which is LOCKED but GATED — unimplemented until Savanne).** Today (single-tenant, per the ADR's "content encryption UNCHANGED" layer) the handoff transfers the raw ContentKey per selected mosaic. Post-hierarchy, adopting a device into a mosaic becomes *wrapping that mosaic's ContentKey to the new device's X25519 key + a signed per-device roster addition* — onboarding stops meaning "re-type the phrase." The mmos wire contract for the pair-bundle (below) must therefore be **shaped so raw→wrapped is additive**: carry a per-mosaic material blob whose encoding can swap from raw-key to wrapped-key without changing the envelope (do NOT bake "export raw phrase for N groups" in as the only shape — that is exactly the ADR's "must not assume phrase-retyping is the only onboarding path" constraint). mmos v1 does not implement wrapping; it must not preclude it.

### 7.2 Multi-mosaic pairing UX contract

- **Pair-handoff (the roadmap "Mosaic discovery" item, roadmap:161-163).** Today's pairing code carries one mosaic's `(group_id, url, relay_url)` (`PairingCode::from_local`, `peer_sync.rs:262`; adopts into `s.group_identity` + `s.mosaic_root` at `:380`). Because **each mosaic keeps its own group** (§7.1), "pair to a server" = "adopt each selected mosaic's group." Design:
  1. **Authenticate once.** The existing 6-char/QR flow establishes the transport + the **default** mosaic's group (unchanged — single-code path still works). This is the one authentication step.
  2. **Discover, then adopt per mosaic (explicit, user-selected).** After pairing, the joining device calls `GET /mosaics` to see the hosted set, then requests a **per-mosaic pairing bundle** for the mosaics the user selects — a new endpoint returning, per mosaic, `PairingCode::from_local(group_id_of_that_mosaic, public_url, relay_url_of_that_mosaic)` (find-and-mirror `get_pairing_code`, iterated over registry slots; `public_url` comes from `ServerState` per §2, the group_id/relay_url from each `MosaicState`). Each bundle entry carries the per-mosaic material blob of §7.1 (raw ContentKey v1; wrapped later). iOS imports each into its device-local `MosaicRegistry`, adopting each group identity + its own roots/cursors.
  3. **Consent is per mosaic** (§7.1) — the joining device never silently receives every group key; the user picks which mosaics to bring across. (Open question §14: default the selection to all-hosted vs none.)
  4. Keep it **interim-compatible**: the single-mosaic code path is untouched; mmos ADDS the multi-mosaic bundle. Do not fold all mosaics under one group_id (breaks the per-mosaic group + rotation model, §13).
- LAN Bonjour "find devices on my network" stays a separate parallel path (roadmap:163) — one host advert per device (§6), mosaic list fetched over HTTP after connect, per-mosaic adoption gated on group membership (§6/§7.1).

## 8. Migration from today's single-mosaic processes

- **Data: no on-disk change.** Each mosaic keeps its own `.tesela/` (loro snapshots, group identity, device id, lock, config). Migration is code-only; existing mosaics work the instant they are registered.
- **Config:** `Config::general.default_mosaic` stays the default selector (drives the root-alias, §4.4). The registry derives `Closed` mosaics (§4.1a) from the `mosaic_root_dir()` scan + the default. (Optional later: a persisted explicit registered-list; not required for v1 since the scan + default cover it.)
- **Routes:** the root alias (§4.4) means every hard-coded `/notes`, `/loro/*`, `/sync/*`, `/ws` in web/iOS keeps resolving to the default mosaic with **zero client change**. New/updated clients opt into `/m/{id}/*` when the user switches to a non-default mosaic.
- **Standalone bin:** `tesela-server --mosaic X` still works — X registers as default+open; other root-dir mosaics register `Closed` (§4.1a). `ServeConfig{mosaic}` keeps meaning "the default mosaic."
- **Clients:** web/iOS default to bare routes; the `GET /mosaics` list drives a switcher that flips the client's base-path (through the `apiBase()` seam / iOS serverURL builder) to `/m/{id}`. iOS's device-local `MosaicRegistry` + `serverURL` already models multiple mosaics — mmos lets ONE serverURL host many (prefix per mosaic) instead of one-serverURL-per-mosaic-server.
- **Sequencing gate:** land §3 (daemon cancellation) first, land after b8v (per-note eviction), then the state-split + extractor, then per-mosaic daemons, then `GET /mosaics`/pair-bundle, then desktop switcher.

## 9. Desktop embed implications (windows-per-mosaic?)

- **One embedded server process hosts N mosaics** (holds N flocks — the single-instance plugin already guarantees one process, so no cross-process flock contention among the app's own windows). The mosaic **switch becomes in-app navigation** (`/g` → `/g/m/{id}/…` → server opens that mosaic on demand) — **no `/server/restart`, no re-exec.** This directly retires the `switch_mosaic`+`restart_server` dance the embed already refuses (`TESELA_EMBEDDED`, `data_ops.rs:1234`).
- **Windows-per-mosaic = optional fast-follow, needs no server change.** A "New Window" opens a second `WebviewWindow` pointed at `/g/m/{other_id}`; both windows talk to the same embedded server, distinct flocks per mosaic — safe because one process owns all flocks. **v1 = single window + in-app switcher; multi-window is layered on later** (just another `WebviewWindowBuilder`).
- `resolve_mosaic()` becomes "the default mosaic to show first"; the server registers the rest lazily from `mosaic_root_dir()`. `desktop.toml` could later gain an explicit mosaic list, but the root-dir scan covers v1.

## 10. Cross-cutting invariants (must hold)

- **One writer per mosaic, N flocks per process** (§4/§5). A second server or a second embed opening the *same* mosaic loses only *that* mosaic's flock (`409`), not the process. A same-process concurrent opener **never** 409s — it awaits the §4.1a `Opening` barrier.
- **Flock releases only after leases drain to 0** (§4.1a) — an in-flight request or live WS writer always outlives the engine it holds; close/evict cannot yank it.
- **No leaked daemons across open/close** (§3) — the retired `lib.rs:89-94` caveat is the acceptance bar.
- **Field scope is explicit, not "everything but display_name"** (§2/§4.1) — `public_url` + `lan_discovery` + host-identity are process-global (`ServerState`); `relay_url` and the rest are per-mosaic (`MosaicState`).
- **Root alias == default mosaic forever** (§4.4/§8) — the pre-mmos client contract never breaks; absent `{mosaic_id}` resolves the default.
- **Mosaic ↔ group ↔ phrase is 1:1:1; no device-global recovered identity** (§7.1) — the pair-bundle exports per-mosaic material with per-mosaic user consent, in an envelope that swaps raw→wrapped additively.
- **Per-mosaic relay group preserved** (§7) — mmos never collapses mosaics into one group_id; rotation (groupkey-rotation spec) stays per-mosaic.
- **`GET /mosaics` answers off-disk** — never force-opens engines (§5, ADR-6 discipline).
- **mDNS advert is one-per-device on a HOST identity** (§6) — decoupled from any per-mosaic `DeviceId`; peer authz is by group membership, not discovery.

## 11. What ejn.2's interim mosaic-switch UX MUST NOT paint us out of (explicit)

`ejn.2` = the desktop mosaic-switch fix, kept **interim** (decisions.md 2026-07-01b: *"hide/disable, don't build a relaunch flow mmos would replace"*). Constraints the interim UX must honor:

1. **MUST NOT build a restart/re-exec-based switch** (the `switch_mosaic` + `server/restart` re-exec path). mmos makes switching in-process; a relaunch flow is thrown-away work and fights the single-instance flock.
2. **MUST NOT teach a "switch = app restart" mental model** in copy or flow. Prefer hide/disable the switcher over shipping a restart-based one that sets the wrong user expectation.
3. **MUST NOT hard-code bare `/notes`-style paths at call sites.** Keep switching routed through the base/prefix indirection (`apiBase()` on web, the serverURL builder on iOS) so a `/m/{id}` prefix drops in later without touching every fetch.
4. **MUST NOT treat the global `config.toml` `default_mosaic` as *the* switch mechanism** in a way a second window/client observes as a global flip. Per-window / per-client mosaic selection must stay possible (mmos makes selection a client-side URL concern, not a server-global mutation).
5. **MUST NOT spawn a second embedded server / second port per mosaic** as the desktop's answer — that is exactly the workaround mmos removes (one process, N flocks).
6. **MUST NOT couple pairing to "the single current mosaic" irreversibly.** Keep the door open for per-mosaic group identities in the handoff (§7) — don't bake a one-group-per-server assumption into the pairing UI.
7. **MUST NOT assume one global WS/relay for "the mosaic."** Keep the client WS-URL derivation able to carry a mosaic prefix later (`/m/{id}/ws`), even if interim uses the bare `/ws` default-alias.
8. **MUST NOT persist UI state that hard-wires `default_mosaic` as the ONLY reachable mosaic** (e.g. caches, deep links) — leave room for N reachable mosaics addressed by id.
9. **(x) MUST NOT persist mosaics client-side as filesystem-only paths.** A mosaic must be addressable by its stable `MosaicId` (the URL handle, §4.1), not a raw `path` string. iOS's `MosaicRegistry` / web's switcher state must key on the id so a `/m/{id}` route (and a mosaic reached over relay with no local fs path at all) resolves — a path-only handle can't ride the prefix seam or survive being hosted on another device.
10. **(y) MUST NOT keep relay identity a single global app value.** Relay group identity is **per mosaic** (§7/§7.1) — the interim UX must not hardcode one relay group/`group_id` for "the app," or bake a one-relay-per-client assumption into settings, cursor scoping, or APNs registration. Client relay wiring must already be able to carry a per-mosaic group so the pair-bundle's N groups (§7.2) drop in without a rewrite.

## 12. Suggested lane decomposition (for Lead triage)

Each becomes its own bead with a `Verify:`; ordered by dependency.

1. **`mmos.cancel`** — per-mosaic daemon shutdown via `CancellationToken` + `JoinSet`; change each spawner's SIGNATURE (§3, not a call-site wrap) to accept a token + return/join its handle; add `tokio-util`. *Verify:* `cargo test -p tesela-server` incl. a new open→close→reopen-N-times no-leak test.
2. **`mmos.split`** — split `AppState` into `ServerState` (incl. `public_url`, `lan_discovery`, host id) + `MosaicState` (§2/§4.1); introduce `MosaicRegistry` + the five-state `MosaicSlot` machine + `MosaicHandle` + lease/refcount + `Opening`/`Closing` barriers (§4.1a). *Verify:* `cargo build -p tesela-server` + existing tests green (single-mosaic behavior unchanged) + a unit test that two concurrent opens of one cold mosaic both resolve via the barrier (no same-process 409).
3. **`mmos.extractor`** — `Mosaic` extractor (fresh `FromRequestParts` + `FromRef`; `Option<Path<String>>` for absent id; `spawn_blocking` for heavy open; lease into request extensions) + `.nest("/m/{id}")` + root default-alias; mechanically migrate the `85` handler params (§4.3/§4.4). Audit `keymap`/`transcription` scope here (§14). *Verify:* clippy clean + a two-mosaic integration test hitting `/m/{a}/notes` and `/m/{b}/notes` + bare `/notes`==default.
4. **`mmos.lifecycle`** — `open_mosaic`/`close`/`evict` (ordered teardown, lease-drain-before-flock-release) + `serve_multi`; per-mosaic daemons + the process-global resource caps/semaphore (§5/§6); **add `LoroEngine::has_unbroadcast_tail()`** (§5/g2) for the flush-before-evict gate; resolve the AutoSync-scope gate (§14/g3, interim = default-only). *Verify:* open/evict/reopen a mosaic over a live server; flush-before-evict test asserting no un-broadcast tail is stranded; close-while-request-in-flight holds the flock until the lease drops.
5. **`mmos.discovery`** — `GET /mosaics` over the registry + per-mosaic pair-bundle endpoint (per-mosaic material envelope, §7.1/§7.2) + host-identity advert split (§6); CLI `init` adds to registry (§7). *Verify:* pairing a sim imports ≥2 mosaics into iOS `MosaicRegistry`, each with its own group.
6. **`mmos.desktop`** — in-app switcher (retire the restart dance), optional multi-window (§9). *Verify:* desktop build + switch between two mosaics with no relaunch (named human check).

## 13. Non-goals (explicit)

- **No on-disk format change** — the registry is in-memory over existing `.tesela/` dirs (§8).
- **No collapsing mosaics into one relay group** — per-mosaic group_id/group_key stays (§7); rotation stays per-mosaic.
- **No cross-mosaic queries / cross-mosaic links** — a request targets exactly one mosaic; a "search all mosaics" surface is a separate future item.
- **No per-mosaic mDNS advert** — one service per device (§6).
- **No multi-window desktop in v1** — single window + in-app switcher; windows-per-mosaic is a no-server-change fast-follow (§9).
- **No header-based selector in v1** — path prefix only; the header door is noted, not built (§4.2).
- **No auth/tenancy model** — this is one *user's* N mosaics on their own device, not multi-tenant hosting. Loopback-embed + LAN/relay trust boundaries are unchanged.
- **No change to qql/b8v per-note semantics** — mmos is a strictly coarser eviction axis layered above them (§1/§5).

## 14. Open questions (resolve before/within impl)

- **`MosaicId` derivation** (§4.1): basename-slug + path-hash-on-collision, or a registry-assigned opaque id persisted in global config? (Recommendation: basename slug + short path-hash on collision — human-readable URLs, stable across restarts, independent of the rotatable group_id.)
- **Eviction policy knobs** (§5): max resident mosaics + idle TTL — default values? Never-evict-foreground is assumed; is that enough, or also pin the last-N MRU?
- **Pair-bundle default selection** (§7.2): user-consented/user-selected is DECIDED (§7.1 — never auto-share all group keys). Remaining: does the selection UI default to all-hosted-checked or none-checked? (Recommendation: none-checked / opt-in per mosaic, to avoid over-sharing group keys by a stray tap.)
- **`AutoSync` (Apple Reminders / EventKit) scope — device-global vs per-mosaic (glm g3).** `auto_sync` sits in per-mosaic state today, but it drives a **device-global OS store** (EventKit, serialized through one `Mutex`, `reminders/auto.rs`). N mosaics syncing tasks into ONE Apple Reminders store collide — which mosaic owns a given reminder? Round-tripping a completion can't tell them apart. Options: (a) only the default/foreground mosaic runs `AutoSync`; (b) namespace per mosaic (a Reminders list/calendar per mosaic); (c) keep per-mosaic but gate to one active at a time. **Unresolved — must be answered before `mmos.lifecycle` spawns AutoSync per open mosaic** (today it is `ServerState`-adjacent enough that the safe interim is: run it for the default mosaic only).
- **Host-identity persistence (gpt-5.5 m2).** The process-global mDNS advert needs a host id decoupled from any mosaic's `device_id` (§6). Persist where — a new `<mosaic_root_dir()>/.tesela-host/host_id`, a key in global `Config`, or derive-and-cache from hostname? (Recommendation: a persisted process-global host id file at the mosaic-root level; do not reuse a mosaic `device_id`.)
- **`keymap-config` / `transcription` scope**: per-mosaic (they read/write under `<mosaic>/.tesela` or global)? Default per-mosaic if the persistence lives under the mosaic; else keep on `ServerState`. Audit each at `mmos.extractor` time (`state.mosaic_root` appears in `routes/keymap.rs` + `routes/transcription.rs`, 1 line each — verified).
- **Recovery-phrase ergonomics with N mosaics (§7.1).** Recovery is per-mosaic (1 phrase per group). Do power users with many mosaics get N separate phrases to safeguard, or a device-level *wrapper* (a master secret that in turn holds/derives the per-mosaic ContentKeys)? A wrapper is out of v1 scope but must not be precluded by the pair-bundle envelope (§7.1). (Recommendation: v1 = per-mosaic phrases; revisit a wrapper with the key-hierarchy implementation.)
- **Standalone-server explicit registered-list**: do power users need to register mosaics OUTSIDE `mosaic_root_dir()` beyond the `--mosaic` default + cwd-walk? (v1: no — root-scan + default + explicit `POST /mosaics`.)

## Appendix A — Review responses (rev 2)

Every finding from the glm-5.2 (approve-with-nits) and gpt-5.5 (reject) reviews is addressed above. Where a reviewer's framing needed correction rather than plain acceptance, it is called out.

**Converged HIGH — slot lifecycle race + flock-release-under-load** (both reviewers). ACCEPTED in full. §4.1a now defines the five-state machine (`Closed → Opening → Open → Closing → Closed`) with an `Opening` barrier concurrent openers await (kills the same-process 409), a lease/refcount that pins an open mosaic, and a `Closing` drain that releases the flock **last** (after leases reach 0). §5's table and §10's invariants were re-anchored to it.

**gpt-5.5 HIGH — pairing vs recovery-phrase identity.** ACCEPTED, with a premise correction. The reviewer's framing ("one recovered `group_id`/key vs N mosaics") assumes a device-global identity; reconciling against decisions.md 2026-07-02 shows the model is **per-mosaic** — 1 mosaic = 1 group = 1 ContentKey = 1 BIP39 phrase (§7.1). So there was never a single recovered key to reconcile: recovery, group membership, and the phrase are all per-mosaic. That *is* the reconciliation, and §7.1/§7.2 now state the mapping, the per-mosaic user-consented authorization for exporting multiple keys, and the raw→wrapped-additive envelope forced by the (locked-but-gated) key-hierarchy ADR.

**gpt-5.5 MEDIUM m1 (fairness/backpressure), m2 (mDNS vs per-mosaic DeviceId), m3 (process-global `public_url`).** ACCEPTED. m1 → §6 scheduling/limits model (shared runtime, resident cap, global semaphore for heavy blocking ops, jitter, the AutoSync mutex chokepoint). m2 → §6 host-identity/`DeviceId` split with group-membership filtering (no default-mosaic special-casing) + §14 persistence question. m3 → §2 + §4.1 field-split corrected; `public_url` (bind-derived, verified `lib.rs:356/537`) moves to `ServerState`, `relay_url` confirmed per-mosaic.

**glm MEDIUM g1 (no inline heavy work in the extractor), g2 (broadcast_cursor has no accessor), g3 (AutoSync device-global).** ACCEPTED. g1 → §4.3 requires `spawn_blocking` for the open path, mirroring `data_ops.rs`/`reminders/darwin.rs`, run once by the barrier winner. g2 → §5 corrects the citation (`broadcast_cursor` is a **private field** at `crates/tesela-sync/src/engine/loro_engine.rs:293`, no accessor) and specs a new `LoroEngine::has_unbroadcast_tail()` method with a signature sketch, owned by `mmos.lifecycle`. g3 → §14 open question with an interim (default-mosaic-only AutoSync) and three resolution options.

**LOW / nits.** ACCEPTED. (a) Count corrected — `85` handler sites, `34` route lines / `37` crate-wide for `.mosaic_root` (verified via `rg`), not "86 / 56". (b) §3 now states daemon cancellation is a **signature change** per spawner (`presence_relay::spawn`, `presence_relay.rs:96`, returns `()` and spawns internally — cannot be cancelled by wrapping the call site). (c) §4.4 spells out `Option<Path<String>>` for the absent `{mosaic_id}` on the root alias, and grounds the vite rewrite claim (`web/vite.config.ts:16`). (d) §11 gains items (x) no filesystem-only mosaic persistence and (y) no single-global relay identity. Also corrected: the prior draft's "find-and-mirror the existing extractor idioms" — verified there are **zero** custom `FromRequestParts` impls in the crate, so §4.3 now says implement fresh per axum docs + `FromRef`.

**Net verdict on gpt-5.5's reject:** the "not implementation-ready" gaps (lifecycle states, identity coherence, resource model, the field-split and non-existent accessor) are now closed with grounded citations and named-new-method sketches. No finding was dismissed.

## Verify

None — design spec. The orchestrator Lead-reviews. Each derived implementation bead (§12) carries its own `Verify:` (`cargo test -p tesela-server`, a two-mosaic integration test, `cargo build -p tesela-desktop`, or a named desktop human check), mirroring the phasing above.

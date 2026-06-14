# L4 — tesela-server lib-embed `serve()` (spec)

**Status:** in progress (2026-06-14). Scope = L. **Staged cutover** (Taylor's call 2026-06-14): the library refactor + tests land on `main`; the desktop in-process cutover lands on a **separate branch** Taylor live-tests before it's default.

## Goal

Turn `tesela-server` (today **bin-only**, all boot logic inline in `#[tokio::main] async fn main()`) into a **linkable library** exposing a `serve()`/`bind()` entry point, so the desktop Tauri app can run the server **in-process** (on Tauri's own tokio runtime) instead of spawning it as a child process — retiring the child-spawn + `wait_for_port` + `ServerChild` reaper + parent-death watchdog. Mirrors the existing **`tesela-relay` lib+bin split**.

## Two phases

### Phase A — library refactor (lands on `main`, behavior-identical bin)

Mirror `crates/tesela-relay` (lib.rs holds the reusable surface; main.rs is a thin `#[tokio::main]`).

1. **Cargo.toml:** add `[lib] name="tesela_server" path="src/lib.rs"` alongside the existing `[[bin]]` (bin keeps `path="src/main.rs"`). Follow relay's Cargo.toml.
2. **src/lib.rs:** declare the existing modules `pub mod` (backup_scheduler, error, notifications, reminders, routes, state, sync_relay, …); re-export `pub use state::AppState`, `pub use routes::build`.
3. **`ServeConfig`** struct capturing what `main()` reads from env/CLI today (mosaic, bind, static_dir, `TESELA_DISABLE_{MDNS,PEER_SYNC,RELAY}`, relay_url, …) + `ServeConfig::from_env()` reproducing today's resolution so the **standalone bin behaves identically**.
4. **Split build/run so the embedder gets the bound addr before awaiting serve:**
   - `pub async fn bind(config) -> Result<Bound>` where `struct Bound { addr: SocketAddr, lock: std::fs::File /*flock guard*/, app: Router, shutdown: <daemon cancel handles> }`. Binds `127.0.0.1:0`-capable, returns the **actual** `SocketAddr`.
   - `impl Bound { pub async fn serve(self, shutdown: impl Future<Output=()> + Send + 'static) -> Result<()> }` — runs `axum::serve(...).with_graceful_shutdown(shutdown)`, then stops the indexer + runs auto-backup-on-quit, then cancels the daemons.
   - Convenience `pub async fn serve(config, shutdown)` = `bind(config).await?.serve(shutdown).await` for the bin.
5. **`serve()` is a plain `async fn`, NEVER `#[tokio::main]`** — it is driven by the embedder's runtime (nesting a runtime panics inside Tauri).
6. **Flock guard moves into `Bound`** and is held by the embedder for the window lifetime — NOT dropped when `bind()` returns (dropping releases the single-writer lock ⇒ data-safety break).
7. **Daemon shutdown (the load-bearing concern).** The ~5 detached `tokio::spawn` loops (sync_daemon, relay tick, reminders, notifications, backup scheduler) currently have NO shutdown — they survive only because process-exit kills them. On the embedder's persistent runtime they'd leak after the window closes. Give them a shared `CancellationToken` (or broadcast shutdown) cloned into each loop body, `select!`-ed in the loop, cancelled after `serve()` returns. Model: the existing `IndexerHandle::stop()` (tesela-core `indexer.rs`).
8. **main.rs becomes thin:** `tesela_server::serve(ServeConfig::from_env(), wait_for_shutdown_signal()).await` (keep `wait_for_shutdown_signal` for the bin).

**Phase A acceptance:** existing `cargo test -p tesela-server` (integration tests spawn the BIN — must still pass, proving bin behavior-identical) + a **new lib-level test**: `bind(config)` on `127.0.0.1:0` ⇒ `addr.port()!=0`, hit `/health`, fire the shutdown future ⇒ it completes, assert auto-backup ran + no lingering daemons. `cargo build -p tesela-server`.

### Phase B — Tauri cutover (lands on a BRANCH, default off until Taylor live-tests)

9. `src-tauri/Cargo.toml`: add `tesela-server = { path = "../crates/tesela-server" }`.
10. `src-tauri/main.rs`: delete `spawn_server` / `wait_for_port` / `tesela_server_bin` / `ServerChild` / SIGTERM reaper. Build a `ServeConfig` (loopback bind, static_dir, disable flags — same values `spawn_server` set), `let bound = tesela_server::bind(config).await?` **on Tauri's tokio runtime**, read `bound.addr` to format `http://{addr}/g` for `run_app`, spawn the serve future as a managed task, and fire the shutdown future + await drain on `RunEvent::Exit` (instead of killing a child).

**Phase B hazards (must each be handled):**
- **Daemon leak** (see #7) — the single biggest correctness risk.
- **`/server/restart`** (`routes/data_ops.rs`) re-execs `std::env::current_exe()` — in-process that's the **Tauri** binary. Disable/handle for the embed.
- **Runtime nesting** — `serve()` must be a plain async fn (see #5).
- **Flock lifetime** — `Bound.lock` held for the window lifetime (see #6).
- **Bound-port race** — eliminate the old pre-pick TOCTOU by binding `:0` in-process and building the URL *after* `bind()` resolves (changes `run_app` ordering).
- **Single-instance plugin** — the 2nd launch's focus-existing path must run BEFORE `bind()` so it focuses rather than erroring on the flock.
- **Auto-backup-on-quit** can exceed 5s (VACUUM INTO) — the Exit path must await `serve()` drain before the process exits (mirror today's 30s grace).
- **whisper-rs/objc2/EventKit** link moves into the desktop binary (already pulled via the child today; now a compile/link edge in src-tauri).

**Phase B acceptance (Taylor live-tests on the branch):** desktop launches + renders `/g` on the real mosaic, edits persist; standalone `tesela-server` on the same mosaic fails fast on the flock; 2nd launch focuses; Cmd+Q ⇒ NO lingering `tesela-server` proc (there is none) + auto-backup written; live WS/relay edits still flow.

## Notes
- `tesela-relay`'s lib+bin split is the conformance reference (`cargo build -p tesela-relay` proves the pattern in-workspace).
- The parent-death watchdog (`spawn_parent_death_watchdog`) becomes dead code for the embed (no separate process) — leave it gated behind `TESELA_EXIT_WITH_PARENT` (never set by the embed) for the standalone bin.

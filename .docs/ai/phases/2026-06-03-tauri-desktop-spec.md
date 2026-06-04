# Tauri Desktop App — Spec (2026-06-03)

## Decision (locked)

Desktop app = **Tauri-wrap the SvelteKit `/g` UI**, NOT a fresh SwiftUI macOS app.
Rationale (full discussion in `decisions.md` 2026-06-03): `/g` is the most mature,
hardest-won surface (CodeMirror+vim, ⌘K, leader, Loro collab); reuse it 100% and
build step-3 features ONCE for web+desktop. SwiftUI would rebuild it all, starting
from the iOS app which is *behind* `/g`. Web is the de-facto-canonical UI (Taylor
daily-drives it). Native-feel cost is near-zero for a vim user living in a controlled
CodeMirror surface. Reversible: keep the FFI/iOS path; a native SwiftUI Mac shell stays
shelved as a possible premium tier.

## Architecture — "native window + loopback child server that serves API + UI"

```
Tauri app (single .app, nothing to run separately)
├─ native window  →  http://127.0.0.1:<port>/g
└─ Rust shell (src-tauri)
   └─ child process: tesela-server, bound 127.0.0.1:<port>
      ├─ serves the API at root (/notes, /loro, /ws, /sync, …)
      └─ serves the pre-built /g UI (TESELA_STATIC_DIR, SPA fallback)
         → SAME ORIGIN as the API ⇒ no CORS, no base injection gymnastics
```

**Why the embedded server serves BOTH API and UI:** the webview loads from
`http://127.0.0.1:<port>`, so `/notes` / `/loro` / `/ws` are same-origin — no CORS,
and the existing `window.location.host`-based WS URL just works. The only frontend
change is the API base prefix (`/api` in vite-dev → `""` same-origin root in the app).

**Sync-node model (the load-bearing design rule):** the embedded server binds
**127.0.0.1 only** — it is a *loopback Loro-replica node*, NOT a hub other devices
point at. Cross-device sync flows through the spine (cloud relay + LAN), the same
transport as iOS. The webview↔server HTTP is purely local UI plumbing. Do NOT let it
bind 0.0.0.0 / become a hub (that's the "Mac as hub" posture the spine retires).
Transition note: until the relay live-tick (1b-iii) lands, iOS still points at the
*standalone* Mac server over Tailscale; the Tauri app is additive and loopback-only,
so it doesn't disturb that. The standalone server and the Tauri app must not both
materialize the SAME mosaic simultaneously (two-writer problem) — for now run one.

## Tasks (see roadmap #196–#200)

- **T1** tesela-server: optional `TESELA_STATIC_DIR` → tower-http `ServeDir` fallback
  (after API routes) with SPA `index.html` fallback. Unset = today's behavior.
- **T2** web: adapter-static + SPA (`ssr=false`); `runtime-base.ts` `apiBase()` =
  `window.__TESELA_API_BASE__ ?? "/api"`; wire api-client `BASE_URL` + loro
  `note-doc.defaultBase` to it. WS already host-derived. `pnpm build` → `web/build`.
- **T3** `src-tauri/`: Tauri 2 crate. Spawn the server child (free loopback port,
  `--mosaic`, `TESELA_STATIC_DIR`), wait for `/health`, window → `…/g`, inject
  `window.__TESELA_API_BASE__=""`, kill child on exit.
- **T4** run + verify end-to-end (embedded, real mosaic, edit round-trip, no separate
  server, loopback-only, no zombie child).
- **T5** adversarial review (loopback security / origin model / child lifecycle /
  sync-node correctness) + docs/memory + commit.

## v1 scope / deferred

- **MVP (this phase):** working native window, embedded server (child process),
  serves `/g` on the real mosaic, edits persist. That's "something real."
- **Deferred follow-ups:** lib-embed `tesela_server::serve()` (one process instead of a
  child — architecturally identical, a purification); native menus/tray; auto-update;
  codesign + notarization for distribution; the loopback-vs-hub flip once the relay
  live-tick lands; route-collision hardening for hard-nav deep-links into frontend
  paths that shadow API paths (SPA client-nav is unaffected; `/g` entry doesn't collide).

## Verify commands

- `cargo build -p tesela-server` (T1) · `cd web && pnpm build && pnpm check` (T2)
- `cargo build` in `src-tauri/` (T3) · launch the app, edit a note, confirm on disk (T4)

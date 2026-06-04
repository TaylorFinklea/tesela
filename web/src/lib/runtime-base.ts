/**
 * Resolves the base URL the client uses to reach tesela-server, across the
 * two deployment shapes:
 *
 *  - **vite-dev / hosted web:** `apiBase()` is `/api`. The vite dev server (or
 *    the host's reverse proxy) rewrites `/api/*` → the server, stripping `/api`,
 *    so `/api/notes` lands on the server's `/notes` route.
 *  - **desktop (Tauri):** the Tauri shell injects `window.__TESELA_API_BASE__ = ""`
 *    via an initialization script that runs BEFORE this bundle. The embedded
 *    tesela-server serves both the API and this UI on the same loopback origin,
 *    so an empty base makes `/notes` / `/loro` same-origin requests — no proxy,
 *    no CORS. (An absolute base like `http://127.0.0.1:47474` is also honored,
 *    should the shell ever serve the UI itself and point the API cross-origin.)
 *
 * `??` (not `||`) is deliberate: an injected `""` must be preserved as
 * same-origin, not fall through to `/api`.
 */
export function apiBase(): string {
  if (typeof window !== "undefined") {
    const injected = (window as Window & { __TESELA_API_BASE__?: string }).__TESELA_API_BASE__;
    if (typeof injected === "string") return injected;
  }
  return "/api";
}

import { redirect } from "@sveltejs/kit";

// B3: the legacy Prism v4 chrome is gone — Graphite (/g) is the only web
// UI. This stub keeps stale `/v4` bookmarks/muscle-memory from hitting a
// 404; `/` itself forwards into `/g`.
export function load() {
  throw redirect(307, "/");
}

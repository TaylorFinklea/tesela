<script lang="ts">
  /*
   * `/v4` entry. URL → state adapter; renders nothing visible (the
   * layout owns the whole shell).
   *
   * Two responsibilities, in order:
   *
   * 1. Consume `#tile=<slug>` on mount. The Phase 6 default-route swap
   *    redirects `/p/<slug>` into `/v4#tile=<slug>`; we read the hash
   *    on first run, jumpToTile into the focused pane, and clear the
   *    hash so the URL settles at `/v4`.
   *
   * 2. Otherwise, when the focused pane is a fresh empty editor in a
   *    tab we haven't seen, seed it with today's daily note. Covers
   *    both the very first /v4 mount and any ⌘T afterwards. A tab the
   *    user has deliberately emptied stays empty because its id is
   *    already in the seen set.
   */
  import { api } from "$lib/api-client";
  import {
    getFocusedPane,
    getFocusedTab,
    jumpToTile,
  } from "$lib/stores/pane-tree.svelte";

  const seenTabs = new Set<string>();
  let consumedHash = false;

  $effect(() => {
    // ── (1) hash consume ────────────────────────────────────────────
    if (typeof window !== "undefined" && !consumedHash) {
      consumedHash = true;
      const hash = window.location.hash;
      const prefix = "#tile=";
      if (hash.startsWith(prefix)) {
        const id = decodeURIComponent(hash.slice(prefix.length));
        // Strip the hash so the URL settles at /v4 (no permanent
        // tile=... pollution). `replaceState` skips a navigation cycle.
        history.replaceState(null, "", "/v4");
        if (id) {
          jumpToTile(id, "url");
          return;
        }
      }
    }

    // ── (2) daily seed ──────────────────────────────────────────────
    const tab = getFocusedTab();
    const pane = getFocusedPane();
    if (!tab || !pane) return;
    if (seenTabs.has(tab.id)) return;
    seenTabs.add(tab.id);
    if (pane.kind !== "editor" || pane.tiles.length > 0) return;
    api
      .getDailyNote()
      .then((daily) => jumpToTile(daily.id))
      .catch((e) => console.error("v4: failed to seed daily note", e));
  });
</script>

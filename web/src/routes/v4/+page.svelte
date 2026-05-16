<script lang="ts">
  /*
   * `/v4` entry under Prism v5.
   *
   * Two responsibilities:
   *
   * 1. Consume `#tile=<slug>` on mount. The Phase 6 (v4) default-route swap
   *    redirects `/p/<slug>` into `/v4#tile=<slug>`; we read the hash on
   *    first run, seed the focused leaf with that page, and clear the hash.
   *
   * 2. Otherwise, scan the active tab's leaves and seed today's daily
   *    into every empty page buffer (targets a specific leaf id rather
   *    than the focused one, so derived/ambient panes are untouched).
   */
  import { api } from "$lib/api-client";
  import {
    getActiveTab,
    openPageInFocused,
    openPageInLeaf,
  } from "$lib/buffer/state.svelte";
  import { asPageId, type LeafId } from "$lib/buffer/types";

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
        history.replaceState(null, "", "/v4");
        if (id) {
          openPageInFocused(asPageId(id));
          return;
        }
      }
    }

    // ── (2) daily seed ──────────────────────────────────────────────
    const tab = getActiveTab();
    if (!tab) return;
    if (seenTabs.has(tab.id)) return;
    seenTabs.add(tab.id);

    const emptyLeafIds: LeafId[] = [];
    function walk(node: import("$lib/buffer/types").Node): void {
      if (node.type === "leaf") {
        if (node.buffer.kind === "page" && node.buffer.pageId === "") {
          emptyLeafIds.push(node.id);
        }
      } else {
        walk(node.children[0]);
        walk(node.children[1]);
      }
    }
    walk(tab.layout);
    if (emptyLeafIds.length === 0) return;

    api
      .getDailyNote()
      .then((daily) => {
        for (const id of emptyLeafIds) {
          openPageInLeaf(id, asPageId(daily.id));
        }
      })
      .catch((e) => console.error("v5: failed to seed daily note", e));
  });
</script>

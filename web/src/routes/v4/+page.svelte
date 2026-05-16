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
   * 2. Otherwise, if the active tab's focused leaf is an empty page-buffer,
   *    fetch today's daily and seed it. Covers fresh boots and migration
   *    paths where the focused leaf came over from v4 with an empty pageId.
   */
  import { api } from "$lib/api-client";
  import {
    getActiveTab,
    openPageInFocused,
  } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

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

    // Walk leaves to find the first empty page-buffer.
    let needsDaily = false;
    function walk(node: import("$lib/buffer/types").Node): void {
      if (node.type === "leaf") {
        if (node.buffer.kind === "page" && node.buffer.pageId === "") {
          needsDaily = true;
        }
      } else {
        walk(node.children[0]);
        walk(node.children[1]);
      }
    }
    walk(tab.layout);
    if (!needsDaily) return;

    api
      .getDailyNote()
      .then((daily) => openPageInFocused(asPageId(daily.id)))
      .catch((e) => console.error("v5: failed to seed daily note", e));
  });
</script>

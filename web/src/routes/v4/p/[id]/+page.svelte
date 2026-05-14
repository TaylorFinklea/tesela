<script lang="ts">
  /*
   * `/v4/p/<slug>` — URL deep-link into a tile. Like `/v4/+page.svelte`
   * this is a URL→state adapter that renders nothing visible (the v4
   * layout owns the shell).
   *
   * Reconciliation: if any editor pane (in any tab) already shows the
   * tile, switch to that tab and focus that pane — don't open a
   * duplicate. Otherwise jump the tile into the focused editor pane.
   *
   * Per the locked redesign decision the pane tree is NOT URL-encoded;
   * this is a one-shot entry point for external deep-links / new-tab
   * opens. Intra-app navigation is tracked by the Journey breadcrumb
   * (Phase 5), not the URL.
   */
  import { page } from "$app/state";
  import {
    getTileLocation,
    switchTab,
    focusPane,
    jumpToTile,
  } from "$lib/stores/pane-tree.svelte";

  function reconcile(slug: string) {
    const hit = getTileLocation(slug);
    if (hit) {
      switchTab(hit.tabId);
      focusPane(hit.row, hit.col);
    } else {
      jumpToTile(slug);
    }
  }

  // Re-run whenever the slug changes (covers browser back/forward and
  // client-side navigations between `/v4/p/*` routes).
  $effect(() => {
    const slug = page.params.id;
    if (slug) reconcile(slug);
  });
</script>

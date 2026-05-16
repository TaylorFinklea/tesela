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
   * 2. Otherwise, seed every empty editor leaf in the active tab with
   *    today's daily note. Covers the first /v4 mount, ⌘T new tabs,
   *    and stale localStorage where focus may have ended up on a
   *    non-editor pane. If at least one editor was filled, also park
   *    focus on it so the user can type immediately.
   */
  import { api } from "$lib/api-client";
  import {
    focusPane,
    getFocusedPane,
    getFocusedTab,
    jumpToTile,
  } from "$lib/stores/pane-tree.svelte";
  import { leaves } from "$lib/stores/pane-tree";

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
          jumpToTile(id, "url");
          return;
        }
      }
    }

    // ── (2) daily seed ──────────────────────────────────────────────
    const tab = getFocusedTab();
    if (!tab) return;
    if (seenTabs.has(tab.id)) return;
    seenTabs.add(tab.id);

    // Collect every editor leaf in this tab. The first empty one gets
    // today's daily; subsequent empties also get it so a fresh layout
    // with several panes all land on something useful.
    const editorIds: string[] = [];
    const emptyEditorIds: string[] = [];
    for (const leaf of leaves(tab.layout)) {
      if (leaf.pane.kind === "editor") {
        editorIds.push(leaf.pane.id);
        if (leaf.pane.tiles.length === 0) emptyEditorIds.push(leaf.pane.id);
      }
    }
    if (emptyEditorIds.length === 0) return;

    api
      .getDailyNote()
      .then((daily) => {
        // Park focus on an editor (preferring an empty one) so the
        // first keystroke lands in the daily, not on the widget pane.
        const prior = getFocusedPane();
        const target = emptyEditorIds[0];
        focusPane(target);
        jumpToTile(daily.id);
        // Restore prior focus only if it was *already* an editor —
        // otherwise leaving the user on the widget pane was the bug
        // we're trying to fix.
        if (prior?.kind === "editor" && editorIds.includes(prior.id)) {
          focusPane(prior.id);
        }
      })
      .catch((e) => console.error("v4: failed to seed daily note", e));
  });
</script>

<script lang="ts">
  /*
   * `/v4` entry. This component is the URL→state adapter — it renders
   * nothing visible (the layout owns the whole shell).
   *
   * Bootstrap: whenever the active tab becomes one we've never seen
   * before AND its focused pane is a fresh empty editor, seed it with
   * today's daily note. This covers both the very first `/v4` mount and
   * any `⌘T` afterwards (the layout's `+`/`⌘T` handler also resolves to
   * `newTab()` which switches activeTabId; the effect picks it up). A
   * tab the user has deliberately emptied stays empty because its id is
   * already in the seen set.
   */
  import { api } from "$lib/api-client";
  import {
    getFocusedPane,
    getFocusedTab,
    jumpToTile,
  } from "$lib/stores/pane-tree.svelte";

  const seenTabs = new Set<string>();

  $effect(() => {
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

<script lang="ts">
  /*
   * `/v4` entry. This component is the URL→state adapter — it renders
   * nothing visible (the layout owns the whole shell). Phase 1: if the
   * focused pane is an empty editor, seed it with today's daily note so
   * `/v4` has a sensible landing. Phase 3 grows this into full URL deep-
   * link routing (`/v4/p/<slug>` etc.).
   */
  import { onMount } from "svelte";
  import { api } from "$lib/api-client";
  import { getFocusedPane, jumpToTile } from "$lib/stores/pane-tree.svelte";

  onMount(async () => {
    const pane = getFocusedPane();
    if (pane?.kind === "editor" && pane.tiles.length === 0) {
      try {
        const daily = await api.getDailyNote();
        jumpToTile(daily.id);
      } catch (e) {
        console.error("v4: failed to seed daily note", e);
      }
    }
  });
</script>

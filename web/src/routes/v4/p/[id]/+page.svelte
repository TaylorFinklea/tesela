<script lang="ts">
  /*
   * `/v4/p/<slug>` — URL deep-link into a page. v5: opens the slug as a
   * page buffer in the focused leaf. The pane tree is NOT URL-encoded;
   * intra-app navigation is tracked by Journey, not the URL.
   */
  import { page } from "$app/state";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  $effect(() => {
    const slug = page.params.id;
    if (slug) openPageInFocused(asPageId(slug));
  });
</script>

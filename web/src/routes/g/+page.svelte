<!-- web/src/routes/g/+page.svelte -->
<script lang="ts">
  /*
   * `/g` — the Graphite shell, and (since the B2 cutover) the app's
   * default chrome (`/` 307s here). Besides mounting the shell, this
   * page owns the boot-time entry responsibilities the v4 page used to
   * carry:
   *
   * 1. Consume `#tile=<slug>` on mount. The `/p/<slug>` redirect carries
   *    the slug across in the hash; we seed the focused buffer with that
   *    page and clear the hash. (No daily-seed counterpart: the shell's
   *    default empty page buffer already renders the journal natively.)
   *
   * 2. Scratch prune sweep — once per local-day, same as v4's boot hook.
   */
  import GraphiteShell from '$lib/graphite/shell/GraphiteShell.svelte';
  import { getScratchPruneAfterDays, openPageInFocused } from '$lib/buffer/state.svelte';
  import { asPageId } from '$lib/buffer/types';
  import { maybeRunScratchPruneAtBoot } from '$lib/state/scratch-prune';

  let consumedHash = false;
  let prunedThisBoot = false;

  $effect(() => {
    if (!prunedThisBoot) {
      prunedThisBoot = true;
      const days = getScratchPruneAfterDays();
      if (days && days > 0) {
        maybeRunScratchPruneAtBoot(days).catch((e) =>
          console.warn('scratch prune failed', e),
        );
      }
    }

    if (typeof window !== 'undefined' && !consumedHash) {
      consumedHash = true;
      const hash = window.location.hash;
      const prefix = '#tile=';
      if (hash.startsWith(prefix)) {
        const id = decodeURIComponent(hash.slice(prefix.length));
        history.replaceState(null, '', '/g');
        if (id) openPageInFocused(asPageId(id));
      }
    }
  });
</script>

<GraphiteShell />

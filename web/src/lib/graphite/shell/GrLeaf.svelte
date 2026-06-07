<!-- web/src/lib/graphite/shell/GrLeaf.svelte -->
<script lang="ts">
  /*
   * One leaf of the Graphite pane tree. Routes the leaf's buffer to the
   * matching Graphite view — the SAME mapping GraphiteShell used for its
   * single pane (A6): a daily page (empty pageId or YYYY-MM-DD) -> GrDaily,
   * any other page -> GrPage, the inbox/agenda ambients -> GrInbox/GrAgenda,
   * everything else -> the placeholder card.
   *
   * Clicking the leaf focuses it (so the leader / palette / colon verbs and
   * the focused-page Loro doc follow it). The accent top border marks the
   * focused pane, but only when more than one is open (`showFocus`) — a lone
   * pane renders exactly as the pre-split shell did, with no border.
   */
  import type { Buffer, LeafId } from '$lib/buffer/types';
  import { focusLeaf } from '$lib/buffer/state.svelte';
  import GrPane from './GrPane.svelte';
  import GrDaily from '$lib/graphite/views/GrDaily.svelte';
  import GrPage from '$lib/graphite/views/GrPage.svelte';
  import GrInbox from '$lib/graphite/views/GrInbox.svelte';
  import GrAgenda from '$lib/graphite/views/GrAgenda.svelte';

  let {
    leafId,
    buffer,
    focused,
    showFocus,
  }: {
    leafId: LeafId;
    buffer: Buffer;
    focused: boolean;
    showFocus: boolean;
  } = $props();

  // A daily page is either the default (empty pageId) leaf or a page whose
  // id is a YYYY-MM-DD date — those render as the continuous JournalView.
  function isDailyPageId(pageId: string): boolean {
    return pageId === '' || /^\d{4}-\d{2}-\d{2}$/.test(pageId);
  }

  type ViewKind = 'daily' | 'page' | 'inbox' | 'agenda' | 'placeholder';
  const view = $derived.by<ViewKind>(() => {
    if (buffer.kind === 'page') return isDailyPageId(buffer.pageId) ? 'daily' : 'page';
    if (buffer.kind === 'ambient') {
      if (buffer.ambientName === 'inbox') return 'inbox';
      if (buffer.ambientName === 'agenda') return 'agenda';
    }
    return 'placeholder';
  });

  const pageId = $derived(buffer.kind === 'page' ? buffer.pageId : '');

  const title = $derived.by(() => {
    if (buffer.kind === 'page')
      return isDailyPageId(buffer.pageId) ? 'Journal' : (buffer.pageId || 'Untitled page');
    if (buffer.kind === 'derived') return buffer.rendererName;
    if (buffer.kind === 'ambient') return buffer.ambientName;
    return 'Graphite';
  });

  function onLeafClick() {
    // Bubble-phase: inner editors place their caret first, then this leaf
    // becomes the focused pane. Skip the commit when already focused.
    if (!focused) focusLeaf(leafId);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="gr-leaf"
  class:gr-leaf-multi={showFocus}
  class:focused
  data-leaf-id={leafId}
  onclick={onLeafClick}
>
  {#if view === 'daily'}
    <GrPane {title} variant="focus">
      {#key pageId}
        <GrDaily anchorDate={/^\d{4}-\d{2}-\d{2}$/.test(pageId) ? pageId : undefined} />
      {/key}
    </GrPane>
  {:else if view === 'page'}
    {#key pageId}
      <GrPage {pageId} paneId={leafId as unknown as string} />
    {/key}
  {:else if view === 'inbox'}
    <GrInbox />
  {:else if view === 'agenda'}
    <GrAgenda />
  {:else}
    <GrPane {title} variant="focus">
      <div class="gr-placeholder">
        <div class="ph-title">{title}</div>
        <div class="ph-sub">This view lands in a later phase.</div>
      </div>
    </GrPane>
  {/if}
</div>

<style>
  .gr-leaf {
    flex: 1;
    display: flex;
    flex-direction: row;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* Focus accent only when split — a lone pane keeps the pre-split look.
   * The 2px border is reserved on every split leaf (transparent when
   * unfocused) so moving focus never shifts content. */
  .gr-leaf.gr-leaf-multi {
    border-top: 2px solid transparent;
  }
  .gr-leaf.gr-leaf-multi.focused {
    border-top-color: var(--coral);
  }
  .gr-placeholder {
    height: 100%;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
  }
  .gr-placeholder .ph-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--fg2);
  }
  .gr-placeholder .ph-sub {
    font-size: 12.5px;
    color: var(--faint);
  }
</style>

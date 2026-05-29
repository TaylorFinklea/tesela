<!-- web/src/lib/graphite/shell/GraphiteShell.svelte -->
<script lang="ts">
  /*
   * Graphite shell root. Composes the chrome — top bar / (rail + main with
   * one pane) / status — and mounts the ⌘K palette + Space leader overlays.
   *
   * Behavior is 100% reused. The capture-phase keydown listener below mirrors
   * web/src/routes/v4/+layout.svelte's wiring (the only place behavior
   * connects today): Space → openLeader(), ⌘K → openStation(), `:` →
   * openColonMode(), all behind the same text-entry guard (`isTextEntry`) and
   * the same plain-input exception for `:` so colons can still be typed into
   * settings fields. Only the markup + CSS are new.
   *
   * The `.gr-root` scope + tokens are provided by /g/+layout.svelte
   * (the foundation), so this component renders inside that.
   *
   * Pane content routes by the focused buffer's kind (A6): a daily (empty
   * pageId or a YYYY-MM-DD page) → GrDaily; any other page → GrPage; the
   * "inbox"/"agenda" ambients → GrInbox/GrAgenda; everything else falls
   * back to the placeholder.
   */
  import { onMount } from 'svelte';
  import GrTopBar from './GrTopBar.svelte';
  import GrRail from './GrRail.svelte';
  import GrPane from './GrPane.svelte';
  import GrStatus from './GrStatus.svelte';
  import GrCommandPalette from './GrCommandPalette.svelte';
  import GrLeaderOverlay from './GrLeaderOverlay.svelte';
  import GrDaily from '$lib/graphite/views/GrDaily.svelte';
  import GrPage from '$lib/graphite/views/GrPage.svelte';
  import GrInbox from '$lib/graphite/views/GrInbox.svelte';
  import GrAgenda from '$lib/graphite/views/GrAgenda.svelte';
  import { getFocusedBuffer, getFocusedLeafId } from '$lib/buffer/state.svelte';
  import { openStation } from '$lib/stores/station.svelte';
  import { openLeader } from '$lib/v5/leader-tree.svelte';
  import { openColonMode } from '$lib/stores/colon-mode.svelte';

  const focusedBuffer = $derived(getFocusedBuffer());
  const focusedLeafId = $derived(getFocusedLeafId());

  // A daily page is either the default (empty pageId) leaf or a page whose
  // id is a YYYY-MM-DD date — those render as the continuous JournalView.
  function isDailyPageId(pageId: string): boolean {
    return pageId === '' || /^\d{4}-\d{2}-\d{2}$/.test(pageId);
  }

  // Which top-level view the focused buffer maps to. `placeholder` keeps
  // the original "coming soon" card for any kind we don't render yet.
  type ViewKind = 'daily' | 'page' | 'inbox' | 'agenda' | 'placeholder';
  const view = $derived.by<ViewKind>(() => {
    const b = focusedBuffer;
    if (!b) return 'daily';
    if (b.kind === 'page') return isDailyPageId(b.pageId) ? 'daily' : 'page';
    if (b.kind === 'ambient') {
      if (b.ambientName === 'inbox') return 'inbox';
      if (b.ambientName === 'agenda') return 'agenda';
    }
    return 'placeholder';
  });

  // Pane title for the daily / placeholder GrPane head + the status path.
  const paneTitle = $derived.by(() => {
    const b = focusedBuffer;
    if (!b) return 'Journal';
    if (b.kind === 'page') return isDailyPageId(b.pageId) ? 'Journal' : (b.pageId || 'Untitled page');
    if (b.kind === 'derived') return b.rendererName;
    if (b.kind === 'ambient') return b.ambientName;
    return 'Graphite';
  });

  const activePageId = $derived(
    focusedBuffer?.kind === 'page' ? focusedBuffer.pageId : '',
  );

  // ── keydown wiring (mirror of v4/+layout.svelte) ─────────────────────────
  function isTextEntry(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el) return false;
    return (
      el.tagName === 'INPUT' ||
      el.tagName === 'TEXTAREA' ||
      el.isContentEditable ||
      !!el.closest?.('.cm-editor')
    );
  }

  function openCommandStation() {
    openStation({
      tab: 'palette',
      priorPaneId: getFocusedLeafId() as unknown as string | undefined,
    });
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey;

      // `:` opens ex-mode (colon command line), even inside a cm-editor —
      // but let plain HTML inputs/textareas/contenteditables keep the colon.
      if (!mod && !e.altKey && !e.ctrlKey && e.key === ':') {
        const t = e.target as HTMLElement | null;
        const inCmEditor = !!t?.closest?.('.cm-editor');
        const inPlainEntry =
          !inCmEditor &&
          (t?.tagName === 'INPUT' ||
            t?.tagName === 'TEXTAREA' ||
            !!t?.isContentEditable);
        if (!inPlainEntry) {
          e.preventDefault();
          e.stopPropagation();
          openColonMode();
          return;
        }
      }

      // Space opens the leader chord menu when NOT in a text entry.
      if (
        !mod &&
        !e.altKey &&
        !e.ctrlKey &&
        e.key === ' ' &&
        !isTextEntry(e.target)
      ) {
        e.preventDefault();
        e.stopPropagation();
        openLeader();
        return;
      }

      // ⌘K opens the command station.
      if (mod && !e.shiftKey && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        openCommandStation();
        return;
      }
    };
    document.addEventListener('keydown', onKey, true);

    // Mirror v4: the journal's cm-vim <Space> action fires `tesela:leader`
    // at document level — catch it so the leader still opens from inside the
    // editor.
    const onLeaderEvent = () => openLeader();
    document.addEventListener('tesela:leader', onLeaderEvent);

    return () => {
      document.removeEventListener('keydown', onKey, true);
      document.removeEventListener('tesela:leader', onLeaderEvent);
    };
  });
</script>

<div class="gr-shell">
  <GrTopBar />

  <div class="gr-body">
    <GrRail />
    <div class="gr-main">
      {#if view === 'daily'}
        <GrPane title={paneTitle} variant="focus">
          {#key activePageId}
            <GrDaily anchorDate={/^\d{4}-\d{2}-\d{2}$/.test(activePageId) ? activePageId : undefined} />
          {/key}
        </GrPane>
      {:else if view === 'page'}
        {#key activePageId}
          <GrPage pageId={activePageId} paneId={focusedLeafId as unknown as string | undefined} />
        {/key}
      {:else if view === 'inbox'}
        <GrInbox />
      {:else if view === 'agenda'}
        <GrAgenda />
      {:else}
        <GrPane title={paneTitle} variant="focus">
          <div class="gr-placeholder">
            <div class="ph-title">{paneTitle}</div>
            <div class="ph-sub">This view lands in a later phase.</div>
          </div>
        </GrPane>
      {/if}
    </div>
  </div>

  <GrStatus path={paneTitle} />

  <GrCommandPalette />
  <GrLeaderOverlay />
</div>

<style>
  .gr-shell {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-rows: 48px 1fr 30px;
    overflow: hidden;
  }
  .gr-body {
    display: flex;
    min-height: 0;
    overflow: hidden;
    position: relative;
    flex: 1;
  }
  .gr-main {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
  }
  .gr-placeholder {
    height: 100%;
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

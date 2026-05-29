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
   * Pane content is a placeholder this phase — the daily-driver views fill it
   * in the next plan.
   */
  import { onMount } from 'svelte';
  import GrTopBar from './GrTopBar.svelte';
  import GrRail from './GrRail.svelte';
  import GrPane from './GrPane.svelte';
  import GrStatus from './GrStatus.svelte';
  import GrCommandPalette from './GrCommandPalette.svelte';
  import GrLeaderOverlay from './GrLeaderOverlay.svelte';
  import { getFocusedBuffer, getFocusedLeafId } from '$lib/buffer/state.svelte';
  import { openStation } from '$lib/stores/station.svelte';
  import { openLeader } from '$lib/v5/leader-tree.svelte';
  import { openColonMode } from '$lib/stores/colon-mode.svelte';

  const focusedBuffer = $derived(getFocusedBuffer());

  // Placeholder pane title from the focused buffer (real views come next).
  const paneTitle = $derived.by(() => {
    const b = focusedBuffer;
    if (!b) return 'Graphite';
    if (b.kind === 'page') return b.pageId || 'Untitled page';
    if (b.kind === 'derived') return b.rendererName;
    if (b.kind === 'ambient') return b.ambientName;
    return 'Graphite';
  });

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
      <GrPane title={paneTitle} variant="focus">
        <div class="gr-placeholder">
          <div class="ph-title">{paneTitle}</div>
          <div class="ph-sub">Daily-driver views land in the next phase.</div>
        </div>
      </GrPane>
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

<script lang="ts">
  /*
   * Peek popover — Telescope-shaped quick lookup.
   *
   * Rewritten for v5: resolves the target page via the v5 buffer state's
   * `lastFocusedPageId` rather than the v4 pane tree, and hosts derived
   * renderers through the v5 registry so the same renderer code path
   * runs here as in a derived buffer pane.
   *
   * `Tab` / `Shift-Tab` cycles renderer; Esc dismisses; Enter is
   * delegated to the inner renderer.
   */
  import { onMount } from "svelte";
  import BacklinksTab from "$lib/components/v4/BacklinksTab.svelte";
  import OutlineTab from "$lib/components/v4/OutlineTab.svelte";
  import PropertiesView from "$lib/components/v4/PropertiesView.svelte";
  import LinkedTasksTab from "$lib/components/LinkedTasksTab.svelte";
  import {
    closePeek,
    getPeekKind,
    isPeekOpen,
    setPeekKind,
    type PeekKind,
  } from "$lib/stores/peek.svelte";
  import {
    getLastFocusedPageId,
    openPageInFocused,
  } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import {
    getJourneyEntries,
    jumpToJourneyEntry,
  } from "$lib/stores/journey.svelte";

  const open = $derived(isPeekOpen());
  const kind = $derived(getPeekKind());

  // Resolve the target page from v5 buffer state.
  const pageId = $derived.by(() => {
    if (!open) return undefined;
    const pid = getLastFocusedPageId();
    return pid || undefined;
  });

  const entries = $derived(getJourneyEntries());

  // Cycle order — `Tab` walks forward; `Shift-Tab` walks back.
  const CYCLE: PeekKind[] = [
    "backlinks",
    "outline",
    "properties",
    "journey",
  ];

  function cycle(dir: 1 | -1) {
    const i = CYCLE.indexOf(kind);
    const next = (i + dir + CYCLE.length) % CYCLE.length;
    setPeekKind(CYCLE[next]);
  }

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closePeek();
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      cycle(e.shiftKey ? -1 : 1);
      return;
    }
  }

  function pickEntry(idx: number) {
    const t = jumpToJourneyEntry(idx);
    if (t) {
      openPageInFocused(asPageId(t));
      closePeek();
    }
  }

  function openHere(id: string) {
    openPageInFocused(asPageId(id));
    closePeek();
  }

  onMount(() => {
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="peek-backdrop"
    onclick={(e) => {
      if (e.target === e.currentTarget) closePeek();
    }}
  >
    <div class="peek">
      <header class="peek-head">
        <div class="peek-meta">
          <span class="peek-label">peek</span>
          {#if pageId}
            <span class="peek-tile">{pageId}</span>
          {/if}
        </div>
        <div class="peek-controls">
          <select
            class="peek-kind"
            value={kind}
            onchange={(e) =>
              setPeekKind(e.currentTarget.value as PeekKind)}
          >
            {#each CYCLE as k (k)}
              <option value={k}>{k}</option>
            {/each}
          </select>
          <button
            class="peek-close"
            type="button"
            onclick={closePeek}
            title="close · Esc"
          >×</button>
        </div>
      </header>
      <div class="peek-hint">
        Tab / Shift-Tab to cycle renderer
      </div>
      <div class="peek-body">
        {#if !pageId && kind !== "journey"}
          <p class="peek-empty">no focused page to peek at</p>
        {:else if kind === "backlinks"}
          <BacklinksTab noteId={pageId} onOpenNote={openHere} />
        {:else if kind === "outline"}
          <OutlineTab noteId={pageId} onOpenNote={openHere} />
        {:else if kind === "properties"}
          <PropertiesView noteId={pageId} focusedBlock={null} />
        {:else if kind === "journey"}
          {#if entries.length === 0}
            <p class="peek-empty">no journey entries yet</p>
          {:else}
            <ul class="peek-journey">
              {#each entries as e, i (e.ts)}
                <li>
                  <button
                    type="button"
                    class="peek-journey-row"
                    onclick={() => pickEntry(i)}
                  >
                    <span class="peek-journey-tile">{e.tileId}</span>
                    <span class="peek-journey-via">{e.via}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .peek-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: color-mix(in srgb, var(--v4-bg) 30%, transparent);
    display: flex;
    align-items: flex-start;
    justify-content: flex-end;
    padding: 80px 40px 20px;
    animation: v4-fade-in var(--v4-dur-fast) var(--v4-ease-overlay);
  }
  .peek {
    width: min(420px, calc(100vw - 80px));
    max-height: calc(100vh - 120px);
    background: var(--v4-bg);
    border: 1px solid var(--v4-hair);
    border-radius: 10px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: v4-popover-in var(--v4-dur-base) var(--v4-ease-overlay);
  }
  .peek-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--v4-hair);
    flex-shrink: 0;
  }
  .peek-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .peek-label {
    font-family: var(--v4-mono);
    font-size: 9.5px;
    letter-spacing: 1.4px;
    text-transform: uppercase;
    color: var(--v4-ink5);
  }
  .peek-tile {
    font-family: var(--v4-mono);
    font-size: 11px;
    color: var(--v4-ink2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .peek-controls {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .peek-kind {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink2);
    font-family: var(--v4-mono);
    font-size: 11px;
    border-radius: 5px;
    padding: 2px 6px;
  }
  .peek-close {
    background: transparent;
    border: 0;
    color: var(--v4-ink5);
    font-size: 14px;
    line-height: 1;
    padding: 2px 6px;
    cursor: pointer;
    border-radius: 4px;
  }
  .peek-close:hover {
    color: var(--v4-ink2);
    background: var(--v4-surface-lo);
  }
  .peek-hint {
    padding: 4px 12px;
    color: var(--v4-ink6);
    font-family: var(--v4-mono);
    font-size: 10px;
    border-bottom: 1px solid var(--v4-hair);
  }
  .peek-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
  }
  .peek-empty {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 11.5px;
    padding: 16px 4px;
    text-align: center;
  }
  .peek-journey {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .peek-journey-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 5px;
    padding: 5px 8px;
    cursor: pointer;
  }
  .peek-journey-row:hover {
    background: var(--v4-surface-lo);
  }
  .peek-journey-tile {
    font-family: var(--v4-mono);
    font-size: 11.5px;
    color: var(--v4-ink2);
  }
  .peek-journey-via {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink5);
  }
</style>

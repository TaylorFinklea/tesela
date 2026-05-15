<script lang="ts">
  /*
   * Prism v4 — `i` peek popover.
   *
   * Floating card anchored to the focused pane. Re-uses the existing
   * BacklinksTab / OutlineTab / PropertiesView components for the
   * data-driven kinds, plus a Journey list for the "where have I
   * been" kind. The graph and timeline kinds show placeholders for
   * now — full inline rendering lands as polish.
   */
  import { onMount } from "svelte";
  import BacklinksTab from "$lib/components/v4/BacklinksTab.svelte";
  import OutlineTab from "$lib/components/v4/OutlineTab.svelte";
  import PropertiesView from "$lib/components/v4/PropertiesView.svelte";
  import {
    closePeek,
    getPeekAnchorPaneId,
    getPeekKind,
    getPeekKinds,
    isPeekOpen,
    setPeekKind,
    type PeekKind,
  } from "$lib/stores/peek.svelte";
  import {
    getFocusedPane,
    getPaneById,
    jumpToTile,
  } from "$lib/stores/pane-tree.svelte";
  import {
    getJourneyEntries,
    jumpToJourneyEntry,
  } from "$lib/stores/journey.svelte";

  const open = $derived(isPeekOpen());
  const kind = $derived(getPeekKind());

  /** Tile id the popover is currently looking at — anchor pane if set,
   *  else the live focused pane. */
  const tileId = $derived.by(() => {
    if (!open) return undefined;
    const anchor = getPeekAnchorPaneId();
    if (anchor) {
      const hit = getPaneById(anchor);
      if (hit?.pane.kind === "editor") return hit.pane.tiles[hit.pane.activeIdx];
    }
    const p = getFocusedPane();
    if (p?.kind === "editor") return p.tiles[p.activeIdx];
    return undefined;
  });

  const entries = $derived(getJourneyEntries());

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closePeek();
    }
  }

  function pickEntry(idx: number) {
    const t = jumpToJourneyEntry(idx);
    if (t) jumpToTile(t, "peek-journey");
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
          {#if tileId}
            <span class="peek-tile">{tileId}</span>
          {/if}
        </div>
        <div class="peek-controls">
          <select
            class="peek-kind"
            value={kind}
            onchange={(e) => setPeekKind(e.currentTarget.value as PeekKind)}
          >
            {#each getPeekKinds() as k (k)}
              <option value={k}>{k}</option>
            {/each}
          </select>
          <button class="peek-close" type="button" onclick={closePeek} title="close · Esc · K">×</button>
        </div>
      </header>
      <div class="peek-body">
        {#if !tileId}
          <p class="peek-empty">no focused tile to peek at</p>
        {:else if kind === "backlinks"}
          <BacklinksTab
            noteId={tileId}
            onOpenNote={(id) => {
              jumpToTile(id, "peek");
              closePeek();
            }}
          />
        {:else if kind === "outline"}
          <OutlineTab
            noteId={tileId}
            onOpenNote={(id) => {
              jumpToTile(id, "peek");
              closePeek();
            }}
          />
        {:else if kind === "properties"}
          <PropertiesView noteId={tileId} focusedBlock={null} />
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
                    onclick={() => {
                      pickEntry(i);
                      closePeek();
                    }}
                  >
                    <span class="peek-journey-tile">{e.tileId}</span>
                    <span class="peek-journey-via">{e.via}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {:else if kind === "timeline" || kind === "graph"}
          <p class="peek-empty">
            {kind} · inline rendering lands as polish — for now, use the
            {kind === "graph" ? "fullscreen graph (`g`)" : "Recent widget"} from the dashboard.
          </p>
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
  .peek-meta { display: flex; align-items: center; gap: 8px; min-width: 0; }
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
  .peek-controls { display: flex; align-items: center; gap: 4px; }
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
  .peek-close:hover { color: var(--v4-ink2); background: var(--v4-surface-lo); }
  .peek-body { flex: 1; min-height: 0; overflow: auto; padding: 10px 12px; }

  .peek-empty {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 11.5px;
    padding: 16px 4px;
    text-align: center;
  }
  .peek-journey { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
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
  .peek-journey-row:hover { background: var(--v4-surface-lo); }
  .peek-journey-tile { font-family: var(--v4-mono); font-size: 11.5px; color: var(--v4-ink2); }
  .peek-journey-via { font-family: var(--v4-mono); font-size: 10px; color: var(--v4-ink5); }
</style>

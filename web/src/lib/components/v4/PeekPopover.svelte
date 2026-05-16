<script lang="ts">
  /*
   * Peek popover — Telescope-shaped quick lookup.
   *
   * Hosts derived renderers via the v5 registry — the SAME Svelte
   * component instances that mount inside a derived-buffer pane. The
   * goal is host-agnosticism: a renderer doesn't know whether it's in
   * Peek or a pane. Different hosts wrap the renderer in different
   * chrome; that's the only place behavior diverges.
   *
   * `Tab` / `Shift-Tab` cycles renderer; Esc dismisses; Enter delegates
   * to the inner renderer (most rows already accept Enter). Peek's
   * `onNavigate` closes the popover before navigating; a pane's
   * `onNavigate` just navigates. Renderers themselves are identical.
   *
   * Workspace state owns: per-page-type first-shown renderer (so daily
   * pages can open Peek on outline by default, regularly notes can open
   * on backlinks, etc.) and a hide-list of renderers to skip in the
   * cycle.
   */
  import { onMount } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import {
    closePeek,
    cyclePeek,
    getPeekKind,
    isPeekOpen,
    setPeekKind,
    DEFAULT_PEEK_CYCLE,
    type PeekKind,
  } from "$lib/stores/peek.svelte";
  import {
    getLastFocusedPageId,
    getPeekFirstRendererFor,
    getPeekHideList,
    openPageInFocused,
    setPeekFirstRendererFor,
  } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import {
    getJourneyEntries,
    jumpToJourneyEntry,
  } from "$lib/stores/journey.svelte";
  import "$lib/renderers/register"; // side-effect: ensure registries are populated
  import { mount as mountDerived } from "$lib/renderers/derived";
  import {
    pickCascadeMember,
    type NavigationIntent,
  } from "$lib/buffer/protocol";

  const open = $derived(isPeekOpen());
  const kind = $derived(getPeekKind());
  const hideList = $derived(new Set(getPeekHideList()));

  // Resolve the target page from v5 buffer state.
  const pageId = $derived.by(() => {
    if (!open) return undefined;
    const pid = getLastFocusedPageId();
    return pid || undefined;
  });

  const entries = $derived(getJourneyEntries());

  // Fetch the focused note to read its type for the per-page-type
  // first-shown lookup. Cheap query; cache hit when the page is already
  // open in a buffer.
  const noteQuery = createQuery(() => ({
    queryKey: ["note", pageId] as const,
    queryFn: () => api.getNote(pageId as string),
    enabled: !!pageId,
  }));
  const pageType = $derived.by(() => {
    const n = noteQuery.data as Note | undefined;
    return n?.metadata.note_type ?? "note";
  });

  // When Peek opens (or the focused page changes while open), consult
  // the workspace's per-page-type preferred first renderer. Only fires
  // on the OPEN transition — once open, the user's Tab choices stick.
  let lastOpenedFor: string | undefined = undefined;
  $effect(() => {
    if (!open) {
      lastOpenedFor = undefined;
      return;
    }
    if (!pageId) return;
    if (lastOpenedFor === pageId) return;
    lastOpenedFor = pageId;
    const pref = getPeekFirstRendererFor(pageType);
    if (pref) setPeekKind(pref);
  });

  // When the user picks a different renderer from the dropdown while
  // peeking a page of a given type, remember it as the new first-shown
  // for that page type. Tab cycling does NOT update the preference (it's
  // exploration, not commitment).
  function onKindChange(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value as PeekKind;
    setPeekKind(v);
    if (pageType) setPeekFirstRendererFor(pageType, v);
  }

  // Peek-constrained size — small enough that cascade-aware renderers
  // pick a compact mode when Phase 10 lands.
  const PEEK_SIZE = { cols: 50, rows: 18 };

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
      cyclePeek(e.shiftKey ? -1 : 1, hideList);
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

  /** Peek's onNavigate: close-then-navigate. The renderer doesn't know
   *  this is Peek; the host translates intent into "dismiss popover +
   *  open page in the user's last editor". */
  function handleIntent(i: NavigationIntent) {
    closePeek();
    if (i.kind === "open-page") {
      openPageInFocused(asPageId(i.path));
    }
    // tag/query intents not yet wired
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
            onchange={onKindChange}
          >
            {#each DEFAULT_PEEK_CYCLE.filter((k) => !hideList.has(k)) as k (k)}
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
        Tab / Shift-Tab to cycle · Esc to close
      </div>
      <div class="peek-body">
        {#if !pageId && kind !== "journey"}
          <p class="peek-empty">no focused page to peek at</p>
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
        {:else}
          <!-- Mount the derived renderer via the registry. Same component
               that a derived-buffer pane mounts; only the host chrome and
               onNavigate semantics differ. -->
          {@const ref = { kind: "page" as const, path: pageId as string }}
          {#await Promise.resolve(mountDerived(kind, ref)) then renderer}
            {@const C = pickCascadeMember(renderer.cascade, PEEK_SIZE)}
            <C
              reference={ref}
              size={PEEK_SIZE}
              onNavigate={handleIntent}
            />
          {:catch err}
            <p class="peek-empty">
              renderer error: {err instanceof Error ? err.message : String(err)}
            </p>
          {/await}
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
    width: min(460px, calc(100vw - 80px));
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

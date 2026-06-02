<script lang="ts">
  /*
   * Prism v5 — single buffer leaf shell. Hosts one of the three buffer kinds
   * (page / derived / ambient) and routes mount duties accordingly:
   *
   *   page    → fetch note + render via the existing NoteRenderer (which
   *             dispatches on note.metadata.note_type — block outliner /
   *             journal / query widget / etc.). The page-type registry
   *             from Phase 1 will replace this dispatch in Phase 4 when
   *             derived renderers come in.
   *   derived → placeholder "coming in Phase 4" card
   *   ambient → placeholder "coming in Phase 5" card
   *
   * Every leaf wraps its renderer in `<svelte:boundary>` so a crashing
   * renderer fails soft without taking the pane / tab with it.
   */
  import { onDestroy } from "svelte";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type {
    Buffer,
    LeafId,
    PageId,
    Reference,
  } from "$lib/buffer/types";
  import {
    closeFocusedLeaf,
    focusLeaf,
    getLastFocusedPageId,
    openPageInFocused,
  } from "$lib/buffer/state.svelte";
  import {
    setFocusedBlockForPane,
    clearPaneFocusedBlock,
  } from "$lib/stores/current-block.svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";
  import NoteRenderer from "$lib/components/v4/NoteRenderer.svelte";
  import { openLeader } from "$lib/v5/leader-tree.svelte";
  import "$lib/renderers/register"; // side-effect: register all v5 renderers
  import { mount as mountDerived } from "$lib/renderers/derived";
  import { get as getAmbient } from "$lib/renderers/ambient";
  import { pickCascadeMember, type NavigationIntent } from "$lib/buffer/protocol";

  let {
    leafId,
    buffer,
    focused,
  }: {
    leafId: LeafId;
    buffer: Buffer;
    focused: boolean;
  } = $props();

  let shellEl = $state<HTMLElement | undefined>();

  // When this leaf becomes focused AND it's a page buffer, push focus
  // into the inner cm-editor so the user can immediately start typing.
  // For non-page buffers (derived, ambient), focus stays on the shell —
  // there's nothing to type into.
  //
  // We poll briefly because freshly-created notes (e.g. `:scratch`) take
  // a few ticks to: fetch the note, mount NoteRenderer, mount BlockOutliner,
  // and mount the first BlockEditor's cm-editor. A single `setTimeout(0)`
  // races with that. The poll gives up after ~500ms.
  $effect(() => {
    if (!focused || !shellEl) return;
    if (buffer.kind !== "page") return;
    // Track buffer.pageId so the effect re-runs whenever the focused
    // buffer's page changes (incl. when `:scratch` creates a new page
    // and points the focused leaf at it).
    const _trigger = (buffer as { pageId?: string }).pageId;
    let cancelled = false;
    let elapsed = 0;
    const tick = () => {
      if (cancelled || !shellEl) return;
      if (shellEl.contains(document.activeElement)) {
        if (document.activeElement?.classList.contains("cm-content")) return;
      }
      const cmContent = shellEl.querySelector(".cm-content");
      if (cmContent instanceof HTMLElement) {
        cmContent.focus();
        return;
      }
      elapsed += 50;
      if (elapsed > 500) return;
      setTimeout(tick, 50);
    };
    const start = setTimeout(tick, 0);
    return () => {
      cancelled = true;
      clearTimeout(start);
    };
  });

  // ── page buffer: TanStack Query for the note ──────────────────────────
  const activePageId = $derived(
    buffer.kind === "page" ? buffer.pageId : undefined,
  );
  const queryClient = useQueryClient();
  const noteQuery = createQuery(() => ({
    queryKey: ["note", activePageId] as const,
    queryFn: () => api.getNote(activePageId as string),
    enabled: !!activePageId,
  }));
  const note = $derived(noteQuery.data as Note | undefined);

  // ── debounced save (mirrors v4) ────────────────────────────────────────
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: AbortController | null = null;
  let pending: string | null = null;
  // Edit BASE for the pending save (body the outliner last reseeded from).
  // Sent as `base_content` so the server diffs the author's real changes and
  // never re-asserts an untouched block over a concurrent peer edit. First
  // base of the window wins (base doesn't shift mid-burst); cleared on flush.
  let pendingBase: string | undefined = undefined;

  function handleContentChange(fullContent: string, baseContent?: string) {
    pending = fullContent;
    if (pendingBase === undefined) pendingBase = baseContent;
    if (saveTimer) clearTimeout(saveTimer);
    setSaving();
    saveTimer = setTimeout(() => void flushSave(), 500);
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (pending === null || !activePageId) return;
    const content = pending;
    pending = null;
    const base = pendingBase;
    pendingBase = undefined;
    if (inFlight) inFlight.abort();
    const controller = new AbortController();
    inFlight = controller;
    const id = activePageId;
    if (note) queryClient.setQueryData(["note", id], { ...note, content });
    try {
      const updated = await api.updateNote(id, content, base, controller.signal);
      if (controller.signal.aborted) return;
      queryClient.setQueryData(["note", id], updated);
      setSaved();
    } catch (e) {
      if ((e as { name?: string })?.name === "AbortError") return;
      setSaveError(e instanceof Error ? e.message : "Unknown error");
      console.error("v5 buffer save failed:", e);
    } finally {
      if (inFlight === controller) inFlight = null;
    }
  }

  async function cancelAndFlush(fullContent: string, baseContent?: string) {
    pending = fullContent;
    if (baseContent !== undefined) pendingBase = baseContent;
    await flushSave();
  }

  onDestroy(() => clearPaneFocusedBlock(leafId as unknown as string));

  // Clicking inside the shell focuses this leaf (unless the click hit an
  // interactive control or an inner editor).
  function onShellClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (
      target.closest(
        "button, a, input, textarea, select, [role='button'], [role='option']",
      )
    ) {
      return;
    }
    focusLeaf(leafId);
    if (!target.closest(".cm-editor")) shellEl?.focus();
  }

  // Opening a note from inside a page buffer (e.g. wiki link, query row):
  // for now, replace this pane's page. Phase 4 routes derived renderers
  // through the explicit intent sink below.
  function openNoteHere(targetPageId: string) {
    openPageInFocused(targetPageId as PageId);
  }

  // Derived + ambient renderers emit NavigationIntents through this sink.
  function handleIntent(i: NavigationIntent) {
    if (i.kind === "open-page") {
      // For Phase 4, "replace" simply swaps the focused pane's content.
      // "split-right", "split-down", "new-tab" are deferred to Phase 9
      // when Peek navigation lands; default everything to replace.
      openPageInFocused(i.path as PageId);
    } else if (i.kind === "open-tag") {
      // Reference: tag resolution. The tag's page lives at NoteId
      // `<slug>`; opening it routes through the same page-open path as
      // any other note. Phase 2 moves the file to `tags/<slug>.md`; the
      // NoteId stays a bare slug until then.
      openPageInFocused(i.value.toLowerCase() as PageId);
    }
    // query navigation intents remain deferred.
  }

  // Resolve a derived buffer's binding into a concrete Reference.
  // Follow → reads the active tab's lastFocusedPageId; Pinned → the
  // explicit reference. Returns undefined when Follow has no source yet.
  function resolveDerivedReference(buf: Buffer & { kind: "derived" }): Reference | undefined {
    if (buf.binding.mode === "pinned") return buf.binding.reference;
    const pid = getLastFocusedPageId();
    if (!pid) return undefined;
    return { kind: "page", path: pid };
  }

  // ── size measurement for cascade ─────────────────────────────────────
  //
  // Phase 10: BufferShell measures its content area via ResizeObserver,
  // converts px → cell-units (cols, rows) using a fixed monospace cell
  // (CHAR_WIDTH × LINE_HEIGHT), and feeds that to renderer cascades.
  // Renderers don't self-decide which mode to use — the host picks the
  // most-featured cascade member that fits this size.
  //
  // Cell units rather than raw pixels because the spec specifies them and
  // renderers read more naturally as "needs ≥80 cols" than "≥640px".
  const CHAR_WIDTH = 7;   // approx monospace char @ 12px
  const LINE_HEIGHT = 20; // approx body line height
  let bodyEl = $state<HTMLElement | undefined>();
  let measuredSize = $state({ cols: 80, rows: 24 });

  $effect(() => {
    if (!bodyEl) return;
    const ro = new ResizeObserver((entries) => {
      const e = entries[0];
      if (!e) return;
      const w = e.contentRect.width;
      const h = e.contentRect.height;
      measuredSize = {
        cols: Math.max(1, Math.floor(w / CHAR_WIDTH)),
        rows: Math.max(1, Math.floor(h / LINE_HEIGHT)),
      };
    });
    ro.observe(bodyEl);
    return () => ro.disconnect();
  });

  function referenceLabel(ref: Reference): string {
    if (ref.kind === "page") return ref.path || "(empty)";
    if (ref.kind === "tag") return `#${ref.value}`;
    return ref.dsl;
  }

  function truncate(s: string, n: number): string {
    return s.length > n ? s.slice(0, n - 1) + "…" : s;
  }

  const KIND_LABEL: Record<Buffer["kind"], string> = {
    page: "page",
    derived: "derived",
    ambient: "ambient",
  };
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<section
  bind:this={shellEl}
  class="v5-buffer"
  class:focused
  data-leaf-id={leafId}
  tabindex={0}
  onclick={onShellClick}
>
  <header class="v5-buffer-header">
    <div class="v5-buffer-header-left">
      <span class="v5-focus-dot">{focused ? "●" : "○"}</span>
      {#if buffer.kind === "page"}
        <span class="v5-buffer-title">{activePageId || "empty"}</span>
        {#if note?.metadata.note_type === "scratch"}
          <span class="v5-scratch-chip" title="scratch · :promote to keep"
            >scratch</span
          >
        {:else if (note?.metadata.note_type ?? "").toLowerCase() === "tag"}
          <span class="v5-tag-chip" title="tag page">tag</span>
        {/if}
      {:else if buffer.kind === "derived"}
        <span class="v5-buffer-title">{buffer.rendererName}</span>
        <span class="v5-buffer-sub">
          {#if buffer.binding.mode === "follow"}
            {@const ref = resolveDerivedReference(buffer)}
            {#if ref}
              · following {truncate(referenceLabel(ref), 30)}
            {:else}
              · (nothing focused)
            {/if}
          {:else}
            · pinned · {truncate(referenceLabel(buffer.binding.reference), 30)}
          {/if}
        </span>
      {:else}
        <span class="v5-buffer-title">{buffer.ambientName}</span>
      {/if}
      <span class="v5-kind-chip">{KIND_LABEL[buffer.kind]}</span>
    </div>
    <div class="v5-buffer-header-right">
      <button
        type="button"
        class="v5-buffer-close"
        title="close pane · ⌘W"
        onclick={(e) => {
          e.stopPropagation();
          focusLeaf(leafId);
          closeFocusedLeaf();
        }}>×</button
      >
    </div>
  </header>

  <div class="v5-buffer-body" bind:this={bodyEl}>
    <svelte:boundary>
      {#if buffer.kind === "page"}
        {#if !activePageId}
          <div class="v5-buffer-empty">
            <p>empty pane</p>
            <p class="v5-buffer-empty-hint">jump to a tile to fill it</p>
          </div>
        {:else if noteQuery.isLoading}
          <div class="v5-buffer-empty"><p>loading…</p></div>
        {:else if noteQuery.isError}
          <div class="v5-buffer-empty">
            <p>could not load {activePageId}</p>
          </div>
        {:else if note}
          {#key activePageId}
            <div class="v5-buffer-scroll">
              <NoteRenderer
                {note}
                paneId={leafId as unknown as string}
                size={measuredSize}
                onContentChange={handleContentChange}
                onCancelAndFlush={cancelAndFlush}
                onfocusedblockchange={(b) =>
                  setFocusedBlockForPane(leafId as unknown as string, b)}
                onOpenNote={openNoteHere}
                onLeader={() => openLeader()}
              />
            </div>
          {/key}
        {/if}
      {:else if buffer.kind === "derived"}
        {@const ref = resolveDerivedReference(buffer)}
        {#if !ref}
          <div class="v5-buffer-empty">
            <p>nothing focused yet</p>
            <p class="v5-buffer-empty-hint">
              focus a page buffer to see {buffer.rendererName}
            </p>
          </div>
        {:else}
          {@const r = mountDerived(buffer.rendererName, ref)}
          {@const C = pickCascadeMember(r.cascade, measuredSize)}
          <div class="v5-buffer-scroll">
            <C
              reference={ref}
              size={measuredSize}
              onNavigate={handleIntent}
            />
          </div>
        {/if}
      {:else if buffer.kind === "ambient"}
        {@const r = getAmbient(buffer.ambientName)}
        {#if !r}
          <div class="v5-buffer-empty">
            <p>ambient "{buffer.ambientName}" is not registered</p>
          </div>
        {:else}
          {@const C = pickCascadeMember(r.cascade, measuredSize)}
          <C size={measuredSize} onNavigate={handleIntent} />
        {/if}
      {/if}
      {#snippet failed(error, reset)}
        <div class="v5-buffer-empty">
          <p>renderer crashed</p>
          <p class="v5-buffer-empty-hint">
            {error instanceof Error ? error.message : String(error)}
          </p>
          <button type="button" onclick={reset}>reload</button>
        </div>
      {/snippet}
    </svelte:boundary>
  </div>
</section>

<style>
  .v5-buffer {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--v4-bg);
    border-top: 2px solid transparent;
    transition:
      border-color var(--v4-dur-base, 220ms) var(--v4-ease-settle, ease-out),
      background var(--v4-dur-base, 220ms) var(--v4-ease-settle, ease-out);
    outline: none;
  }
  .v5-buffer.focused {
    /* The focused buffer lifts to the elevated surface so it reads as
     * active against the deeper `--v4-bg` canvas; the accent top border
     * is the focus signal. (A flat surface, not the old gradient
     * overlay — the `background` transition on `.v5-buffer` animates it.) */
    background: var(--v4-surface-lo);
    border-top-color: var(--v4-accent);
  }
  .v5-buffer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--v4-hair);
    min-height: 30px;
    flex-shrink: 0;
  }
  .v5-buffer-header-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .v5-buffer-header-right {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .v5-focus-dot {
    font-family: var(--v4-mono);
    font-size: 9px;
    color: var(--v4-ink5);
    flex-shrink: 0;
  }
  .v5-buffer.focused .v5-focus-dot {
    color: var(--v4-accent);
  }
  .v5-buffer-title {
    font-family: var(--v4-sans);
    font-size: 12px;
    color: var(--v4-ink2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v5-buffer-sub {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink5);
  }
  .v5-kind-chip {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink5);
    border: 1px solid var(--v4-hair);
    border-radius: 5px;
    padding: 0 6px;
    line-height: 16px;
  }
  .v5-scratch-chip {
    font-family: var(--v4-mono);
    font-size: 9.5px;
    color: var(--v4-accent);
    border: 1px solid var(--v4-accent-dim);
    border-radius: 4px;
    padding: 0 6px;
    line-height: 16px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .v5-tag-chip {
    font-family: var(--v4-mono);
    font-size: 9.5px;
    color: var(--v4-ink2);
    border: 1px solid var(--v4-hair2);
    border-radius: 4px;
    padding: 0 6px;
    line-height: 16px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .v5-buffer-close {
    background: transparent;
    border: 0;
    color: var(--v4-ink5);
    font-size: 14px;
    line-height: 1;
    padding: 2px 6px;
    cursor: pointer;
    border-radius: 4px;
  }
  .v5-buffer-close:hover {
    color: var(--v4-ink2);
    background: var(--v4-surface-lo);
  }
  .v5-buffer-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .v5-buffer-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 14px 18px;
  }
  .v5-buffer-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 12px;
  }
  .v5-buffer-empty-hint {
    color: var(--v4-ink6);
    font-size: 10.5px;
  }
</style>

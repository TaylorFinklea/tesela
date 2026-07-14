<script lang="ts">
  import { onDestroy } from "svelte";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import {
    saveAdmissionRegistry,
    type SaveAdmissionLease,
  } from "$lib/block-ops-saver";
  import BlockOutliner from "./BlockOutliner.svelte";
  import type { PinnedTab } from "$lib/stores/pane-state.svelte";
  import { setFocusedBlock } from "$lib/stores/current-block.svelte";

  let { pin, onunpin }: { pin: PinnedTab | undefined; onunpin: () => void } = $props();

  const queryClient = useQueryClient();

  type SaveState = {
    timer: number | null;
    pending: string | null;
    base: string | undefined;
    inFlightPromise: Promise<void> | null;
    failed: boolean;
    failure: unknown;
    admission: SaveAdmissionLease | null;
  };
  const saveStates = new Map<string, SaveState>();

  function getSaveState(noteId: string): SaveState {
    let state = saveStates.get(noteId);
    if (!state) {
      const next: SaveState = {
        timer: null,
        pending: null,
        base: undefined,
        inFlightPromise: null,
        failed: false,
        failure: undefined,
        admission: null,
      };
      saveStates.set(noteId, next);
      state = next;
    }
    return state;
  }

  function ensureSaveAdmission(noteId: string, state: SaveState): void {
    if (state.admission) return;
    state.admission = saveAdmissionRegistry.admit(
      noteId,
      () => settleSave(noteId),
    );
  }

  function releaseSaveAdmissionIfQuiet(state: SaveState): void {
    if (
      state.failed
      || state.timer !== null
      || state.pending !== null
      || state.inFlightPromise !== null
    ) return;
    const admission = state.admission;
    state.admission = null;
    admission?.release();
  }

  function handleContentChange(fullContent: string, baseContent?: string) {
    if (!pin) return;
    const noteId = pin.noteId;
    const state = getSaveState(noteId);
    ensureSaveAdmission(noteId, state);
    state.pending = fullContent;
    if (state.base === undefined) state.base = baseContent;
    if (state.timer) clearTimeout(state.timer);
    state.timer = window.setTimeout(() => {
      void flushSave(noteId).catch(() => {});
    }, 400);
  }

  function flushSave(noteId: string): Promise<void> {
    const state = getSaveState(noteId);
    if (state.timer) {
      clearTimeout(state.timer);
      state.timer = null;
    }
    if (state.pending === null) return state.inFlightPromise ?? Promise.resolve();
    if (state.inFlightPromise) {
      const predecessor = state.inFlightPromise;
      return predecessor.then(
        () => flushSave(noteId),
        async (error) => {
          await flushSave(noteId);
          throw error;
        },
      );
    }
    const content = state.pending;
    state.pending = null;
    const base = state.base;
    state.base = undefined;
    const controller = new AbortController();
    const completion = (async () => {
      try {
        // `base` (the body the outliner last reseeded from) is sent as
        // `base_content` so the server diffs the author's real changes and a
        // concurrent peer edit to an untouched block survives.
        const updated = await api.updateNote(noteId, content, base, controller.signal);
        if (controller.signal.aborted) return;
        queryClient.setQueryData(["note", noteId], updated);
      } catch (e) {
        if ((e as Error).name === "AbortError") return;
        if (!state.failed) {
          state.failed = true;
          state.failure = e;
        }
        console.error("Save failed", e);
        throw e;
      } finally {
        state.inFlightPromise = null;
        releaseSaveAdmissionIfQuiet(state);
      }
    })();
    state.inFlightPromise = completion;
    void completion.catch(() => {});
    return completion;
  }

  async function settleSave(noteId: string): Promise<void> {
    const state = getSaveState(noteId);
    let failed = false;
    let firstFailure: unknown;
    try {
      while (true) {
        if (state.inFlightPromise) {
          try {
            await state.inFlightPromise;
          } catch (error) {
            if (!failed) firstFailure = error;
            failed = true;
          }
          continue;
        }
        if (state.pending === null) {
          if (state.failed) throw state.failure;
          if (failed) throw firstFailure;
          return;
        }
        try {
          await flushSave(noteId);
        } catch (error) {
          if (!failed) firstFailure = error;
          failed = true;
        }
      }
    } finally {
      releaseSaveAdmissionIfQuiet(state);
    }
  }

  function handleCancelAndFlush(fullContent: string, baseContent?: string): Promise<void> {
    if (!pin) return Promise.resolve();
    const noteId = pin.noteId;
    const state = getSaveState(noteId);
    ensureSaveAdmission(noteId, state);
    state.pending = fullContent;
    if (baseContent !== undefined) state.base = baseContent;
    const completion = settleSave(noteId);
    void completion.catch(() => {});
    return completion;
  }

  onDestroy(() => {
    for (const [noteId, state] of saveStates) {
      if (state.timer) {
        clearTimeout(state.timer);
        state.timer = null;
      }
      const completion = settleSave(noteId);
      void completion.catch((error) => console.error("Pinned tab teardown save failed", error));
    }
  });

  function splitFm(content: string): string {
    if (!content.startsWith("---")) return "";
    const end = content.indexOf("---", 3);
    if (end === -1) return "";
    return content.slice(0, end + 3) + "\n";
  }
  function splitBody(content: string): string {
    if (!content.startsWith("---")) return content;
    const end = content.indexOf("---", 3);
    if (end === -1) return content;
    const after = content.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  }

  const noteQuery = $derived(pin ? createQuery(() => ({
    queryKey: ["note", pin!.noteId] as const,
    queryFn: () => api.getNote(pin!.noteId),
    enabled: true,
  })) : null);
</script>

{#if !pin}
  <div class="v9-pin-empty">This pinned tab is no longer valid. <button onclick={onunpin}>Unpin</button></div>
{:else if noteQuery?.isLoading}
  <div class="v9-pin-loading">Loading…</div>
{:else if noteQuery && !noteQuery.data}
  <div class="v9-pin-empty">
    Note no longer exists. <button onclick={onunpin}>Unpin</button>
  </div>
{:else if noteQuery?.data}
  {@const note = noteQuery.data}
  {#if pin.kind === 'page'}
    <BlockOutliner
      noteId={note.id}
      body={splitBody(note.content)}
      frontmatter={splitFm(note.content)}
      isPinnedTab={true}
      onContentChange={handleContentChange}
      onCancelAndFlush={handleCancelAndFlush}
      onPrepareRelocation={() => settleSave(note.id)}
      onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
      onfocusedblockchange={(b) => setFocusedBlock(b)}
    />
  {:else}
    <BlockOutliner
      noteId={note.id}
      body={splitBody(note.content)}
      frontmatter={splitFm(note.content)}
      drillBlockId={pin.blockId}
      isPinnedTab={true}
      onContentChange={handleContentChange}
      onCancelAndFlush={handleCancelAndFlush}
      onPrepareRelocation={() => settleSave(note.id)}
      onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
      onfocusedblockchange={(b) => setFocusedBlock(b)}
    />
  {/if}
{/if}

<style>
  .v9-pin-empty, .v9-pin-loading {
    padding: 12px;
    color: var(--v9-ink-faint);
    font-size: 12px;
  }
  .v9-pin-empty button {
    background: transparent;
    border: 1px solid var(--v9-line);
    color: var(--v9-ink-2);
    padding: 2px 6px;
    border-radius: 3px;
    cursor: pointer;
    margin-left: 4px;
  }
</style>

<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import BlockOutliner from "./BlockOutliner.svelte";
  import type { PinnedTab } from "$lib/stores/pane-state.svelte";
  import { setFocusedBlock } from "$lib/stores/current-block.svelte";

  let { pin, onunpin }: { pin: PinnedTab | undefined; onunpin: () => void } = $props();

  const queryClient = useQueryClient();

  let saveTimer: number | null = null;
  let inflight: AbortController | null = null;

  function handleContentChange(fullContent: string) {
    if (!pin) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = window.setTimeout(async () => {
      inflight?.abort();
      inflight = new AbortController();
      try {
        const updated = await api.updateNote(pin!.noteId, fullContent, inflight.signal);
        queryClient.setQueryData(["note", pin!.noteId], updated);
      } catch (e) {
        if ((e as Error).name !== "AbortError") console.error("Save failed", e);
      }
    }, 400);
  }

  function handleCancelAndFlush(fullContent: string) {
    if (!pin) return;
    if (saveTimer) clearTimeout(saveTimer);
    inflight?.abort();
    inflight = new AbortController();
    void api.updateNote(pin!.noteId, fullContent, inflight.signal).then((updated) => {
      queryClient.setQueryData(["note", pin!.noteId], updated);
    });
  }

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

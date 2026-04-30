<script lang="ts">
  /**
   * Bottom-drawer History tab (Phase 9.3). Lists historical versions for the
   * focused page. Click a row → opens HistoryDiff modal with side-by-side
   * preview + restore button.
   */
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { lineDiff, relativeTime } from "$lib/line-diff";
  import HistoryDiff from "./HistoryDiff.svelte";

  type Props = { noteId: string };
  let { noteId }: Props = $props();

  const versionsQuery = createQuery(() => ({
    queryKey: ["note-versions", noteId] as const,
    queryFn: () => api.listNoteVersions(noteId, 100),
    enabled: noteId !== "",
  }));
  const versions = $derived(versionsQuery.data ?? []);

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: noteId !== "",
  }));
  const note = $derived(noteQuery.data);

  let openVersionId = $state<bigint | null>(null);
  const openVersion = $derived(versions.find((v) => v.id === openVersionId) ?? null);

  const queryClient = useQueryClient();

  async function restore() {
    if (!openVersion) return;
    try {
      await api.updateNote(noteId, openVersion.content);
      queryClient.invalidateQueries({ queryKey: ["note", noteId] });
      queryClient.invalidateQueries({ queryKey: ["note-versions", noteId] });
      openVersionId = null;
    } catch (e) {
      console.error("Restore failed:", e);
    }
  }
</script>

{#if !noteId}
  <div class="empty">No note focused</div>
{:else if versionsQuery.isLoading}
  <div class="empty">Loading…</div>
{:else if versions.length === 0}
  <div class="empty">No history yet — edit this note to start tracking versions.</div>
{:else}
  {#each versions as v}
    {@const summary = lineDiff(v.prev_content ?? "", v.content)}
    <button class="version-row" type="button" onclick={() => (openVersionId = v.id)}>
      <span class="t">{relativeTime(v.created_at)}</span>
      <span class="diff">+{summary.added} −{summary.removed}</span>
      <span class="ver">v{v.version_number}</span>
    </button>
  {/each}
{/if}

{#if openVersion && note}
  <HistoryDiff
    prev={openVersion.content}
    next={note.content}
    title={note.title}
    versionTimestamp={openVersion.created_at}
    onclose={() => (openVersionId = null)}
    onrestore={restore}
  />
{/if}

<style>
  .empty {
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 11px;
    padding: 4px 0;
  }
  .version-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 12px;
    align-items: center;
    width: 100%;
    padding: 4px 0;
    background: transparent;
    border: none;
    border-bottom: 1px dashed var(--v9-line);
    color: var(--v9-ink);
    font-family: var(--v9-mono);
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
  }
  .version-row:hover { background: var(--v9-bg-3); }
  .version-row .t { color: var(--v9-ink-2); }
  .version-row .diff { color: var(--v9-ink-faint); font-size: 10.5px; }
  .version-row .ver { color: var(--v9-amber); font-size: 10.5px; }
</style>

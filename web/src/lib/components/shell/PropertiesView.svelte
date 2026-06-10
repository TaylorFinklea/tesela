<script lang="ts">
  /*
   * Prism v4 context-pane tab — properties of the followed note and its
   * focused block. Read-only for Phase 2b: the editable property panel
   * in the legacy BottomDrawer is a deeply-coupled inline editor (chord
   * keyboard nav, pickers, registry wiring) — porting it to an editable
   * v4 panel is a dedicated follow-up. The note's `key:: value` block
   * properties are still editable inline in the editor pane meanwhile.
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  let {
    noteId,
    focusedBlock,
  }: {
    noteId: string | undefined;
    focusedBlock: ParsedBlock | null;
  } = $props();

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId as string),
    enabled: !!noteId,
  }));

  // Page-level properties: note_type, tags, plus any custom frontmatter.
  const pageProps = $derived.by(() => {
    const note = noteQuery.data as Note | undefined;
    if (!note) return [];
    const m = note.metadata;
    const rows: { key: string; value: string }[] = [];
    if (m.note_type) rows.push({ key: "type", value: m.note_type });
    if (m.tags.length) rows.push({ key: "tags", value: m.tags.join(", ") });
    for (const [k, v] of Object.entries(m.custom ?? {})) {
      rows.push({ key: k, value: typeof v === "string" ? v : JSON.stringify(v) });
    }
    return rows;
  });

  const blockProps = $derived.by(() => {
    if (!focusedBlock) return [];
    return Object.entries(focusedBlock.properties).map(([key, value]) => ({
      key,
      value,
    }));
  });
</script>

{#if !noteId}
  <p class="v4-ctx-empty">no note focused</p>
{:else}
  <div class="v4-prop-section">
    <p class="v4-prop-heading">page</p>
    {#if pageProps.length === 0}
      <p class="v4-ctx-empty">no page properties</p>
    {:else}
      <ul class="v4-ctx-list">
        {#each pageProps as p (p.key)}
          <li class="v4-prop-row">
            <span class="v4-prop-key">{p.key}</span>
            <span class="v4-prop-val">{p.value}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <div class="v4-prop-section">
    <p class="v4-prop-heading">block</p>
    {#if !focusedBlock}
      <p class="v4-ctx-empty">no block focused</p>
    {:else if blockProps.length === 0}
      <p class="v4-ctx-empty">no block properties</p>
    {:else}
      <ul class="v4-ctx-list">
        {#each blockProps as p (p.key)}
          <li class="v4-prop-row">
            <span class="v4-prop-key">{p.key}</span>
            <span class="v4-prop-val">{p.value}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .v4-prop-section {
    margin-bottom: 14px;
  }
  .v4-prop-heading {
    font-family: var(--v4-mono);
    font-size: 9.5px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: var(--v4-ink5);
    margin: 0 0 5px;
  }
  .v4-prop-row {
    display: flex;
    gap: 8px;
    padding: 2px 6px;
    font-size: 12px;
  }
  .v4-prop-key {
    color: var(--v4-ink4);
    font-family: var(--v4-mono);
    font-size: 11px;
    flex-shrink: 0;
    min-width: 70px;
  }
  .v4-prop-val {
    color: var(--v4-ink2);
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>

<script lang="ts">
  /*
   * Prism v4 context-pane tab — properties of the followed note and its
   * focused block. Block properties are editable inline: click a value
   * to open a text input, Enter / blur commits, Escape cancels. An empty
   * commit clears the property; a non-empty commit writes it. Successful
   * writes invalidate the `["note", noteId]` query so the editor (and
   * anything else reading the note) reconciles.
   *
   * Page properties (note_type / tags / frontmatter) remain read-only —
   * there is no clean note-frontmatter write endpoint today, and the
   * legacy BottomDrawer's deeply-coupled inline editor has not been
   * ported to the v4 chrome. Edit those in the editor pane.
   */
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
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

  const queryClient = useQueryClient();

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

  // ── Inline edit state (block properties only) ────────────────────────────
  /** The key of the property currently being edited, or null. The panel
   *  edits one row at a time. */
  let editingKey = $state<string | null>(null);
  let draft = $state("");
  let inputEl = $state<HTMLInputElement | null>(null);
  /** Set by Escape; checked by the blur handler so a trailing blur after
   *  cancel doesn't accidentally commit a clear. */
  let cancelled = $state(false);

  function startEdit(key: string, value: string) {
    editingKey = key;
    draft = value;
    cancelled = false;
    // Focus + select on next microtask so the input has mounted.
    queueMicrotask(() => {
      if (inputEl) {
        inputEl.focus();
        inputEl.select();
      }
    });
  }

  function cancelEdit() {
    cancelled = true;
    editingKey = null;
  }

  async function commit(key: string, original: string) {
    // If Escape just fired, swallow the trailing blur — we don't want a
    // bare clear to be written just because the input unmounted.
    if (cancelled) {
      cancelled = false;
      editingKey = null;
      return;
    }
    const next = draft.trim();
    editingKey = null;
    draft = "";
    if (next === original) return; // no-op
    if (!focusedBlock || !noteId) return;
    // Address by the stale-proof `<note_id>:<bid>` when we have a bid;
    // the routes accept either form.
    const addr = focusedBlock.bid
      ? `${focusedBlock.note_id}:${focusedBlock.bid}`
      : focusedBlock.id;
    const k = key.toLowerCase();
    try {
      if (next === "") {
        await api.clearBlockProperty(addr, k);
      } else {
        await api.setBlockProperty(addr, k, next);
      }
      // Reconcile the editor's view of the note (and any other consumers
      // keyed on this note). The panel re-derives from `focusedBlock`,
      // which the editor will refetch + push back down.
      queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    } catch (err) {
      // Minimal: surface to console; the editor's own round-trip will
      // also surface the failure. No optimistic patch here — `focusedBlock`
      // is owned by the editor and we don't mirror it.
      console.error("[PropertiesView] failed to write property", { key, err });
    }
  }

  /** If the focused block swaps out from under us (e.g. user clicks a
   *  different block in the editor), abandon any in-flight edit. */
  $effect(() => {
    // Read focusedBlock so the effect re-runs on change.
    void focusedBlock;
    cancelled = true;
    editingKey = null;
    draft = "";
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
      <p class="v4-prop-hint">edit page properties in the editor pane</p>
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
            {#if editingKey === p.key}
              <input
                class="v4-prop-input"
                type="text"
                bind:value={draft}
                bind:this={inputEl}
                onkeydown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    commit(p.key, p.value);
                  } else if (e.key === "Escape") {
                    e.preventDefault();
                    cancelEdit();
                  }
                }}
                onblur={() => commit(p.key, p.value)}
              />
            {:else}
              <button
                type="button"
                class="v4-prop-val v4-prop-val--editable"
                onclick={() => startEdit(p.key, p.value)}
                title="Click to edit"
              >{p.value}</button>
            {/if}
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
    font-family: var(--theme-font-mono);
    font-size: 9.5px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: var(--fg-faint);
    margin: 0 0 5px;
  }
  .v4-prop-row {
    display: flex;
    gap: 8px;
    padding: 2px 6px;
    font-size: 12px;
    align-items: center;
  }
  .v4-prop-key {
    color: var(--fg-subtle);
    font-family: var(--theme-font-mono);
    font-size: 11px;
    flex-shrink: 0;
    min-width: 70px;
  }
  .v4-prop-val {
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }
  .v4-prop-val--editable {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 3px;
    padding: 1px 5px;
    margin: -1px -5px;
    font: inherit;
    color: inherit;
    text-align: left;
    cursor: text;
    width: 100%;
  }
  .v4-prop-val--editable:hover {
    background: var(--bg-2);
    border-color: var(--line-soft);
    color: var(--fg-default);
  }
  .v4-prop-val--editable:focus-visible {
    outline: none;
    background: var(--bg-2);
    border-color: var(--line);
    color: var(--fg-default);
  }
  .v4-prop-input {
    flex: 1;
    min-width: 0;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 3px;
    color: var(--fg-default);
    font: inherit;
    font-size: 12px;
    padding: 1px 5px;
    margin: -1px -5px;
  }
  .v4-prop-input:focus {
    outline: none;
    border-color: var(--accent-spark-dim);
  }
  .v4-prop-hint {
    font-family: var(--theme-font-mono);
    font-size: 10px;
    color: var(--fg-faint);
    font-style: italic;
    margin: 4px 6px 0;
  }
</style>

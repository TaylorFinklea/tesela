<script lang="ts">
  /* `backlinks-of-tag` — derived renderer for `Reference: tag`.
   *
   * Lists every block whose content contains the focused tag (either inline
   * `#tag` or via the legacy `tags::` property line), with reading context.
   * The renderer is a thin styling layer on top of `getTypedBlocks` (same
   * source as `instances-of-tag`'s block section); the difference is the
   * presentation — backlinks is "context-first", instances is "table-first".
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  let {
    reference,
    onNavigate,
  }: DerivedRendererProps<{ kind: "tag"; value: string }> = $props();

  const tagName = $derived(reference.value);

  const blocksQuery = createQuery(() => ({
    queryKey: ["typed-blocks", tagName] as const,
    queryFn: () => api.getTypedBlocks(tagName),
    enabled: !!tagName,
  }));

  const blocks = $derived((blocksQuery.data ?? []) as ParsedBlock[]);

  function cleanBlockText(raw: string, max = 140): string {
    let s = raw.replace(/<!--\s*bid:[^>]*-->/g, "").trim();
    s = s.replace(/^[-*]\s+/, "");
    s = s.replace(/\s+/g, " ").trim();
    if (s.length > max) s = s.slice(0, max - 1) + "…";
    return s;
  }
</script>

{#if !tagName}
  <p class="empty">no tag focused</p>
{:else}
  <div class="root">
    <h3>
      Blocks tagged #{tagName} <span class="count">{blocks.length}</span>
    </h3>
    {#if blocksQuery.isLoading}
      <p class="muted">loading…</p>
    {:else if blocks.length === 0}
      <p class="muted">no blocks tagged</p>
    {:else}
      <ul>
        {#each blocks as b (b.id)}
          <li>
            <button
              type="button"
              onclick={() =>
                onNavigate({
                  kind: "open-page",
                  path: b.note_id,
                  how: "replace",
                })}
            >
              <span class="src">{b.note_id}</span>
              <span class="ctx">{cleanBlockText(b.raw_text)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .root {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  h3 {
    margin: 0;
    color: var(--v4-ink);
    font-family: var(--v4-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .count {
    color: var(--v4-ink5);
    font-size: 10.5px;
    font-weight: 400;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: flex-start;
    text-align: left;
    width: 100%;
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 5px;
    padding: 6px 10px;
    cursor: pointer;
    font-family: var(--v4-mono);
    color: var(--v4-ink2);
  }
  li button:hover {
    border-color: var(--v4-hair2);
    background: var(--v4-surface-lo);
  }
  .src {
    font-size: 11px;
    color: var(--v4-ink3);
  }
  .ctx {
    font-size: 11.5px;
    color: var(--v4-ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    width: 100%;
  }
  .muted,
  .empty {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  .empty {
    text-align: center;
    padding: 16px 0;
  }
</style>

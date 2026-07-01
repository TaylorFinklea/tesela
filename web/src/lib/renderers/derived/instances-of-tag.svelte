<script lang="ts">
  /* `instances-of-tag` — derived renderer for `Reference: tag`.
   *
   * Lists every entity that is an instance of the focused tag:
   *   - page-level instance: page has the tag in its frontmatter `tags:`
   *   - block-level instance: a block has the tag inline (`#tag`) or via
   *     the legacy `tags::` property line
   *
   * Per the spec, a page that is both page-level AND has block-level
   * instances renders as one page-level row plus N block-level rows. Not
   * deduplicated — they answer different questions.
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  let {
    reference,
    onNavigate,
  }: DerivedRendererProps<{ kind: "tag"; value: string }> = $props();

  const tagName = $derived(reference.value);

  /** Page-level instances: notes with this tag in their frontmatter.
   *  Raised 500→5000 (tesela-sclr.1): a heavily-used tag can exceed 500
   *  instances, silently truncating this list past that point. */
  const pagesQuery = createQuery(() => ({
    queryKey: ["notes", { tag: tagName, limit: 5000 }] as const,
    queryFn: () => api.listNotes({ tag: tagName, limit: 5000 }),
    enabled: !!tagName,
  }));

  /** Block-level instances: blocks with this tag (inline or `tags::`). */
  const blocksQuery = createQuery(() => ({
    queryKey: ["typed-blocks", tagName] as const,
    queryFn: () => api.getTypedBlocks(tagName),
    enabled: !!tagName,
  }));

  const pages = $derived((pagesQuery.data ?? []) as Note[]);
  const blocks = $derived((blocksQuery.data ?? []) as ParsedBlock[]);

  /** Clean a block's raw text for inline display — strip the bid comment,
   *  collapse whitespace, cap length. */
  function cleanBlockText(raw: string, max = 100): string {
    let s = raw.replace(/<!--\s*bid:[^>]*-->/g, "").trim();
    s = s.replace(/^[-*]\s+/, "");
    s = s.replace(/\s+/g, " ").trim();
    if (s.length > max) s = s.slice(0, max - 1) + "…";
    return s;
  }

  function openPage(path: string) {
    onNavigate({ kind: "open-page", path, how: "replace" });
  }
</script>

{#if !tagName}
  <p class="empty">no tag focused</p>
{:else}
  <div class="root">
    <section>
      <h3>
        Page instances <span class="count">{pages.length}</span>
      </h3>
      {#if pagesQuery.isLoading}
        <p class="muted">loading…</p>
      {:else if pages.length === 0}
        <p class="muted">no pages with this tag in frontmatter</p>
      {:else}
        <ul>
          {#each pages as p (p.id)}
            <li>
              <button type="button" onclick={() => openPage(p.id)}>
                <span class="kind-glyph">▣</span>
                <span class="title">{p.title}</span>
                <span class="kind-chip">page</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section>
      <h3>
        Block instances <span class="count">{blocks.length}</span>
      </h3>
      {#if blocksQuery.isLoading}
        <p class="muted">loading…</p>
      {:else if blocks.length === 0}
        <p class="muted">no blocks tagged with #{tagName}</p>
      {:else}
        <ul>
          {#each blocks as b (b.id)}
            <li>
              <button type="button" onclick={() => openPage(b.note_id)}>
                <span class="kind-glyph">·</span>
                <span class="context">{cleanBlockText(b.raw_text)}</span>
                <span class="parent">↳ {b.note_id}</span>
                <span class="kind-chip">block</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
{/if}

<style>
  .root {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 0;
  }
  section {
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
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    align-items: center;
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
  .kind-glyph {
    color: var(--v4-ink5);
    font-size: 14px;
  }
  .title {
    font-size: 12px;
    color: var(--v4-ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .context {
    font-size: 11.5px;
    color: var(--v4-ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .parent {
    font-size: 10.5px;
    color: var(--v4-ink5);
  }
  .kind-chip {
    font-size: 9.5px;
    color: var(--v4-ink5);
    border: 1px solid var(--v4-hair);
    border-radius: 4px;
    padding: 0 6px;
    text-transform: uppercase;
    letter-spacing: 0.4px;
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

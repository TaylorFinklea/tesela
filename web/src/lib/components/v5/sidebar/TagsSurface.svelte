<script lang="ts">
  /* Tags surface.
   *
   * Two-section layout:
   *   1. **Tag entities** — pages with `type: tag`. Each row shows the
   *      tag's slug, usage count (how many pages reference it in their
   *      frontmatter `tags:`), and the parent path if any.
   *   2. **Orphan tags** — tag names that appear in some page's
   *      frontmatter `tags:` array but have no `type: tag` page on disk.
   *      Auto-create happens server-side on next save; this section
   *      surfaces the gap until the next save lands.
   *
   * Clicking any row opens the tag's page in the focused buffer. For
   * orphan tags we open by NoteId — the server's `ensure_tag_pages` will
   * have materialized it on save, so the click finds something to load.
   *
   * Inline filter input at the top scopes both sections.
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  const q = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const all = $derived((q.data ?? []) as Note[]);

  /** Per-tag-name usage count, computed from every note's frontmatter
   *  `tags: [...]`. Case-insensitive (everything lower-cased). */
  const usageCount = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const n of all) {
      const ts = (n.metadata.tags ?? []) as string[];
      for (const t of ts) {
        const lower = t.toLowerCase();
        counts.set(lower, (counts.get(lower) ?? 0) + 1);
      }
    }
    return counts;
  });

  /** Tag pages on disk — `type: tag` (case-insensitive). */
  type TagEntity = {
    /** NoteId / slug (used for routing). */
    id: string;
    /** Display name (`title:` frontmatter, falls back to id). */
    title: string;
    /** Parent slug from frontmatter `parent:`, normalized lowercase. */
    parent: string;
    /** Usage count from frontmatter aggregation. */
    count: number;
  };
  const tagEntities = $derived.by((): TagEntity[] => {
    return all
      .filter((n) => (n.metadata.note_type ?? "").toLowerCase() === "tag")
      .map((n): TagEntity => {
        const parent = (n.metadata.custom as Record<string, unknown> | undefined)?.[
          "parent"
        ];
        const parentStr =
          typeof parent === "string" ? parent.toLowerCase() : "";
        const title = n.title ?? n.id;
        return {
          id: n.id,
          title,
          parent: parentStr,
          count: usageCount.get(title.toLowerCase()) ?? 0,
        };
      });
  });

  /** Aggregated tag names that don't have a corresponding `type: tag`
   *  page on disk yet. */
  type OrphanTag = { name: string; count: number };
  const orphanTags = $derived.by((): OrphanTag[] => {
    const knownTitles = new Set(
      tagEntities.map((t) => t.title.toLowerCase()),
    );
    const out: OrphanTag[] = [];
    for (const [name, count] of usageCount.entries()) {
      if (!knownTitles.has(name)) out.push({ name, count });
    }
    return out;
  });

  // ── filtering ─────────────────────────────────────────────────────────────
  let filter = $state("");
  const filterLower = $derived(filter.trim().toLowerCase());

  const filteredEntities = $derived.by(() => {
    const f = filterLower;
    const list = tagEntities.slice().sort((a, b) => {
      // Sort: count desc, then parent path (for visual grouping under
      // parents), then slug.
      if (b.count !== a.count) return b.count - a.count;
      if (a.parent !== b.parent) return a.parent.localeCompare(b.parent);
      return a.title.localeCompare(b.title);
    });
    if (!f) return list;
    return list.filter(
      (t) =>
        t.title.toLowerCase().includes(f) ||
        t.parent.toLowerCase().includes(f),
    );
  });

  const filteredOrphans = $derived.by(() => {
    const f = filterLower;
    const list = orphanTags
      .slice()
      .sort((a, b) =>
        b.count !== a.count ? b.count - a.count : a.name.localeCompare(b.name),
      );
    if (!f) return list;
    return list.filter((t) => t.name.includes(f));
  });

  function openTag(name: string) {
    openPageInFocused(asPageId(name.toLowerCase()));
  }
</script>

<div class="v5-side-surface">
  <header>Tags</header>

  <input
    type="text"
    class="filter-input"
    placeholder="filter…"
    bind:value={filter}
  />

  {#if q.isLoading}
    <p class="muted">loading…</p>
  {:else}
    <section>
      <h4>
        Tag pages <span class="count">{filteredEntities.length}</span>
      </h4>
      {#if filteredEntities.length === 0}
        <p class="muted">no tag pages</p>
      {:else}
        <ul>
          {#each filteredEntities as t (t.id)}
            <li>
              <button
                type="button"
                class="row"
                onclick={() => openTag(t.id)}
                title="open #{t.title}"
              >
                <span class="name">#{t.title}</span>
                {#if t.parent}
                  <span class="parent">· {t.parent}</span>
                {/if}
                <span class="count-chip">{t.count}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    {#if filteredOrphans.length > 0}
      <section>
        <h4>
          Mentions without a page
          <span class="count">{filteredOrphans.length}</span>
        </h4>
        <ul>
          {#each filteredOrphans as t (t.name)}
            <li>
              <button
                type="button"
                class="row"
                onclick={() => openTag(t.name)}
                title="open #{t.name} (auto-creates)"
              >
                <span class="name">#{t.name}</span>
                <span class="parent muted">· orphan</span>
                <span class="count-chip">{t.count}</span>
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}
</div>

<style>
  .v5-side-surface {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
    font-family: var(--v4-mono);
    font-size: 11px;
    color: var(--v4-ink2);
  }
  header {
    color: var(--v4-ink);
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.7px;
    margin-bottom: 8px;
  }
  .filter-input {
    width: 100%;
    box-sizing: border-box;
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 4px;
    padding: 3px 6px;
    color: var(--v4-ink);
    font-family: inherit;
    font-size: 11px;
    margin-bottom: 8px;
  }
  .filter-input:focus {
    outline: none;
    border-color: var(--primary);
  }
  section {
    margin-bottom: 12px;
  }
  h4 {
    margin: 0 0 4px 0;
    color: var(--v4-ink3);
    text-transform: uppercase;
    font-size: 9.5px;
    letter-spacing: 0.6px;
    font-weight: 400;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .count {
    color: var(--v4-ink5);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li {
    display: block;
  }
  .row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: baseline;
    gap: 6px;
    padding: 3px 6px;
    border-radius: 4px;
    background: transparent;
    border: 0;
    color: inherit;
    text-align: left;
    font-family: inherit;
    font-size: inherit;
    width: 100%;
    cursor: pointer;
  }
  .row:hover {
    background: var(--v4-surface-lo);
  }
  .name {
    color: var(--v4-ink2);
  }
  .parent {
    color: var(--v4-ink5);
    font-size: 10px;
  }
  .count-chip {
    color: var(--v4-ink5);
    font-size: 10px;
    min-width: 18px;
    text-align: right;
  }
  .muted {
    color: var(--v4-ink5);
  }
</style>

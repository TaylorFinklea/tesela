<script lang="ts">
  /* Derived renderer wrapper: backlinks-of-page.
   *
   * Three sections:
   *   • Backlinks  — pages that reference this one, with block context
   *   • Forward links — pages this one references
   *   • Unlinked references — placeholder for the future Logseq-style
   *     fuzzy-match feature (see roadmap entry).
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";
  import type { Link } from "$lib/types/Link";

  let {
    reference,
    size,
    onNavigate,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();

  const pageId = $derived(reference.path || undefined);

  // In-mode adaptation: drop the verbose unlinked-references blurb on
  // narrow hosts (Peek, narrow splits). Cascade picks the mode; this
  // size-prop driven branch is just within-mode polish.
  const showUnlinkedSection = $derived(size.cols >= 70);

  import { useQueryClient } from "@tanstack/svelte-query";
  const queryClient = useQueryClient();

  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", pageId] as const,
    queryFn: () => api.getBacklinks(pageId as string),
    enabled: !!pageId,
  }));
  const forwardLinksQuery = createQuery(() => ({
    queryKey: ["forward-links", pageId] as const,
    queryFn: () => api.getForwardLinks(pageId as string),
    enabled: !!pageId,
  }));
  const unlinkedQuery = createQuery(() => ({
    queryKey: ["unlinked", pageId] as const,
    queryFn: () => api.getUnlinkedReferences(pageId as string),
    enabled: !!pageId,
  }));

  const backlinks = $derived((backlinksQuery.data ?? []) as Link[]);
  const forwardLinks = $derived((forwardLinksQuery.data ?? []) as Link[]);
  const unlinked = $derived((unlinkedQuery.data ?? []) as Link[]);

  /** Promote one unlinked reference to a real `[[wiki-link]]`. The backend
   *  match may be on the focused page's title OR one of its aliases (see
   *  `get_unlinked` / `find_unlinked_mentions` in tesela-server), so we
   *  can't assume `pageId` is the literal matched text. Instead, reload the
   *  focused page's title+aliases, rebuild the same needle set the backend
   *  used, and use the row's exact `position` (byte offset into the SOURCE
   *  note's `content`) to identify which needle matched there — then wrap
   *  exactly that span with `[[ ]]`. After save, refetch backlinks +
   *  unlinked so the UI shifts the row from "unlinked" to "backlinks". */
  async function promoteToLink(row: Link): Promise<void> {
    if (!pageId) return;
    const sourceId = row.target;
    const [src, page] = await Promise.all([
      api.getNote(sourceId),
      api.getNote(pageId),
    ]);
    const needles = Array.from(
      new Set(
        [page.title ?? pageId, ...(page.metadata.aliases ?? [])]
          .map((s) => s.trim().toLowerCase())
          .filter((s) => s.length >= 4),
      ),
    );
    const lower = src.content.toLowerCase();
    const pos = row.position;
    const needle = needles.find((n) => lower.startsWith(n, pos));
    if (needle == null) return;
    const next =
      src.content.slice(0, pos) +
      "[[" +
      src.content.slice(pos, pos + needle.length) +
      "]]" +
      src.content.slice(pos + needle.length);
    await api.updateNote(sourceId, next);
    queryClient.invalidateQueries({ queryKey: ["backlinks", pageId] });
    queryClient.invalidateQueries({ queryKey: ["unlinked", pageId] });
  }

  function open(p: string) {
    onNavigate({ kind: "open-page", path: p, how: "replace" });
  }

  /** Clean the raw block text for context display:
   *  - strip the `<!-- bid:... -->` comment
   *  - strip the leading `- ` bullet
   *  - collapse whitespace
   *  - cap length (truncate with ellipsis) */
  function cleanContext(raw: string, max = 120): string {
    let s = raw.replace(/<!--\s*bid:[^>]*-->/g, "").trim();
    s = s.replace(/^[-*]\s+/, "");
    s = s.replace(/\s+/g, " ").trim();
    if (s.length > max) s = s.slice(0, max - 1) + "…";
    return s;
  }
</script>

{#if !pageId}
  <p class="empty">no page focused</p>
{:else}
  <div class="root">
    <section>
      <h3>Backlinks <span class="count">{backlinks.length}</span></h3>
      {#if backlinksQuery.isLoading}
        <p class="muted">loading…</p>
      {:else if backlinks.length === 0}
        <p class="muted">no pages link here</p>
      {:else}
        <ul>
          {#each backlinks as b, i (b.target + ":" + b.position + ":" + i)}
            <li>
              <button type="button" onclick={() => open(b.target)}>
                <span class="src">{b.target}</span>
                <span class="ctx">{cleanContext(b.text)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section>
      <h3>Forward links <span class="count">{forwardLinks.length}</span></h3>
      {#if forwardLinksQuery.isLoading}
        <p class="muted">loading…</p>
      {:else if forwardLinks.length === 0}
        <p class="muted">no outbound links</p>
      {:else}
        <ul>
          {#each forwardLinks as f, i (f.target + ":" + f.position + ":" + i)}
            <li>
              <button type="button" onclick={() => open(f.target)}>
                <span class="src">→ {f.target}</span>
                <span class="ctx">{cleanContext(f.text)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    {#if showUnlinkedSection}
      <section>
        <h3>Unlinked references <span class="count">{unlinked.length}</span></h3>
        {#if unlinkedQuery.isLoading}
          <p class="muted">scanning…</p>
        {:else if unlinked.length === 0}
          <p class="muted">no unlinked mentions</p>
        {:else}
          <ul>
            {#each unlinked as u, i (u.target + ":" + u.position + ":" + i)}
              <li class="unlinked-row">
                <button
                  type="button"
                  class="row-open"
                  onclick={() => open(u.target)}
                >
                  <span class="src">{u.target}</span>
                  <span class="ctx">{cleanContext(u.text)}</span>
                </button>
                <button
                  type="button"
                  class="row-promote"
                  title="Promote this mention to a [[wiki-link]]"
                  onclick={() => void promoteToLink(u)}
                >Link</button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}
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
    color: var(--fg-default);
    font-family: var(--theme-font-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .count {
    color: var(--fg-faint);
    font-size: 10.5px;
    font-weight: 400;
  }
  .badge {
    color: var(--fg-faint);
    border: 1px solid var(--line-soft);
    border-radius: 4px;
    padding: 1px 6px;
    font-size: 9.5px;
    letter-spacing: 0.4px;
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
    border: 1px solid var(--line-soft);
    border-radius: 5px;
    padding: 6px 10px;
    cursor: pointer;
    font-family: var(--theme-font-mono);
    color: var(--fg-muted);
  }
  li button:hover {
    border-color: var(--line);
    background: var(--bg-2);
  }
  .src {
    font-size: 11px;
    color: var(--fg-subtle);
  }
  .ctx {
    font-size: 11.5px;
    color: var(--fg-default);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    width: 100%;
  }
  .muted {
    color: var(--fg-faint);
    font-family: var(--theme-font-mono);
    font-size: 11px;
  }
  .unlinked-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px;
    align-items: stretch;
  }
  .row-open {
    /* Reset the default li button styles so the open + Link buttons sit
     * cleanly side-by-side. */
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: left;
  }
  .row-promote {
    background: transparent;
    border: 1px solid var(--line-soft);
    border-radius: 5px;
    color: var(--accent-spark);
    padding: 0 10px;
    cursor: pointer;
    font-family: var(--theme-font-mono);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    align-self: center;
  }
  .row-promote:hover {
    border-color: var(--accent-spark-dim);
    background: color-mix(in srgb, var(--accent-spark) 12%, transparent);
  }
  .empty {
    color: var(--fg-faint);
    font-family: var(--theme-font-mono);
    font-size: 11px;
    text-align: center;
    padding: 16px 0;
  }
</style>

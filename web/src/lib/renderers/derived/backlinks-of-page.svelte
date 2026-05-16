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
    onNavigate,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();

  const pageId = $derived(reference.path || undefined);

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

  const backlinks = $derived((backlinksQuery.data ?? []) as Link[]);
  const forwardLinks = $derived((forwardLinksQuery.data ?? []) as Link[]);

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

    <section>
      <h3>Unlinked references <span class="badge">soon</span></h3>
      <p class="muted">
        Coming soon — like Logseq's unlinked references, this will surface
        places that mention this page's title or any of its aliases but
        aren't formally `[[wiki-linked]]`. Roadmap: fuzzy-match against
        page corpus on the backend, surface here for one-click linking.
      </p>
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
  .badge {
    color: var(--v4-ink5);
    border: 1px solid var(--v4-hair);
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
  .muted {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  .empty {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 11px;
    text-align: center;
    padding: 16px 0;
  }
</style>

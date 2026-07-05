<script lang="ts">
  /* Ambient buffer: today's in-progress tasks across the workspace. Uses
   * the existing api.listNotes with a tag/status filter approach mirroring
   * v4's `tasks` widget. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { Note } from "$lib/types/Note";

  let { onNavigate }: AmbientRendererProps = $props();

  const q = createQuery(() => ({
    queryKey: ["notes", { tag: "Task", limit: 200 }] as const,
    queryFn: () => api.listNotes({ tag: "Task", limit: 200 }),
  }));
  const tasks = $derived((q.data ?? []) as Note[]);
</script>

<div class="v5-tip">
  <header><b>In progress</b> · today</header>
  {#if q.isLoading}
    <p class="muted">loading…</p>
  {:else if tasks.length === 0}
    <p class="muted">no tasks</p>
  {:else}
    <ul>
      {#each tasks.slice(0, 50) as t (t.id)}
        <li>
          <button
            type="button"
            onclick={() =>
              onNavigate({ kind: "open-page", path: t.id, how: "replace" })}
            >{t.title}</button
          >
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .v5-tip {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 14px;
    font-family: var(--theme-font-mono);
    font-size: 12px;
    color: var(--fg-muted);
  }
  header {
    color: var(--fg-default);
    margin-bottom: 10px;
    font-size: 12.5px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li button {
    width: 100%;
    text-align: left;
    background: transparent;
    border: 1px solid var(--line-soft);
    border-radius: 5px;
    padding: 4px 8px;
    color: var(--fg-muted);
    font-family: var(--theme-font-mono);
    font-size: 11px;
    cursor: pointer;
  }
  li button:hover {
    border-color: var(--line);
    color: var(--fg-default);
  }
  .muted {
    color: var(--fg-faint);
  }
</style>

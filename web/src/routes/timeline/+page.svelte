<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { parseBlocks } from "$lib/block-parser";

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { tag: "daily", limit: 100 }] as const,
    queryFn: () => api.listNotes({ tag: "daily", limit: 100 }),
  }));

  const dailyNotes: Note[] = $derived(
    ((notesQuery.data ?? []) as Note[]).sort((a, b) =>
      b.title.localeCompare(a.title)
    ),
  );

  function splitBody(content: string): string {
    if (!content.startsWith("---")) return content;
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return content;
    const afterFm = content.slice(endIdx + 3);
    return afterFm.startsWith("\n") ? afterFm.slice(1) : afterFm;
  }

  function formatDate(dateStr: string): string {
    try {
      const d = new Date(dateStr + "T00:00:00");
      return d.toLocaleDateString(undefined, {
        weekday: "long",
        year: "numeric",
        month: "long",
        day: "numeric",
      });
    } catch {
      return dateStr;
    }
  }

  function isToday(dateStr: string): boolean {
    const today = new Date().toISOString().slice(0, 10);
    return dateStr === today;
  }
</script>

<div class="flex-1 flex flex-col">
  <header class="border-b border-border px-6 py-3 flex items-center justify-between">
    <span class="text-xs text-muted-foreground">Daily Notes Timeline</span>
  </header>

  <div class="flex-1 overflow-y-auto">
    {#if notesQuery.isLoading}
      <div class="px-8 py-6 text-sm text-muted-foreground">Loading…</div>
    {:else if dailyNotes.length === 0}
      <div class="px-8 py-6 text-sm text-muted-foreground">
        No daily notes yet. Click <a href="/daily" class="underline">Today</a> to create one.
      </div>
    {:else}
      <div class="max-w-3xl mx-auto py-4">
        {#each dailyNotes as note (note.id)}
          {@const body = splitBody(note.content)}
          {@const blocks = parseBlocks(note.id, body)}
          <div class="px-8 py-4 {isToday(note.title) ? 'bg-accent/10 rounded-lg' : ''}">
            <a
              href="/p/{encodeURIComponent(note.id)}"
              class="flex items-baseline gap-3 group mb-2"
            >
              <h2 class="text-sm font-medium group-hover:underline">
                {formatDate(note.title)}
              </h2>
              {#if isToday(note.title)}
                <span class="text-[10px] px-1.5 py-0.5 rounded bg-primary text-primary-foreground">Today</span>
              {/if}
            </a>

            {#if blocks.length === 0}
              <div class="text-xs text-muted-foreground/60 italic ml-4">Empty</div>
            {:else}
              <ul class="space-y-0.5 ml-1">
                {#each blocks.slice(0, 10) as block}
                  <li class="flex items-start gap-1.5" style="padding-left: {block.indent_level * 16}px">
                    <span class="mt-[5px] h-1 w-1 shrink-0 rounded-full bg-muted-foreground/40"></span>
                    <span class="text-xs text-muted-foreground">{block.text || block.raw_text}</span>
                  </li>
                {/each}
                {#if blocks.length > 10}
                  <li class="text-xs text-muted-foreground/50 ml-3">+{blocks.length - 10} more</li>
                {/if}
              </ul>
            {/if}

            <div class="border-b border-border/30 mt-4"></div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { updateFrontmatterKey } from "$lib/property-registry";
  import type { Note } from "$lib/types/Note";

  let { note }: { note: Note } = $props();

  const queryClient = useQueryClient();

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));

  const tagPages = $derived(
    ((allNotesQuery.data ?? []) as Note[]).filter((n) => n.metadata.note_type === "Tag"),
  );

  const selectedTag = $derived((note.metadata.custom?.query_tag as string) ?? "");

  async function setQueryTag(tagName: string) {
    let content = note.content;
    if (!content.startsWith("---")) {
      content = `---\ntitle: "${note.title}"\n---\n${content}`;
    }
    const updated = await api.updateNote(note.id, updateFrontmatterKey(content, "query_tag", tagName));
    queryClient.setQueryData(["note", note.id], updated);
  }
</script>

<div class="space-y-3">
  <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-widest">Query Config</div>
  <div>
    <label class="text-[10px] text-muted-foreground/50 block mb-1" for="query-tag-select">Source Tag</label>
    <select
      id="query-tag-select"
      class="w-full text-[12px] bg-muted/60 border border-border/60 rounded px-2 py-1 text-foreground outline-none focus:border-primary/60 cursor-pointer"
      value={selectedTag}
      onchange={(e) => setQueryTag((e.target as HTMLSelectElement).value)}
    >
      <option value="">— pick a tag —</option>
      {#each tagPages as tagPage}
        <option value={tagPage.title}>{tagPage.title}</option>
      {/each}
    </select>
  </div>
</div>

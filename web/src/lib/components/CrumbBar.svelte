<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";

  const path = $derived(page.url.pathname);

  // Map path → section label.
  const section = $derived.by(() => {
    if (path === "/") return "Pages";
    if (path === "/daily") return "Today";
    if (path === "/timeline") return "Timeline";
    if (path === "/graph") return "Graph";
    if (path === "/properties") return "Properties";
    if (path === "/settings") return "Settings";
    if (path.startsWith("/p/")) return "Pages";
    return "";
  });

  const noteId = $derived(path.startsWith("/p/") ? decodeURIComponent(path.slice(3)) : "");
  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: noteId !== "",
  }));
  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);

  const noteTitle = $derived(note?.title ?? noteId);
</script>

<div class="v9-crumb">
  <span class="seg">Tesela</span>
  <span class="sep">›</span>
  {#if path.startsWith("/p/")}
    <span class="seg">{section}</span>
    <span class="sep">›</span>
    <span class="seg curr">{noteTitle}</span>
  {:else if section}
    <span class="seg curr">{section}</span>
  {/if}
  <span class="sp"></span>
  <span class="end"><kbd>⌘K</kbd> jump · <kbd>⌃w</kbd>+<kbd>hjkl</kbd> split · <kbd>b</kbd> bottom</span>
</div>

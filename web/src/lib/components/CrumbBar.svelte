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

  // Per-view keyboard hints (Phase 9.4). Selected by note id when on a Query
  // widget page; otherwise by route. Falls back to global hints.
  type Hint = { key: string; label: string };
  const hints = $derived.by((): Hint[] => {
    const isQuery = note?.metadata.note_type === "Query";
    if (isQuery && noteId === "inbox") {
      return [
        { key: "t/d/x", label: "triage" },
        { key: "p", label: "project" },
        { key: "j/k", label: "nav" },
      ];
    }
    if (isQuery) {
      return [
        { key: "j/k", label: "nav" },
        { key: "↵", label: "open" },
        { key: "⌘K", label: "jump" },
      ];
    }
    if (path === "/daily" || /^\/p\/\d{4}-\d{2}-\d{2}$/.test(path)) {
      return [
        { key: "i", label: "edit" },
        { key: "u", label: "undo" },
        { key: "⌘K", label: "jump" },
      ];
    }
    if (path === "/graph") {
      return [
        { key: "click", label: "navigate" },
        { key: "⌘K", label: "jump" },
      ];
    }
    if (path === "/timeline") {
      return [
        { key: "j/k", label: "scroll" },
        { key: "i", label: "edit" },
      ];
    }
    // Global default.
    return [
      { key: "⌘K", label: "jump" },
      { key: "⌃w+hjkl", label: "split" },
      { key: "b", label: "bottom" },
    ];
  });
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
  <span class="end">
    {#each hints as h, i}
      {#if i > 0}<span class="sep" style="margin: 0 4px;">·</span>{/if}
      <kbd>{h.key}</kbd>
      {h.label}
    {/each}
  </span>
</div>

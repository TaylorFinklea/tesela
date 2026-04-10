<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";

  let open = $state(false);
  let search = $state("");

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));

  const notes = $derived(notesQuery.data ?? [] as Note[]);
  const filtered = $derived(
    search
      ? notes.filter((n) => n.title.toLowerCase().includes(search.toLowerCase()))
      : notes,
  );

  let selectedIndex = $state(0);

  // Reset selection when filter changes
  $effect(() => {
    // Touch `search` to create the dependency
    search;
    selectedIndex = 0;
  });

  function navigateTo(path: string) {
    open = false;
    search = "";
    goto(path);
  }

  // Actions shown before the notes list
  const actions = [
    { label: "Go to notes list", icon: "⌂", path: "/" },
    { label: "Go to daily note", icon: "☀", path: "/p/daily" },
  ];

  const allItems = $derived([
    ...actions.map((a) => ({ type: "action" as const, ...a })),
    ...filtered.map((n) => ({ type: "note" as const, label: n.title, path: `/p/${encodeURIComponent(n.id)}`, tags: n.metadata.tags })),
  ]);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      navigateTo(allItems[selectedIndex].path);
    } else if (e.key === "Escape") {
      e.preventDefault();
      open = false;
      search = "";
    }
  }

  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        open = !open;
        search = "";
        selectedIndex = 0;
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-50">
    <div class="absolute inset-0 bg-black/50" onclick={() => { open = false; search = ""; }}></div>
    <div class="absolute left-1/2 top-[20%] -translate-x-1/2 w-full max-w-lg">
      <div class="rounded-lg border border-border bg-popover text-popover-foreground shadow-2xl">
        <input
          type="text"
          placeholder="Search notes or type a command…"
          bind:value={search}
          onkeydown={handleKeydown}
          class="w-full border-b border-border bg-transparent px-4 py-3 text-sm outline-none placeholder:text-muted-foreground"
          autofocus
        />
        <div class="max-h-80 overflow-y-auto p-2">
          {#if allItems.length === 0}
            <div class="px-4 py-6 text-center text-sm text-muted-foreground">No results.</div>
          {:else}
            {#each allItems as item, i}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer {i === selectedIndex ? 'bg-accent text-accent-foreground' : ''}"
                onclick={() => navigateTo(item.path)}
                onmouseenter={() => (selectedIndex = i)}
              >
                {#if item.type === "action"}
                  <span class="text-muted-foreground mr-1">{item.icon}</span>
                {/if}
                <span class="truncate">{item.label}</span>
                {#if item.type === "note" && item.tags.length > 0}
                  <span class="ml-auto text-xs text-muted-foreground shrink-0">{item.tags.join(", ")}</span>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

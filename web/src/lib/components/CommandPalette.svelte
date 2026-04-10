<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { SearchHit } from "$lib/types/SearchHit";

  let open = $state(false);
  let search = $state("");
  let mode = $state<"navigate" | "search">("navigate");

  const queryClient = useQueryClient();

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));

  const notes = $derived(notesQuery.data ?? [] as Note[]);
  const titleFiltered = $derived(
    search
      ? notes.filter((n: Note) => n.title.toLowerCase().includes(search.toLowerCase()))
      : notes,
  );

  // Full-text search (debounced, only when search has 2+ chars)
  let searchResults = $state<SearchHit[]>([]);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (search.length >= 2 && mode === "navigate") {
      if (searchTimer) clearTimeout(searchTimer);
      searchTimer = setTimeout(async () => {
        try {
          searchResults = await api.search(search, 10);
        } catch {
          searchResults = [];
        }
      }, 200);
    } else {
      searchResults = [];
    }
  });

  let selectedIndex = $state(0);

  $effect(() => {
    search;
    selectedIndex = 0;
  });

  // Check if search matches any existing note title exactly
  const exactMatch = $derived(
    notes.some((n: Note) => n.title.toLowerCase() === search.toLowerCase()),
  );

  async function createAndNavigate(title: string) {
    try {
      const content = `---\ntitle: "${title}"\ntags: []\n---\n- `;
      const note = await api.createNote(title, content);
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      open = false;
      search = "";
      goto(`/p/${encodeURIComponent(note.id)}`);
    } catch (e) {
      console.error("Failed to create note:", e);
    }
  }

  async function goToDaily() {
    try {
      const note = await api.getDailyNote();
      open = false;
      search = "";
      goto(`/p/${encodeURIComponent(note.id)}`);
    } catch (e) {
      console.error("Failed to get daily note:", e);
    }
  }

  function navigateTo(path: string) {
    open = false;
    search = "";
    goto(path);
  }

  type Item =
    | { type: "action"; label: string; icon: string; action: () => void }
    | { type: "create"; label: string }
    | { type: "note"; label: string; path: string; tags: string[] }
    | { type: "search-hit"; label: string; snippet: string; path: string };

  const allItems: Item[] = $derived.by(() => {
    const items: Item[] = [];

    // Actions (always shown unless filtering narrows them out)
    if (!search || "go to notes list".includes(search.toLowerCase())) {
      items.push({ type: "action", label: "Go to notes list", icon: "⌂", action: () => navigateTo("/") });
    }
    if (!search || "go to daily note".includes(search.toLowerCase()) || "today".includes(search.toLowerCase())) {
      items.push({ type: "action", label: "Go to daily note", icon: "☀", action: () => goToDaily() });
    }

    // Create option (if search doesn't match an existing title)
    if (search.length > 0 && !exactMatch) {
      items.push({ type: "create", label: `Create "${search}"` });
    }

    // Title-matched notes
    for (const n of titleFiltered) {
      items.push({ type: "note", label: (n as Note).title, path: `/p/${encodeURIComponent((n as Note).id)}`, tags: (n as Note).metadata.tags });
    }

    // Full-text search results (only those not already in title matches)
    const titleIds = new Set(titleFiltered.map((n: Note) => n.id));
    for (const hit of searchResults) {
      if (!titleIds.has(hit.note_id)) {
        items.push({ type: "search-hit", label: hit.title, snippet: hit.snippet, path: `/p/${encodeURIComponent(hit.note_id)}` });
      }
    }

    return items;
  });

  function handleSelect(item: Item) {
    if (item.type === "action") {
      item.action();
    } else if (item.type === "create") {
      createAndNavigate(search);
    } else if (item.type === "note" || item.type === "search-hit") {
      navigateTo(item.path);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      handleSelect(allItems[selectedIndex]);
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
        mode = "navigate";
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
          placeholder="Search notes, create new, or type a command…"
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
                onclick={() => handleSelect(item)}
                onmouseenter={() => (selectedIndex = i)}
              >
                {#if item.type === "action"}
                  <span class="text-muted-foreground mr-1">{item.icon}</span>
                  <span>{item.label}</span>
                {:else if item.type === "create"}
                  <span class="text-muted-foreground mr-1">+</span>
                  <span>{item.label}</span>
                {:else if item.type === "note"}
                  <span class="truncate">{item.label}</span>
                  {#if item.tags.length > 0}
                    <span class="ml-auto text-xs text-muted-foreground shrink-0">{item.tags.join(", ")}</span>
                  {/if}
                {:else if item.type === "search-hit"}
                  <div class="flex flex-col min-w-0">
                    <span class="truncate">{item.label}</span>
                    <span class="text-xs text-muted-foreground truncate">{item.snippet}</span>
                  </div>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

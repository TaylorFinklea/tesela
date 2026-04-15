<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { getRecents } from "$lib/stores/recents.svelte";
  import { toggleFavorite } from "$lib/stores/favorites.svelte";
  import { getTheme, applyTheme } from "$lib/themes";
  import { buildCommands, matchesQuery, type Command } from "$lib/commands";
  import type { Note } from "$lib/types/Note";
  import type { SearchHit } from "$lib/types/SearchHit";
  import { IconSearch } from "@tabler/icons-svelte";

  let open = $state(false);
  let search = $state("");
  let selectedIndex = $state(0);

  const queryClient = useQueryClient();

  // Expose sidebar toggle and theme toggle for commands
  let onToggleSidebar: (() => void) | undefined = undefined;
  export function setSidebarToggle(fn: () => void) { onToggleSidebar = fn; }

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));

  const notes: Note[] = $derived((notesQuery.data ?? []) as Note[]);

  // Full-text search (debounced)
  let searchResults = $state<SearchHit[]>([]);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (search.length >= 2) {
      if (searchTimer) clearTimeout(searchTimer);
      searchTimer = setTimeout(async () => {
        try { searchResults = await api.search(search, 8); }
        catch { searchResults = []; }
      }, 150);
    } else {
      searchResults = [];
    }
  });

  // Current route context
  const isNotePage = $derived(page.url.pathname.startsWith("/p/"));
  const currentNoteId = $derived(isNotePage ? decodeURIComponent(page.url.pathname.slice(3)) : "");

  // Build commands
  const commands = $derived(buildCommands({
    goto: (path) => { close(); goto(path); },
    createNote: async (title) => {
      const name = title || search;
      if (!name) return;
      const content = `---\ntitle: "${name}"\ntags: []\n---\n`;
      const note = await api.createNote(name, content);
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      close();
      goto(`/p/${encodeURIComponent(note.id)}`);
    },
    createType: async (title) => {
      const name = title || search;
      if (!name) return;
      const content = `---\ntitle: "${name}"\ntype: "Tag"\nextends: "Root Tag"\ntag_properties: []\ntags: []\n---\n`;
      const note = await api.createNote(name, content);
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      close();
      goto(`/p/${encodeURIComponent(note.id)}`);
    },
    goToDaily: async () => {
      const note = await api.getDailyNote();
      close();
      goto(`/p/${encodeURIComponent(note.id)}`);
    },
    toggleSidebar: () => { onToggleSidebar?.(); close(); },
    toggleTheme: () => {
      const current = document.documentElement.getAttribute("data-theme") || "day";
      applyTheme(current === "day" ? "evening" : "day");
      close();
    },
    deleteNote: isNotePage ? () => {
      const confirmed = window.confirm("Delete this note? This cannot be undone.");
      if (confirmed) {
        api.deleteNote(currentNoteId);
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        close();
        goto("/");
      }
    } : undefined,
    copyNoteLink: isNotePage ? () => {
      navigator.clipboard.writeText(`tesela://p/${currentNoteId}`);
      close();
    } : undefined,
    toggleFavorite: isNotePage ? () => {
      toggleFavorite(currentNoteId);
      close();
    } : undefined,
  }));

  // --- Build sections ---

  type PaletteItem = {
    type: "command" | "note" | "search" | "create";
    label: string;
    sublabel?: string;
    htmlSublabel?: boolean;
    icon?: string;
    shortcut?: string;
    action: () => void;
  };

  type Section = { title: string; items: PaletteItem[] };

  const sections: Section[] = $derived.by(() => {
    const result: Section[] = [];

    if (!search) {
      // Empty state: Recent + Actions
      const recentIds = getRecents().slice(0, 5);
      const recentNotes = recentIds
        .map((id) => notes.find((n: Note) => n.id === id))
        .filter((n): n is Note => n !== undefined);

      if (recentNotes.length > 0) {
        result.push({
          title: "Recent",
          items: recentNotes.map((n) => ({
            type: "note" as const,
            label: n.title,
            sublabel: n.metadata.tags.length > 0 ? n.metadata.tags.join(", ") : undefined,
            action: () => { close(); goto(`/p/${encodeURIComponent(n.id)}`); },
          })),
        });
      }

      result.push({
        title: "Actions",
        items: commands.filter((c) => c.category === "action").map((c) => ({
          type: "command" as const,
          label: c.label,
          icon: c.icon,
          shortcut: c.shortcut,
          action: c.action,
        })),
      });

      if (commands.some((c) => c.category === "context")) {
        result.push({
          title: "This Note",
          items: commands.filter((c) => c.category === "context").map((c) => ({
            type: "command" as const,
            label: c.label,
            icon: c.icon,
            action: c.action,
          })),
        });
      }
    } else {
      // Search state
      const q = search.toLowerCase();

      // Matching actions
      const matchingCmds = commands.filter((c) => matchesQuery(c, search));
      if (matchingCmds.length > 0) {
        result.push({
          title: "Actions",
          items: matchingCmds.map((c) => ({
            type: "command" as const,
            label: c.label,
            icon: c.icon,
            shortcut: c.shortcut,
            action: c.action,
          })),
        });
      }

      // Create option (if no exact title match)
      const exactMatch = notes.some((n: Note) => n.title.toLowerCase() === q);
      if (!exactMatch && search.length > 0) {
        result.push({
          title: "Create",
          items: [
            { type: "create" as const, label: `Create "${search}"`, icon: "IconFilePlus", action: () => commands.find((c) => c.id === "new-note")?.action() },
            { type: "create" as const, label: `Create Type "${search}"`, icon: "IconTag", action: () => commands.find((c) => c.id === "new-type")?.action() },
          ],
        });
      }

      // Matching notes
      const matchingNotes = notes.filter((n: Note) => n.title.toLowerCase().includes(q));
      if (matchingNotes.length > 0) {
        result.push({
          title: "Notes",
          items: matchingNotes.slice(0, 10).map((n) => ({
            type: "note" as const,
            label: n.title,
            sublabel: n.metadata.tags.length > 0 ? n.metadata.tags.join(", ") : undefined,
            action: () => { close(); goto(`/p/${encodeURIComponent(n.id)}`); },
          })),
        });
      }

      // Full-text search results
      const titleIds = new Set(matchingNotes.map((n: Note) => n.id));
      const uniqueSearchResults = searchResults.filter((h) => !titleIds.has(h.note_id));
      if (uniqueSearchResults.length > 0) {
        result.push({
          title: "Search",
          items: uniqueSearchResults.map((h) => ({
            type: "search" as const,
            label: h.title,
            sublabel: h.snippet.replace(/<(?!\/?b>)[^>]+>/g, ""),
            htmlSublabel: true,
            action: () => { close(); goto(`/p/${encodeURIComponent(h.note_id)}`); },
          })),
        });
      }
    }

    return result;
  });

  // Flat list of all items for keyboard navigation
  const allItems = $derived(sections.flatMap((s) => s.items));

  $effect(() => { search; selectedIndex = 0; });

  function close() {
    open = false;
    search = "";
    selectedIndex = 0;
  }

  function handleKeydown(e: KeyboardEvent) {
    const down = e.key === "ArrowDown" || (e.ctrlKey && e.key === "j");
    const up = e.key === "ArrowUp" || (e.ctrlKey && e.key === "k");
    if (down) {
      e.preventDefault();
      selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1);
    } else if (up) {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      allItems[selectedIndex].action();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (search) { search = ""; }
      else { close(); }
    }
  }

  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        if (open) close();
        else { open = true; search = ""; selectedIndex = 0; }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]" style="animation: palette-fade-in 0.12s ease-out;">
    <div class="absolute inset-0 bg-foreground/10 backdrop-blur-sm" onclick={close}></div>

    <div class="relative w-full max-w-[560px] max-h-[70vh] flex flex-col rounded-xl border border-border bg-popover text-popover-foreground shadow-2xl overflow-hidden" style="animation: palette-slide-in 0.12s ease-out;">
      <!-- Search input -->
      <div class="flex items-center gap-3 px-4 py-3 border-b border-border">
        <IconSearch size={16} stroke={1.5} class="text-muted-foreground shrink-0" />
        <input
          type="text"
          placeholder="Search commands, notes, or content…"
          bind:value={search}
          onkeydown={handleKeydown}
          class="w-full text-[14px] bg-transparent outline-none placeholder:text-muted-foreground/50"
          autofocus
        />
        <kbd class="text-[10px] font-mono text-muted-foreground bg-muted px-1.5 py-0.5 rounded shrink-0">esc</kbd>
      </div>

      <!-- Results -->
      <div class="max-h-[400px] overflow-y-auto py-1">
        {#if allItems.length === 0}
          <div class="px-4 py-8 text-center text-[13px] text-muted-foreground">No results</div>
        {:else}
          {#each sections as section}
            <div class="px-3 pt-2 pb-1">
              <div class="text-[10px] font-semibold text-muted-foreground uppercase tracking-[0.12em] px-2">{section.title}</div>
            </div>
            {#each section.items as item}
              {@const globalIdx = allItems.indexOf(item)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="mx-2 flex items-center gap-3 rounded-lg px-3 py-2 text-[13px] cursor-pointer transition-colors {globalIdx === selectedIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'}"
                onclick={() => item.action()}
                onmouseenter={() => (selectedIndex = globalIdx)}
              >
                {#if item.type === "create"}
                  <span class="text-primary text-[14px]">+</span>
                {/if}
                <span class="flex-1 truncate">{item.label}</span>
                {#if item.sublabel}
                  {#if item.htmlSublabel}
                    <span class="text-[11px] text-muted-foreground truncate max-w-[220px] [&>b]:text-foreground [&>b]:font-semibold">{@html item.sublabel}</span>
                  {:else}
                    <span class="text-[11px] text-muted-foreground truncate max-w-[180px]">{item.sublabel}</span>
                  {/if}
                {/if}
                {#if item.shortcut}
                  <kbd class="text-[10px] font-mono text-muted-foreground bg-muted px-1.5 py-0.5 rounded shrink-0">{item.shortcut}</kbd>
                {/if}
              </div>
            {/each}
          {/each}
        {/if}
      </div>

      <!-- Footer hint -->
      <div class="border-t border-border px-4 py-2 flex items-center gap-4 text-[10px] text-muted-foreground">
        <span><kbd class="font-mono bg-muted px-1 py-px rounded">↑↓</kbd> navigate</span>
        <span><kbd class="font-mono bg-muted px-1 py-px rounded">↵</kbd> select</span>
        <span><kbd class="font-mono bg-muted px-1 py-px rounded">esc</kbd> close</span>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes palette-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes palette-slide-in {
    from { opacity: 0; transform: translateY(-8px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>

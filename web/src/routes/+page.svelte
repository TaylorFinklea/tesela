<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { browser } from "$app/environment";
  import { api, ApiError } from "$lib/api-client";
  import { getConnected } from "$lib/ws-client.svelte";
  import { isFavorite } from "$lib/stores/favorites.svelte";
  import {
    IconTable,
    IconLayoutGrid,
    IconList,
  } from "@tabler/icons-svelte";
  import TabStrip from "$lib/components/TabStrip.svelte";
  import ViewSwitcher from "$lib/components/ViewSwitcher.svelte";
  import type { Note } from "$lib/types/Note";

  type ViewMode = "table" | "cards" | "list";
  const ALL_VIEWS: ViewMode[] = ["table", "cards", "list"];

  /** A saved view tab (filters/sort/columns/view-mode applied to all notes). */
  type Tab = {
    name: string;
    view: ViewMode;
    /** Filter token language similar to QueryBlock's: tag:Foo, recent, favorite, etc. */
    filter: string;
  };

  const DEFAULT_TABS: Tab[] = [
    { name: "All",       view: "table", filter: "" },
    { name: "Recent",    view: "table", filter: "recent" },
    { name: "Favorites", view: "table", filter: "favorite" },
  ];

  const STORAGE_KEY = "tesela:home-views";
  const ACTIVE_KEY = "tesela:home-active-view";

  function loadTabs(): Tab[] {
    if (!browser) return DEFAULT_TABS;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return DEFAULT_TABS;
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed) && parsed.length > 0) {
        return parsed.map((t, i): Tab => ({
          name: typeof t?.name === "string" ? t.name : `View ${i + 1}`,
          view: (ALL_VIEWS as string[]).includes(t?.view) ? t.view : "table",
          filter: typeof t?.filter === "string" ? t.filter : "",
        }));
      }
    } catch { /* fall through */ }
    return DEFAULT_TABS;
  }
  function loadActive(): number {
    if (!browser) return 0;
    const raw = localStorage.getItem(ACTIVE_KEY);
    const n = raw ? parseInt(raw, 10) : 0;
    return Number.isNaN(n) ? 0 : n;
  }

  let tabs = $state<Tab[]>(loadTabs());
  let activeIdx = $state<number>(loadActive());

  function persist() {
    if (!browser) return;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(tabs));
      localStorage.setItem(ACTIVE_KEY, String(activeIdx));
    } catch { /* localStorage full or blocked */ }
  }

  function setActiveTab(i: number) {
    activeIdx = Math.max(0, Math.min(tabs.length - 1, i));
    persist();
  }
  function setView(view: ViewMode) {
    tabs = tabs.map((t, i) => i === activeIdx ? { ...t, view } : t);
    persist();
  }
  function setFilter(filter: string) {
    tabs = tabs.map((t, i) => i === activeIdx ? { ...t, filter } : t);
    persist();
  }
  function addTab() {
    tabs = [...tabs, { name: `View ${tabs.length + 1}`, view: "table", filter: "" }];
    activeIdx = tabs.length - 1;
    persist();
  }
  function deleteTab(i: number) {
    if (tabs.length <= 1) return;
    tabs = tabs.filter((_, idx) => idx !== i);
    if (activeIdx >= tabs.length) activeIdx = tabs.length - 1;
    persist();
  }
  function renameTab(i: number, name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    tabs = tabs.map((t, idx) => idx === i ? { ...t, name: trimmed } : t);
    persist();
  }

  const activeTab = $derived(tabs[activeIdx] ?? tabs[0]);

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const allNotes = $derived((notesQuery.data ?? []) as Note[]);
  const wsConnected = $derived(getConnected());

  /**
   * Apply the active tab's filter to the notes list. Tokens supported:
   *   - `recent` — last 30 days by modified_at
   *   - `favorite` — note id is in the favorites store
   *   - `tag:Foo` — note has tag Foo (case-insensitive)
   *   - `type:Tag` — note has note_type Tag
   *   - any bare word — substring match against title
   */
  const filteredNotes = $derived.by(() => {
    const tokens = activeTab.filter.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const recentCutoff = Date.now() - 30 * 24 * 60 * 60 * 1000;
    return allNotes.filter((n) => {
      for (const tok of tokens) {
        if (tok === "recent") {
          const m = Date.parse(n.modified_at ?? "");
          if (Number.isNaN(m) || m < recentCutoff) return false;
        } else if (tok === "favorite" || tok === "favorites") {
          if (!isFavorite(n.id)) return false;
        } else if (tok.startsWith("tag:")) {
          const want = tok.slice(4);
          if (!n.metadata.tags.some((t) => t.toLowerCase() === want)) return false;
        } else if (tok.startsWith("type:")) {
          const want = tok.slice(5);
          if ((n.metadata.note_type ?? "").toLowerCase() !== want) return false;
        } else {
          if (!n.title.toLowerCase().includes(tok)) return false;
        }
      }
      return true;
    }).sort((a, b) => Date.parse(b.modified_at ?? "0") - Date.parse(a.modified_at ?? "0"));
  });

  function formatTimestamp(iso: string | null): string {
    if (!iso) return "";
    try {
      return new Date(iso).toLocaleString(undefined, {
        year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
      });
    } catch {
      return iso;
    }
  }

  const VIEW_META: { id: ViewMode; label: string; Icon: typeof IconTable }[] = [
    { id: "table", label: "Table", Icon: IconTable },
    { id: "cards", label: "Cards", Icon: IconLayoutGrid },
    { id: "list",  label: "List",  Icon: IconList },
  ];

  let filterDraft = $state("");
  let editingFilter = $state(false);
  function openFilterEditor() {
    filterDraft = activeTab.filter;
    editingFilter = true;
  }
  function commitFilter() {
    setFilter(filterDraft.trim());
    editingFilter = false;
  }
</script>

<div class="flex-1 flex flex-col min-h-0">
  <header class="border-b border-border px-8 h-14 flex items-center justify-between shrink-0">
    <h1 class="font-display text-xl font-semibold tracking-tight">All Notes</h1>
    <div class="flex items-center gap-2 text-[12px] text-muted-foreground">
      <span class="inline-block h-[6px] w-[6px] rounded-full {wsConnected ? 'bg-emerald-500' : 'bg-muted-foreground'}"></span>
      <span>{notesQuery.isLoading ? "loading" : wsConnected ? "live" : "offline"}</span>
    </div>
  </header>

  <!-- Tab strip + view switcher -->
  <div class="border-b border-border/40 px-6 py-2 flex items-center justify-between gap-3 shrink-0">
    <TabStrip
      tabs={tabs}
      activeIdx={activeIdx}
      onSelect={setActiveTab}
      onAdd={addTab}
      onDelete={deleteTab}
      onRename={renameTab}
    />

    <div class="flex items-center gap-2 shrink-0">
      <!-- Filter chip -->
      {#if editingFilter}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          autofocus
          type="text"
          bind:value={filterDraft}
          placeholder="filter (e.g. tag:Task recent)"
          onblur={commitFilter}
          onkeydown={(e) => {
            if (e.key === "Enter") commitFilter();
            if (e.key === "Escape") { editingFilter = false; }
          }}
          class="text-[11px] bg-surface border border-primary/40 rounded px-2 py-0.5 outline-none w-64"
        />
      {:else}
        <button
          class="text-[11px] px-2 py-0.5 rounded bg-muted/40 hover:bg-muted/60 transition-colors {activeTab.filter ? 'text-primary/80' : 'text-muted-foreground/60'}"
          onclick={openFilterEditor}
          title="Click to edit filter"
        >
          {activeTab.filter || "no filter"}
        </button>
      {/if}

      <ViewSwitcher views={VIEW_META} active={activeTab.view} onChange={setView} />
    </div>
  </div>

  <section class="flex-1 overflow-y-auto">
    {#if notesQuery.isLoading}
      <div class="px-6 py-8 text-sm text-muted-foreground">Loading…</div>
    {:else if notesQuery.isError}
      {@const error = notesQuery.error}
      <div class="px-6 py-8 text-sm">
        <div class="text-destructive font-medium">Could not reach tesela-server</div>
        <div class="mt-1 text-muted-foreground">
          {error instanceof ApiError ? `${error.status} — ${error.body || "no body"}` : error.message}
        </div>
        <div class="mt-3 text-xs text-muted-foreground">
          Start it with <code class="font-mono">cargo run -p tesela-server</code> and reload.
        </div>
      </div>
    {:else if filteredNotes.length === 0}
      <div class="px-6 py-8 text-sm text-muted-foreground/60 italic">
        No notes match this view{activeTab.filter ? ` (filter: ${activeTab.filter})` : ""}.
      </div>

    <!-- TABLE -->
    {:else if activeTab.view === "table"}
      <table class="w-full text-[12px]">
        <thead>
          <tr class="text-[10px] text-muted-foreground/70 uppercase tracking-wider border-b border-border/40">
            <th class="text-left font-medium py-2 px-4">Title</th>
            <th class="text-left font-medium py-2 px-4">Type</th>
            <th class="text-left font-medium py-2 px-4">Tags</th>
            <th class="text-left font-medium py-2 px-4">Updated</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredNotes as note (note.id)}
            <tr class="border-b border-border/20 hover:bg-muted/20 transition-colors">
              <td class="px-4 py-2">
                <a href="/p/{encodeURIComponent(note.id)}" class="text-foreground/85 font-medium hover:text-primary transition-colors">{note.title}</a>
              </td>
              <td class="px-4 py-2 text-muted-foreground/70">{note.metadata.note_type ?? ""}</td>
              <td class="px-4 py-2">
                <div class="flex flex-wrap gap-0.5">
                  {#each note.metadata.tags as tag}
                    <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70">{tag}</span>
                  {/each}
                </div>
              </td>
              <td class="px-4 py-2 text-muted-foreground/60 font-mono text-[11px]">{formatTimestamp(note.modified_at)}</td>
            </tr>
          {/each}
        </tbody>
      </table>

    <!-- CARDS -->
    {:else if activeTab.view === "cards"}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3 p-4">
        {#each filteredNotes as note (note.id)}
          <a
            href="/p/{encodeURIComponent(note.id)}"
            class="block p-4 rounded-md border border-border/40 hover:border-primary/40 hover:bg-muted/20 transition-colors"
          >
            <div class="flex items-center gap-2">
              <div class="text-[14px] text-foreground/90 font-medium flex-1 min-w-0 truncate">{note.title}</div>
              {#if note.metadata.note_type}
                <span class="text-[10px] text-muted-foreground/60">{note.metadata.note_type}</span>
              {/if}
            </div>
            <div class="flex items-center flex-wrap gap-1 mt-2">
              {#each note.metadata.tags as tag}
                <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70">{tag}</span>
              {/each}
            </div>
            <div class="text-[10px] text-muted-foreground/50 font-mono mt-2">{formatTimestamp(note.modified_at)}</div>
          </a>
        {/each}
      </div>

    <!-- LIST (the original compact look) -->
    {:else}
      <ul>
        {#each filteredNotes as note (note.id)}
          <li>
            <a href="/p/{encodeURIComponent(note.id)}" class="block px-6 py-3 hover:bg-muted/30 transition-all border-b border-border/30">
              <div class="flex items-center justify-between gap-4">
                <div class="flex items-center gap-2.5 min-w-0">
                  <span class="text-[13px] font-medium truncate">{note.title}</span>
                  {#each note.metadata.tags.slice(0, 3) as tag}
                    <span class="text-[9px] px-1.5 py-px rounded-full bg-primary/10 text-primary/70 shrink-0">{tag}</span>
                  {/each}
                  {#if note.metadata.note_type}
                    <span class="text-[9px] text-muted-foreground/40">{note.metadata.note_type}</span>
                  {/if}
                </div>
                <span class="text-[10px] text-muted-foreground/40 font-mono shrink-0">{formatTimestamp(note.modified_at)}</span>
              </div>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";

  let { onclose }: { onclose: () => void } = $props();

  type MenuItem = {
    key: string;
    label: string;
    action?: () => void;
    children?: MenuItem[];
  };

  let currentLevel = $state<MenuItem[]>([]);
  let breadcrumb = $state<string[]>([]);
  let selectedIndex = $state(0);

  const rootMenu: MenuItem[] = [
    {
      key: "f",
      label: "File",
      children: [
        { key: "n", label: "New note", action: () => openNewNote() },
        { key: "d", label: "Daily note", action: () => goToDaily() },
        { key: "h", label: "Home", action: () => { goto("/"); onclose(); } },
      ],
    },
    {
      key: "s",
      label: "Search",
      children: [
        { key: "s", label: "Search notes (⌘K)", action: () => { onclose(); triggerCmdK(); } },
        { key: "g", label: "Search in graph", action: () => { goto("/graph"); onclose(); } },
      ],
    },
    {
      key: "g",
      label: "Go to",
      children: [
        { key: "g", label: "Graph view", action: () => { goto("/graph"); onclose(); } },
        { key: "t", label: "Timeline", action: () => { goto("/timeline"); onclose(); } },
        { key: "h", label: "Home", action: () => { goto("/"); onclose(); } },
      ],
    },
    {
      key: "d",
      label: "Daily",
      action: () => goToDaily(),
    },
    {
      key: "t",
      label: "Tasks",
      action: () => { goto("/p/task"); onclose(); },
    },
  ];

  currentLevel = rootMenu;

  async function goToDaily() {
    try {
      const note = await api.getDailyNote();
      goto(`/p/${encodeURIComponent(note.id)}`);
    } catch (e) {
      console.error("Failed to get daily note:", e);
    }
    onclose();
  }

  async function openNewNote() {
    // Close leader menu, open command palette
    onclose();
    triggerCmdK();
  }

  function triggerCmdK() {
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true }),
    );
  }

  function handleSelect(item: MenuItem) {
    if (item.children) {
      breadcrumb = [...breadcrumb, item.label];
      currentLevel = item.children;
      selectedIndex = 0;
    } else if (item.action) {
      item.action();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    // Direct key press — find matching item
    const match = currentLevel.find((m) => m.key === e.key);
    if (match) {
      e.preventDefault();
      handleSelect(match);
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(currentLevel.length - 1, selectedIndex + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && currentLevel[selectedIndex]) {
      e.preventDefault();
      handleSelect(currentLevel[selectedIndex]);
    } else if (e.key === "Escape" || e.key === "Backspace") {
      e.preventDefault();
      if (breadcrumb.length > 0) {
        breadcrumb = breadcrumb.slice(0, -1);
        // Walk back to parent
        let level = rootMenu;
        for (const name of breadcrumb) {
          const found = level.find((m) => m.label === name);
          if (found?.children) level = found.children;
        }
        currentLevel = level;
        selectedIndex = 0;
      } else {
        onclose();
      }
    }
  }

  onMount(() => {
    document.addEventListener("keydown", handleKeydown);
    return () => document.removeEventListener("keydown", handleKeydown);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50">
  <div class="absolute inset-0" onclick={onclose}></div>
  <div class="absolute left-1/2 top-[30%] -translate-x-1/2 w-full max-w-sm">
    <div class="rounded-lg border border-border bg-popover text-popover-foreground shadow-2xl">
      <div class="px-4 py-2 border-b border-border flex items-center gap-2">
        <span class="text-xs text-muted-foreground font-mono">SPACE</span>
        {#each breadcrumb as crumb}
          <span class="text-xs text-muted-foreground">›</span>
          <span class="text-xs text-muted-foreground">{crumb}</span>
        {/each}
      </div>
      <div class="p-1">
        {#each currentLevel as item, i (item.key)}
          <div
            class="flex items-center gap-3 rounded-md px-3 py-2 text-sm cursor-pointer {i === selectedIndex ? 'bg-accent text-accent-foreground' : ''}"
            onclick={() => handleSelect(item)}
            onmouseenter={() => (selectedIndex = i)}
          >
            <span class="font-mono text-xs w-5 text-center text-primary font-bold">{item.key}</span>
            <span>{item.label}</span>
            {#if item.children}
              <span class="ml-auto text-xs text-muted-foreground">›</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>

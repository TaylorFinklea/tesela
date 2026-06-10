<script lang="ts">
  import { onMount } from "svelte";
  import { scoreFuzzy, highlightRuns } from "$lib/fuzzy";
  // The LIVE recents store ('tesela:workspace:recent', fed by the focusPane
  // chokepoint). The legacy $lib/stores/recents.svelte store has had no
  // writers since the legacy-chrome deletion, so the recency boost was inert.
  import { getRecent } from "$lib/state/shared.svelte";

  export type AutocompleteItem = {
    id: string;
    label: string;
    secondary?: string;
  };

  let {
    items,
    filter,
    position,
    type,
    onselect,
    onselectInline,
    onclose,
  }: {
    items: AutocompleteItem[];
    filter: string;
    position: { x: number; y: number };
    /** The autocomplete kind (drives the footer hint). "tag" shows the
     *  ↵-chip / ⌘↵-inline gesture; others show a plain select hint. */
    type?: "tag" | "link" | "tagmanage" | "templatepick";
    onselect: (item: AutocompleteItem) => void;
    /** ⌘↵/Ctrl↵ accept — the "keep it inline" variant (Model A tag gesture).
     *  Falls back to `onselect` when not provided. */
    onselectInline?: (item: AutocompleteItem) => void;
    onclose: () => void;
  } = $props();

  let selectedIndex = $state(0);

  // Phase 9.8 — fuzzy match + recency tie-break. When `filter` is empty,
  // sort the full list by recency so the user's recent notes are at the top.
  type Scored = { item: AutocompleteItem; score: number; positions: number[] };

  const scored = $derived.by((): Scored[] => {
    const recents = getRecent();
    const recentRank = (id: string) => {
      const idx = recents.indexOf(id);
      return idx === -1 ? Infinity : idx;
    };
    if (!filter) {
      return items
        .map((item) => ({ item, score: 0, positions: [] as number[] }))
        .sort((a, b) => recentRank(a.item.id) - recentRank(b.item.id));
    }
    return items
      .map((item) => {
        const m = scoreFuzzy(item.label, filter);
        return { item, score: m.score, positions: m.positions };
      })
      .filter((x) => x.score > 0)
      .sort((a, b) => {
        if (a.score !== b.score) return b.score - a.score;
        return recentRank(a.item.id) - recentRank(b.item.id);
      });
  });

  $effect(() => {
    filter;
    selectedIndex = 0;
  });

  $effect(() => {
    if (selectedIndex >= scored.length) selectedIndex = Math.max(0, scored.length - 1);
  });

  export function handleKeydown(e: KeyboardEvent): boolean {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(scored.length - 1, selectedIndex + 1);
      return true;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
      return true;
    } else if ((e.key === "Enter" || e.key === "Tab") && scored[selectedIndex]) {
      e.preventDefault();
      // ⌘↵ / Ctrl↵ = the "keep inline" accept (Model A tag gesture); plain
      // ↵ / Tab = the default (commit to a chip). Tab always commits.
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && onselectInline) {
        onselectInline(scored[selectedIndex].item);
      } else {
        onselect(scored[selectedIndex].item);
      }
      return true;
    } else if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return true;
    }
    return false;
  }

  onMount(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".autocomplete-menu")) onclose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  });
</script>

{#if scored.length > 0}
  <div
    class="autocomplete-menu fixed z-50 rounded-md border border-border bg-popover text-popover-foreground shadow-lg w-56 max-h-56 overflow-hidden"
    style="left: {position.x}px; top: {position.y}px"
  >
    <div class="py-0.5 max-h-44 overflow-y-auto">
      {#each scored as { item, positions }, i (item.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ac-row flex items-center gap-2 px-2 py-1 text-[12px] cursor-pointer"
          class:is-selected={i === selectedIndex}
          onclick={() => onselect(item)}
          onmouseenter={() => (selectedIndex = i)}
        >
          <span class="truncate">
            {#if positions.length > 0}
              {#each highlightRuns(item.label, positions) as run, ri (ri)}
                {#if run.match}<strong class="text-primary font-semibold">{run.ch}</strong>{:else}<span>{run.ch}</span>{/if}
              {/each}
            {:else}
              {item.label}
            {/if}
          </span>
          {#if item.secondary}
            <span class="ml-auto text-[10px] text-muted-foreground/50 shrink-0">{item.secondary}</span>
          {/if}
        </div>
      {/each}
    </div>
    <div class="ac-hint">
      {#if type === "tag"}
        <span class="ac-kbd">↵</span> chip<span class="ac-sep">·</span><span class="ac-kbd">⌘↵</span> inline<span class="ac-sep">·</span><span class="ac-kbd">↑↓</span> move
      {:else}
        <span class="ac-kbd">↵</span> select<span class="ac-sep">·</span><span class="ac-kbd">↑↓</span> move
      {/if}
    </div>
  </div>
{/if}

<style>
  /* Visible selection — in /g the old `bg-accent` resolves to the popover bg
     (invisible). A coral tint + left rail reads clearly; var() fallbacks keep
     it visible outside the Graphite token scope too. */
  .ac-row.is-selected {
    background: var(--coral-dim, rgba(255, 107, 90, 0.14));
    box-shadow: inset 2px 0 0 var(--coral, #ff6b5a);
  }
  .ac-hint {
    display: flex;
    align-items: center;
    gap: 4px;
    border-top: 1px solid var(--line, rgba(255, 255, 255, 0.08));
    padding: 4px 8px;
    font-size: 9.5px;
    color: var(--faint, #8a909c);
    font-family: var(--mono, ui-monospace, monospace);
  }
  .ac-kbd {
    color: var(--muted, #aab0bb);
    background: var(--raised-2, rgba(255, 255, 255, 0.06));
    border-radius: 3px;
    padding: 0 3px;
  }
  .ac-sep {
    opacity: 0.45;
    margin: 0 1px;
  }
</style>

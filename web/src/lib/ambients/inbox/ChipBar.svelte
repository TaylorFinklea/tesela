<script lang="ts">
  import { CHIP_REGISTRY, type ChipState } from "./chips";

  let {
    state,
    availableTypes,
    onToggleStatic,
    onToggleType,
    onUnhidePage,
    onUnhideBlock,
    onEditRaw,
  }: {
    state: ChipState;
    /** Names of all known types, used to render the Types chip-group.
     * Sourced from `/types` by the parent so chips reflect what the
     * user's TypeRegistry actually contains. */
    availableTypes: string[];
    /** Toggle the static chip with id `chipId`. */
    onToggleStatic: (chipId: string) => void;
    /** Toggle inclusion of the given type name in the activeTypes set. */
    onToggleType: (typeName: string) => void;
    /** Remove a `-page:<pageId>` exclusion (the × on a hidden-page chip). */
    onUnhidePage: (pageId: string) => void;
    /** Remove a `-block:<blockId>` exclusion. */
    onUnhideBlock: (blockId: string) => void;
    /** Open the raw-DSL editor sheet. */
    onEditRaw: () => void;
  } = $props();

  // Registry order is canonical for static rendering — `defaultOn`
  // chips first so the "always-relevant" filters anchor the left.
  const orderedStatic = $derived(
    [...CHIP_REGISTRY].sort((a, b) => {
      if (a.defaultOn === b.defaultOn) return 0;
      return a.defaultOn ? -1 : 1;
    }),
  );

  // Activate set for O(1) lookup when rendering type chips.
  const activeTypeSet = $derived(new Set(state.activeTypes));
</script>

<div class="flex flex-col gap-1.5 mb-3 text-[11px]">
  <!-- Row 1: static chips + raw-edit button on the right -->
  <div class="flex flex-wrap gap-1.5 items-center">
    {#each orderedStatic as chip (chip.id)}
      {@const active = state.active[chip.id]}
      <button
        type="button"
        class={[
          "px-2 py-0.5 rounded-full border transition-colors",
          active
            ? "bg-accent border-accent text-foreground"
            : "border-muted-foreground/20 text-muted-foreground hover:border-muted-foreground/40",
        ].join(" ")}
        onclick={() => onToggleStatic(chip.id)}
        title={chip.hint}
      >{chip.glyph} {chip.label}</button>
    {/each}

    {#each state.unknownClauses as clause (clause)}
      <!-- Unknown clauses (DSL fragments not claimed by any chip-shape)
           render read-only — the user can still see every filter that's
           active on the underlying query. Edit via the raw-DSL sheet. -->
      <span
        class="px-2 py-0.5 rounded-full border border-dashed border-muted-foreground/30 text-muted-foreground/70 font-mono text-[10px]"
        title="Raw clause — edit via </>"
      >{clause}</span>
    {/each}

    <button
      type="button"
      class="ml-auto px-2 py-0.5 rounded border border-muted-foreground/20 text-muted-foreground hover:text-foreground hover:border-muted-foreground/40 transition-colors font-mono text-[10px]"
      onclick={onEditRaw}
      title="Edit raw DSL"
    >{"<"}/{">"} Edit query</button>
  </div>

  <!-- Row 2: Types chip-group — only renders when there are types to
       show. Multi-select composes a single tag-in: clause. -->
  {#if availableTypes.length > 0}
    <div class="flex flex-wrap gap-1.5 items-center">
      <span class="text-muted-foreground/50 text-[10px] uppercase tracking-wide mr-1">Types:</span>
      {#each availableTypes as typeName (typeName)}
        {@const active = activeTypeSet.has(typeName)}
        <button
          type="button"
          class={[
            "px-2 py-0.5 rounded-full border transition-colors text-[11px]",
            active
              ? "bg-accent border-accent text-foreground"
              : "border-muted-foreground/15 text-muted-foreground/80 hover:border-muted-foreground/40",
          ].join(" ")}
          onclick={() => onToggleType(typeName)}
          title={active
            ? `Click to remove ${typeName} from include set`
            : `Click to include blocks tagged ${typeName}`}
        >{typeName}</button>
      {/each}
    </div>
  {/if}

  <!-- Row 3: hidden-page / hidden-block exclusions. Each renders with
       an × to un-hide. Skipped when empty so we don't reserve space. -->
  {#if state.hiddenPages.length > 0 || state.hiddenBlocks.length > 0}
    <div class="flex flex-wrap gap-1.5 items-center">
      <span class="text-muted-foreground/50 text-[10px] uppercase tracking-wide mr-1">Hidden:</span>
      {#each state.hiddenPages as pageId (`page:${pageId}`)}
        <button
          type="button"
          class="px-2 py-0.5 rounded-full border border-dashed border-muted-foreground/30 text-muted-foreground hover:text-foreground hover:border-muted-foreground/50 transition-colors font-mono text-[10px] flex items-center gap-1"
          onclick={() => onUnhidePage(pageId)}
          title={`Un-hide page ${pageId}`}
        >
          <span>📄 {pageId}</span>
          <span class="text-muted-foreground/60">×</span>
        </button>
      {/each}
      {#each state.hiddenBlocks as blockId (`block:${blockId}`)}
        <button
          type="button"
          class="px-2 py-0.5 rounded-full border border-dashed border-muted-foreground/30 text-muted-foreground hover:text-foreground hover:border-muted-foreground/50 transition-colors font-mono text-[10px] flex items-center gap-1"
          onclick={() => onUnhideBlock(blockId)}
          title={`Un-hide block ${blockId}`}
        >
          <span>· {blockId}</span>
          <span class="text-muted-foreground/60">×</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

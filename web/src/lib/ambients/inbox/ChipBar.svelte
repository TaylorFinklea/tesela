<script lang="ts">
  import { CHIP_REGISTRY, type ChipState } from "./chips";

  let {
    state,
    onToggle,
    onEditRaw,
  }: {
    state: ChipState;
    /** Toggle the chip with id `chipId`. Parent re-derives DSL + persists. */
    onToggle: (chipId: string) => void;
    /** Open the raw-DSL editor. Hands off to the parent. */
    onEditRaw: () => void;
  } = $props();

  // Registry order is canonical for rendering — `defaultOn` chips first,
  // then the rest, within their original definition order. Keeps the
  // "always-relevant" filters anchored to the left of the bar.
  const orderedChips = $derived(
    [...CHIP_REGISTRY].sort((a, b) => {
      if (a.defaultOn === b.defaultOn) return 0;
      return a.defaultOn ? -1 : 1;
    }),
  );
</script>

<div class="flex flex-wrap gap-1.5 items-center mb-3 text-[11px]">
  {#each orderedChips as chip (chip.id)}
    {@const active = state.active[chip.id]}
    <button
      type="button"
      class={[
        "px-2 py-0.5 rounded-full border transition-colors",
        active
          ? "bg-accent border-accent text-foreground"
          : "border-muted-foreground/20 text-muted-foreground hover:border-muted-foreground/40",
      ].join(" ")}
      onclick={() => onToggle(chip.id)}
      title={chip.hint}
    >{chip.glyph} {chip.label}</button>
  {/each}

  {#each state.unknownClauses as clause (clause)}
    <!-- Unknown clauses (DSL fragments not claimed by any chip) render
         read-only so the user can see every filter that's active on
         the underlying query. Click the "</>" button to edit them. -->
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

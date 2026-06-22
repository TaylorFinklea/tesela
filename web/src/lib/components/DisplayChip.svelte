<script lang="ts">
  /**
   * Phase 10.6 — generalized chip renderer for properties pinned via a
   * tag's `display_chips` array. Visualization is fully driven by the
   * Property page's frontmatter (`chip_icon`, `chip_label_mode`,
   * `chip_value_format`, …) so the same property looks the same wherever
   * it surfaces. See `recursive-rolling-fountain.md` for the schema.
   *
   * Phase 12.2 — when `propKey === "recurring"`, value is formatted via
   * `formatRecurrence` and clicking the chip opens a minimal skip menu.
   */
  import type { PropertyDefinition } from "$lib/property-registry";
  import { resolveChipIcon } from "$lib/icon-registry";
  import { formatRecurrence } from "$lib/recurrence-format";
  import { skipRecurrence } from "$lib/recurrence-actions";
  import { formatDateMonthDay } from "$lib/date-format";

  let {
    propKey,
    value,
    def,
    blockId = null,
  }: {
    propKey: string;
    value: string;
    def: PropertyDefinition;
    blockId?: string | null;
  } = $props();

  /** Whether this is the recurring chip (drives formatting + skip affordance). */
  const isRecurring = $derived(propKey === "recurring");

  /** Popover open state for the skip menu. */
  let skipMenuOpen = $state(false);

  function handleChipClick(e: MouseEvent) {
    if (!isRecurring) return;
    e.stopPropagation();
    skipMenuOpen = !skipMenuOpen;
  }

  function handleSkip(e: MouseEvent) {
    e.stopPropagation();
    skipMenuOpen = false;
    if (blockId) skipRecurrence(blockId);
  }

  function handleClickOutside(e: MouseEvent) {
    if (skipMenuOpen) {
      skipMenuOpen = false;
    }
  }

  /** Effective label mode: explicit setting > derived ("icon" if icon set, else "full"). */
  const labelMode = $derived(
    def.chip_label_mode ?? (def.chip_icon ? "icon" : "full"),
  );

  /** Effective value format: explicit setting > type default. */
  const valueFormat = $derived(def.chip_value_format ?? defaultValueFormat(def.value_type));

  const icon = $derived(resolveChipIcon(def.chip_icon));

  /**
   * Per-type value-formatter default. Date → month-day so "[[2026-05-13]]"
   * becomes "May 13"; everything else just shows the raw value (truncated).
   */
  function defaultValueFormat(type: string): string {
    if (type === "date") return "month-day";
    return "value";
  }

  /**
   * Map a select value to a 3-segment bar string by its rank in `choices`.
   * Choices are read low-to-high (e.g. `["low", "medium", "high"]`), so
   * `high` → ▰▰▰, `medium` → ▰▰▱, `low` → ▰▱▱. Off-list values render
   * as a single ▰ rather than crashing.
   */
  function formatBars(v: string, choices: string[]): string {
    const idx = choices.findIndex((c) => c.toLowerCase() === v.trim().toLowerCase());
    const total = choices.length || 1;
    const rank = idx < 0 ? 1 : idx + 1;
    const filledSegments = Math.max(1, Math.round((rank / total) * 3));
    return "▰".repeat(filledSegments) + "▱".repeat(3 - filledSegments);
  }

  function formatTruncate(v: string, max: number): string {
    return v.length > max ? v.slice(0, max - 1) + "…" : v;
  }

  const formattedValue = $derived.by((): string => {
    const v = (value ?? "").trim();
    // Recurring chips always route through the recurrence formatter regardless
    // of chip_value_format so users see "Daily, 10×" instead of raw grammar.
    if (isRecurring) return formatRecurrence(v);
    switch (valueFormat) {
      case "month-day": return formatDateMonthDay(v);
      case "iso": return v.replace(/^\[\[|\]\]$/g, "");
      case "bars":
        if (def.value_type !== "select" && def.value_type !== "multi-select") return formatTruncate(v, 24);
        return formatBars(v, def.choices);
      case "truncate": return formatTruncate(v, 10);
      case "recurrence": return formatRecurrence(v);
      default: return formatTruncate(v, 24);
    }
  });

  const labelText = $derived.by((): string | null => {
    if (labelMode === "none" || labelMode === "icon") return null;
    if (labelMode === "short") return def.chip_short_label ?? def.name.slice(0, 4);
    return def.name; // "full"
  });

  /**
   * Phase 4 — per-choice color. For a select / multi-select value with a
   * `choice_colors` entry (Property-page frontmatter, keyed by lowercased
   * choice), tint the chip with that color. We mix it into a translucent
   * background + a saturated foreground via `color-mix`, mirroring the tag-chip
   * recipe (cm-decorations `.cm-tesela-tag-chip`) so it stays readable in both
   * the warm-dark and light themes. `null` → fall back to the default muted
   * chip classes. The recurring chip keeps its own affordance and is never
   * tinted. A multi-select value can be a `, `-joined list — color by the FIRST
   * matching choice so the chip still reads as one pill.
   */
  const choiceColor = $derived.by((): string | null => {
    if (isRecurring) return null;
    if (def.value_type !== "select" && def.value_type !== "multi-select") return null;
    const colors = def.choice_colors;
    if (!colors || Object.keys(colors).length === 0) return null;
    const raw = (value ?? "").trim();
    if (!raw) return null;
    const parts = def.value_type === "multi-select" ? raw.split(",").map((p) => p.trim()) : [raw];
    for (const p of parts) {
      const hit = colors[p.toLowerCase()];
      if (hit) return hit;
    }
    return null;
  });

  const chipStyle = $derived(
    choiceColor
      ? `background: color-mix(in srgb, ${choiceColor} 16%, transparent); ` +
        `color: color-mix(in srgb, ${choiceColor} 78%, var(--foreground)); ` +
        `border: 1px solid color-mix(in srgb, ${choiceColor} 32%, transparent);`
      : "",
  );
</script>

<svelte:window onclick={handleClickOutside} />

<span class="relative">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full font-medium {choiceColor ? '' : 'bg-muted/40 text-muted-foreground/90'} {isRecurring && blockId ? 'cursor-pointer hover:bg-muted/70' : ''}"
    style={chipStyle}
    title="{def.name}: {value}"
    onclick={handleChipClick}
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleChipClick(e as unknown as MouseEvent); }}
    role={isRecurring && blockId ? "button" : undefined}
    tabindex={isRecurring && blockId ? 0 : undefined}
  >
    {#if labelMode === "icon" && (icon.component || icon.emoji)}
      {#if icon.component}
        {@const Cmp = icon.component as import("svelte").Component<{ size?: number; stroke?: number }>}
        <Cmp size={11} stroke={1.75} />
      {:else}
        <span class="leading-none">{icon.emoji}</span>
      {/if}
    {:else if labelText}
      <span class="text-muted-foreground/50">{labelText}</span>
    {/if}
    <span>{formattedValue}</span>
  </span>

  {#if skipMenuOpen && isRecurring && blockId}
    <div
      class="absolute top-full left-0 mt-1 z-50 min-w-max rounded-md border border-border bg-popover shadow-md py-1"
      role="menu"
    >
      <button
        class="flex w-full items-center gap-2 px-3 py-1.5 text-xs hover:bg-muted/60 text-popover-foreground transition-colors"
        onclick={handleSkip}
        role="menuitem"
      >
        ⏭ Skip to next occurrence
      </button>
    </div>
  {/if}
</span>

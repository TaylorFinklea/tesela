<script lang="ts">
  /**
   * Phase 10.6 — generalized chip renderer for properties pinned via a
   * tag's `display_chips` array. Visualization is fully driven by the
   * Property page's frontmatter (`chip_icon`, `chip_label_mode`,
   * `chip_value_format`, …) so the same property looks the same wherever
   * it surfaces. See `recursive-rolling-fountain.md` for the schema.
   */
  import type { PropertyDefinition } from "$lib/property-registry";
  import { resolveChipIcon } from "$lib/icon-registry";

  let {
    propKey,
    value,
    def,
  }: {
    propKey: string;
    value: string;
    def: PropertyDefinition;
  } = $props();

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

  function formatDateMonthDay(v: string): string {
    const m = v.trim().match(/^\[\[(\d{4})-(\d{2})-(\d{2})\]\]$/) ||
              v.trim().match(/^(\d{4})-(\d{2})-(\d{2})$/);
    if (!m) return v.trim();
    const [, y, mo, d] = m;
    const date = new Date(Number(y), Number(mo) - 1, Number(d));
    const month = date.toLocaleString("en-US", { month: "short" });
    const day = Number(d);
    const thisYear = new Date().getFullYear();
    return Number(y) === thisYear ? `${month} ${day}` : `${month} ${day}, ${y}`;
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
    switch (valueFormat) {
      case "month-day": return formatDateMonthDay(v);
      case "iso": return v.replace(/^\[\[|\]\]$/g, "");
      case "bars":
        if (def.value_type !== "select" && def.value_type !== "multi-select") return formatTruncate(v, 24);
        return formatBars(v, def.choices);
      case "truncate": return formatTruncate(v, 10);
      default: return formatTruncate(v, 24);
    }
  });

  const labelText = $derived.by((): string | null => {
    if (labelMode === "none" || labelMode === "icon") return null;
    if (labelMode === "short") return def.chip_short_label ?? def.name.slice(0, 4);
    return def.name; // "full"
  });
</script>

<span
  class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full bg-muted/40 text-muted-foreground/90 font-medium"
  title="{def.name}: {value}"
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

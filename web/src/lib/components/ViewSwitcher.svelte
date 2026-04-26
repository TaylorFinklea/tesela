<script lang="ts" generics="T extends string">
  // Use a representative Tabler icon for the type reference. All Tabler icons
  // share this shape; using `typeof IconTable` is more permissive than the
  // Svelte 5 `Component<>` generic and matches how callers already type their
  // VIEW_META arrays.
  import { IconTable } from "@tabler/icons-svelte";

  type ViewSpec = {
    id: T;
    label: string;
    Icon: typeof IconTable;
  };

  let {
    views,
    active,
    onChange,
    size = 12,
  }: {
    views: ViewSpec[];
    active: T;
    onChange: (id: T) => void;
    /** Icon size in px (default 12). */
    size?: number;
  } = $props();
</script>

<div class="flex items-center gap-0.5 bg-muted/40 rounded-md p-0.5 shrink-0">
  {#each views as v}
    {@const isActive = v.id === active}
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="p-1 rounded transition-all {isActive ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-foreground/70'}"
      onclick={() => onChange(v.id)}
      title={v.label}
    >
      <v.Icon {size} stroke={1.5} />
    </button>
  {/each}
</div>

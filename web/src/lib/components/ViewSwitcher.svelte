<script lang="ts">
  import { IconTable, IconLayoutKanban } from "@tabler/icons-svelte";

  let { mode, onchange }: { mode: "table" | "kanban"; onchange: (mode: "table" | "kanban") => void } = $props();
</script>

<div class="flex items-center gap-0.5 rounded-md border border-border/40 p-0.5">
  <button
    onclick={() => onchange("table")}
    class="p-1 rounded transition-colors {mode === 'table' ? 'bg-primary/10 text-primary' : 'text-muted-foreground/50 hover:text-foreground/70'}"
    title="Table view"
  >
    <IconTable size={14} />
  </button>
  <button
    onclick={() => onchange("kanban")}
    class="p-1 rounded transition-colors {mode === 'kanban' ? 'bg-primary/10 text-primary' : 'text-muted-foreground/50 hover:text-foreground/70'}"
    title="Kanban view"
  >
    <IconLayoutKanban size={14} />
  </button>
</div>

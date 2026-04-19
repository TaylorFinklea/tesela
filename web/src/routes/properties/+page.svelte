<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { PROPERTY_TYPE_LABELS } from "$lib/property-registry";
  import type { PropertyType } from "$lib/property-registry";

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));

  const propertyPages = $derived(
    ((notesQuery.data ?? []) as Note[])
      .filter((n) => n.metadata.note_type === "Property")
      .sort((a, b) => a.title.localeCompare(b.title)),
  );
</script>

<div class="flex-1 flex flex-col">
  <header class="border-b border-border px-8 h-14 flex items-center shrink-0">
    <h1 class="font-display text-xl font-semibold tracking-tight">Properties</h1>
    <span class="ml-3 text-[11px] text-muted-foreground/60">{propertyPages.length} defined</span>
  </header>

  <section class="flex-1 overflow-y-auto">
    {#if notesQuery.isLoading}
      <div class="px-8 py-8 text-sm text-muted-foreground">Loading…</div>
    {:else if propertyPages.length === 0}
      <div class="px-8 py-12 text-sm text-muted-foreground/60 italic">
        No properties yet. Add properties to a tag page to define them.
      </div>
    {:else}
      <table class="w-full text-[13px]">
        <thead>
          <tr class="border-b border-border">
            <th class="text-left px-8 py-3 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Name</th>
            <th class="text-left px-4 py-3 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Type</th>
            <th class="text-left px-4 py-3 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Options</th>
          </tr>
        </thead>
        <tbody>
          {#each propertyPages as prop}
            {@const valueType = (prop.metadata.custom.value_type as PropertyType) ?? "text"}
            {@const choices = Array.isArray(prop.metadata.custom.choices) ? (prop.metadata.custom.choices as string[]) : []}
            <tr class="border-b border-border/30 hover:bg-accent/20 transition-colors">
              <td class="px-8 py-2.5">
                <a href="/p/{encodeURIComponent(prop.id)}" class="font-medium hover:text-primary transition-colors">
                  {prop.title}
                </a>
              </td>
              <td class="px-4 py-2.5 text-muted-foreground">
                {PROPERTY_TYPE_LABELS[valueType] ?? valueType}
              </td>
              <td class="px-4 py-2.5">
                {#if choices.length > 0}
                  <div class="flex flex-wrap gap-1">
                    {#each choices as choice}
                      <span class="text-[10px] px-1.5 py-px rounded-full bg-muted/60 text-muted-foreground">{choice}</span>
                    {/each}
                  </div>
                {:else}
                  <span class="text-muted-foreground/40 text-[11px]">—</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
</div>

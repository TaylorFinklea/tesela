<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import type { Note } from "$lib/types/Note";
  import {
    buildRegistry,
    parseHiddenChoices,
    updateFrontmatterKey,
    removeFrontmatterKey,
    serializeStringArray,
  } from "$lib/property-registry";

  let { tagName, noteId }: { tagName: string; noteId: string } = $props();

  const queryClient = useQueryClient();

  const typeQuery = createQuery(() => ({
    queryKey: ["type", tagName] as const,
    queryFn: () => api.getType(tagName),
  }));

  const typeDef: TypeDefinition | undefined = $derived(typeQuery.data as TypeDefinition | undefined);
  const properties = $derived(typeDef?.properties ?? []);

  let addingProperty = $state(false);
  let newPropertyName = $state("");

  // Available property pages for autocomplete
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));

  const allNotes = $derived((allNotesQuery.data ?? []) as Note[]);
  const propertyPages = $derived(
    allNotes.filter((n) => n.metadata.note_type === "Property").map((n) => n.title),
  );
  const propertyRegistry = $derived(buildRegistry(allNotes));

  const tagNoteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
  }));

  const tagNote = $derived(tagNoteQuery.data as Note | undefined);
  const hiddenChoices = $derived(
    tagNote ? parseHiddenChoices(tagNote.metadata.custom as Record<string, unknown>) : {},
  );

  let expandedProp = $state<string | null>(null);

  async function toggleHiddenChoice(propName: string, choice: string, isHidden: boolean) {
    const note = await api.getNote(noteId);
    const key = `hidden_${propName}`;
    const current: string[] = Array.isArray(note.metadata.custom[key])
      ? (note.metadata.custom[key] as string[])
      : [];
    const updated = isHidden ? current.filter((c) => c !== choice) : [...current, choice];
    const newContent = updated.length
      ? updateFrontmatterKey(note.content, key, serializeStringArray(updated))
      : removeFrontmatterKey(note.content, key);
    await api.updateNote(noteId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

  async function removeProperty(propName: string) {
    try {
      const note = await api.getNote(noteId);
      const content = note.content;
      // Parse tag_properties from frontmatter and remove the property
      const updated = content.replace(
        /tag_properties: \[([^\]]*)\]/,
        (_match, inner: string) => {
          const props = inner
            .split(",")
            .map((s) => s.trim().replace(/"/g, ""))
            .filter((s) => s && s.toLowerCase() !== propName.toLowerCase());
          return `tag_properties: [${props.map((p) => `"${p}"`).join(", ")}]`;
        },
      );
      await api.updateNote(noteId, updated);
      queryClient.invalidateQueries({ queryKey: ["type", tagName] });
      queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    } catch (e) {
      console.error("Failed to remove property:", e);
    }
  }

  async function addProperty() {
    if (!newPropertyName.trim()) return;
    try {
      const note = await api.getNote(noteId);
      const content = note.content;
      const name = newPropertyName.trim();

      // Add to tag_properties array in frontmatter
      const updated = content.replace(
        /tag_properties: \[([^\]]*)\]/,
        (_match, inner: string) => {
          const props = inner
            .split(",")
            .map((s) => s.trim().replace(/"/g, ""))
            .filter(Boolean);
          if (!props.some((p) => p.toLowerCase() === name.toLowerCase())) {
            props.push(name);
          }
          return `tag_properties: [${props.map((p) => `"${p}"`).join(", ")}]`;
        },
      );

      await api.updateNote(noteId, updated);

      // Create the Property page if it doesn't exist
      const propertyPageId = name.toLowerCase();
      try {
        await api.getNote(propertyPageId);
      } catch {
        // Page doesn't exist — create it
        const propertyContent = `---\ntitle: "${name}"\ntype: "Property"\nvalue_type: "text"\ntags: []\n---\n- ${name} property.\n`;
        await api.createNote(name, propertyContent);
      }

      queryClient.invalidateQueries({ queryKey: ["type", tagName] });
      queryClient.invalidateQueries({ queryKey: ["note", noteId] });
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      newPropertyName = "";
      addingProperty = false;
    } catch (e) {
      console.error("Failed to add property:", e);
    }
  }
</script>

<div class="space-y-2">
  <div class="flex items-center justify-between">
    <h3 class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Properties</h3>
    <button
      class="text-[10px] text-muted-foreground/50 hover:text-foreground transition-colors"
      onclick={() => (addingProperty = !addingProperty)}
    >
      {addingProperty ? "Cancel" : "+ Add"}
    </button>
  </div>

  {#if addingProperty}
    <div class="flex gap-1">
      <input
        type="text"
        placeholder="Property name…"
        bind:value={newPropertyName}
        onkeydown={(e) => { if (e.key === "Enter") addProperty(); if (e.key === "Escape") { addingProperty = false; newPropertyName = ""; } }}
        class="flex-1 text-[12px] bg-muted/50 rounded px-2 py-1 outline-none border border-transparent focus:border-ring/30"
        autofocus
        list="property-suggestions"
      />
      <datalist id="property-suggestions">
        {#each propertyPages as pp}
          <option value={pp}></option>
        {/each}
      </datalist>
      <button
        class="text-[11px] px-2 py-1 rounded bg-accent hover:bg-accent/80 text-accent-foreground transition-colors"
        onclick={addProperty}
      >
        Add
      </button>
    </div>
  {/if}

  {#if properties.length === 0}
    <div class="text-[11px] text-muted-foreground/40 italic">No properties defined</div>
  {:else}
    <div class="space-y-0.5">
      {#each properties as prop}
        {@const def = propertyRegistry.get(prop.name.toLowerCase())}
        {@const hasChoices = def && (def.value_type === "select" || def.value_type === "multi-select") && def.choices.length > 0}
        {@const isExpanded = expandedProp === prop.name}
        {@const hidden = hiddenChoices[prop.name] ?? hiddenChoices[prop.name.toLowerCase()] ?? []}
        <div class="rounded hover:bg-accent/30 group transition-colors">
          <div class="flex items-center justify-between px-2 py-1">
            <div class="flex items-center gap-2">
              <span class="text-[12px]">{prop.name}</span>
              <span class="text-[10px] text-muted-foreground/40">{def?.value_type ?? prop.value_type}</span>
              {#if def && def.choices.length > 0}
                <span class="text-[10px] text-muted-foreground/30">{def.choices.length} choices</span>
              {/if}
            </div>
            <div class="flex items-center gap-1">
              {#if hasChoices}
                <button
                  class="text-[10px] text-muted-foreground/40 hover:text-foreground transition-colors px-1"
                  title="Hide options for this tag"
                  onclick={() => { expandedProp = isExpanded ? null : prop.name; }}
                >
                  {isExpanded ? "▴" : "▾"}{hidden.length > 0 ? ` ${hidden.length} hidden` : ""}
                </button>
              {/if}
              <button
                class="text-[10px] text-muted-foreground/30 hover:text-destructive opacity-0 group-hover:opacity-100 transition-opacity"
                onclick={() => removeProperty(prop.name)}
              >
                ×
              </button>
            </div>
          </div>
          {#if isExpanded && hasChoices && def}
            <div class="px-4 pb-2 space-y-0.5">
              <div class="text-[10px] text-muted-foreground/50 mb-1">Hide choices for #{tagName}:</div>
              {#each def.choices as choice}
                {@const isHidden = hidden.some((h) => h.toLowerCase() === choice.toLowerCase())}
                <label class="flex items-center gap-2 cursor-pointer group/choice">
                  <input
                    type="checkbox"
                    checked={isHidden}
                    class="w-3 h-3 accent-primary"
                    onchange={() => toggleHiddenChoice(prop.name, choice, isHidden)}
                  />
                  <span class="text-[11px] {isHidden ? 'line-through text-muted-foreground/40' : ''}">{choice}</span>
                </label>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

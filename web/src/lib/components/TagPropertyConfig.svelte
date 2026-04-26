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

  // Tag inheritance
  const extendsTagName = $derived.by((): string => {
    const ext = tagNote?.metadata.custom.extends;
    return typeof ext === "string" ? ext.trim() : "";
  });

  let editingExtends = $state(false);
  let extendsInput = $state("");

  const parentTypeQuery = createQuery(() => ({
    queryKey: ["type", extendsTagName] as const,
    queryFn: () => api.getType(extendsTagName),
    enabled: extendsTagName !== "",
  }));

  const parentProperties = $derived.by(() => {
    if (!extendsTagName) return [];
    const data = parentTypeQuery.data as TypeDefinition | undefined;
    return data?.properties ?? [];
  });

  const inheritedProperties = $derived.by(() => {
    const ownNames = new Set(properties.map((p) => p.name.toLowerCase()));
    return parentProperties.filter((p) => !ownNames.has(p.name.toLowerCase()));
  });

  async function saveExtends(value: string) {
    editingExtends = false;
    const note = await api.getNote(noteId);
    const newContent = value.trim()
      ? updateFrontmatterKey(note.content, "extends", `"${value.trim()}"`)
      : removeFrontmatterKey(note.content, "extends");
    await api.updateNote(noteId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", noteId] });
  }

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

  /**
   * Toggle a visibility flag (hide_by_default or hide_empty) on the Property
   * page's frontmatter. Affects how the property renders on every block that
   * has a tag using this property.
   */
  async function togglePropertyVisibilityFlag(
    propName: string,
    flag: "hide_by_default" | "hide_empty",
    nextValue: boolean,
  ) {
    const propPageId = propName.toLowerCase();
    let propNote;
    try {
      propNote = await api.getNote(propPageId);
    } catch {
      // Property page doesn't exist yet — skip silently. The property is
      // only configured if the page exists; toggles are no-ops otherwise.
      return;
    }
    const newContent = updateFrontmatterKey(propNote.content, flag, String(nextValue));
    await api.updateNote(propPageId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", propPageId] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
    queryClient.invalidateQueries({ queryKey: ["type", tagName] });
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
  <!-- Extends row -->
  <div class="flex items-center gap-2 text-[11px] text-muted-foreground/50 mb-1">
    <span class="text-[10px] uppercase tracking-widest font-medium text-muted-foreground/40">Extends</span>
    {#if editingExtends}
      <input
        class="flex-1 text-[11px] bg-muted/50 rounded px-2 py-0.5 outline-none border border-ring/30"
        placeholder="Parent tag name…"
        bind:value={extendsInput}
        autofocus
        onkeydown={(e) => { if (e.key === "Enter") saveExtends(extendsInput); if (e.key === "Escape") editingExtends = false; }}
        onblur={() => saveExtends(extendsInput)}
      />
    {:else}
      <button
        class="flex-1 text-left text-[11px] px-1 -mx-1 rounded hover:bg-muted/50 transition-colors {extendsTagName ? 'text-primary/70' : 'text-muted-foreground/30 italic'}"
        onclick={() => { extendsInput = extendsTagName; editingExtends = true; }}
      >
        {extendsTagName || "None"}
      </button>
    {/if}
  </div>

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
        {@const hideByDefault = def?.hide_by_default ?? false}
        {@const hideEmpty = def?.hide_empty ?? true}
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
              <button
                class="text-[10px] text-muted-foreground/40 hover:text-foreground transition-colors px-1"
                title="Configure visibility & choices"
                onclick={() => { expandedProp = isExpanded ? null : prop.name; }}
              >
                {isExpanded ? "▴" : "▾"}{hasChoices && hidden.length > 0 ? ` ${hidden.length} hidden` : ""}
              </button>
              <button
                class="text-[10px] text-muted-foreground/30 hover:text-destructive opacity-0 group-hover:opacity-100 transition-opacity"
                onclick={() => removeProperty(prop.name)}
              >
                ×
              </button>
            </div>
          </div>
          {#if isExpanded}
            <div class="px-4 pb-2 space-y-2">
              <!-- Visibility toggles (per-property, applies wherever the property is used) -->
              <div class="space-y-1">
                <div class="text-[10px] text-muted-foreground/50">Block visibility</div>
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={hideByDefault}
                    class="w-3 h-3 accent-primary"
                    onchange={() => togglePropertyVisibilityFlag(prop.name, "hide_by_default", !hideByDefault)}
                  />
                  <span class="text-[11px]">Hide by default</span>
                  <span class="text-[10px] text-muted-foreground/40">— always hidden until block is expanded</span>
                </label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={hideEmpty}
                    class="w-3 h-3 accent-primary"
                    onchange={() => togglePropertyVisibilityFlag(prop.name, "hide_empty", !hideEmpty)}
                  />
                  <span class="text-[11px]">Hide empty value</span>
                  <span class="text-[10px] text-muted-foreground/40">— hidden when no value set</span>
                </label>
              </div>

              <!-- Choices (only for select-type properties) -->
              {#if hasChoices && def}
                <div class="space-y-0.5 pt-1 border-t border-border/30">
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
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Inherited properties from parent tag -->
  {#if inheritedProperties.length > 0}
    <div class="mt-2 pt-2 border-t border-border/30">
      <div class="text-[10px] text-muted-foreground/40 uppercase tracking-widest mb-1.5">
        Inherited from #{extendsTagName}
      </div>
      <div class="space-y-0.5 opacity-60">
        {#each inheritedProperties as prop}
          {@const def = propertyRegistry.get(prop.name.toLowerCase())}
          <div class="flex items-center gap-2 px-2 py-1 rounded text-muted-foreground/60">
            <span class="text-[12px]">{prop.name}</span>
            <span class="text-[10px] text-muted-foreground/30">{def?.value_type ?? prop.value_type}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

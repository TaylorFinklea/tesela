<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import type { Note } from "$lib/types/Note";
  import {
    buildRegistry,
    parseHiddenChoices,
    parsePropertyOverridesRaw,
    serializePropertyOverrides,
    updateFrontmatterKey,
    removeFrontmatterKey,
    serializeStringArray,
    serializeChoiceColors,
    type RawPropOverride,
  } from "$lib/property-registry";
  import type { Visibility } from "$lib/types/Visibility";
  import { TABLER_ICONS, resolveChipIcon } from "$lib/icon-registry";

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

  // Phase 3 — RAW per-type override map, read straight off the Tag page
  // frontmatter (NOT the flattened/resolved PropertyDef). This is what lets the
  // editor distinguish overridden-vs-inherited and never bake an inherited
  // value into an override (spec §3.5 tail).
  const rawOverrides = $derived(
    tagNote
      ? parsePropertyOverridesRaw(tagNote.metadata.custom as Record<string, unknown>)
      : {},
  );

  // Find the raw override entry for a property by case-insensitive name. Returns
  // both the key as-written (so a write preserves the existing casing) and the
  // entry. `key` is null when no override exists for this property yet.
  function rawOverrideFor(propName: string): { key: string | null; entry: RawPropOverride } {
    const lower = propName.toLowerCase();
    for (const [k, v] of Object.entries(rawOverrides)) {
      if (k.toLowerCase() === lower) return { key: k, entry: v };
    }
    return { key: null, entry: {} };
  }

  // Type metadata: icon + plural, read raw from the Tag page frontmatter.
  const tagIcon = $derived.by((): string => {
    const v = tagNote?.metadata.custom.icon;
    return typeof v === "string" ? v : "";
  });
  const tagPlural = $derived.by((): string => {
    const v = tagNote?.metadata.custom.plural;
    return typeof v === "string" ? v : "";
  });

  const resolvedTagIcon = $derived(resolveChipIcon(tagIcon || null));

  const iconNames = Object.keys(TABLER_ICONS).sort();
  let iconPickerOpen = $state(false);
  let iconSearch = $state("");
  const filteredIcons = $derived(
    iconSearch.trim()
      ? iconNames.filter((n) => n.includes(iconSearch.trim().toLowerCase()))
      : iconNames,
  );

  let pluralInput = $state("");
  let editingPlural = $state(false);

  /**
   * Persist the whole `property_overrides` map to the Tag page frontmatter.
   * `mutate(map)` mutates a fresh copy of the current raw map (case-preserving),
   * then we serialize to single-line FLOW YAML / JSON via the existing
   * updateFrontmatterKey pipeline — or remove the key when the map is empty so we
   * never write `property_overrides: {}`. Does NOT clobber other frontmatter keys.
   */
  async function writeOverrides(mutate: (map: Record<string, RawPropOverride>) => void) {
    const note = await api.getNote(noteId);
    const current = parsePropertyOverridesRaw(note.metadata.custom as Record<string, unknown>);
    // Deep-ish clone so the mutator can't disturb the derived state.
    const next: Record<string, RawPropOverride> = {};
    for (const [k, v] of Object.entries(current)) {
      next[k] = {
        ...v,
        ...(v.choices ? { choices: [...v.choices] } : {}),
        ...(v.hide_choices ? { hide_choices: [...v.hide_choices] } : {}),
      };
    }
    mutate(next);
    const serialized = serializePropertyOverrides(next);
    const newContent = serialized
      ? updateFrontmatterKey(note.content, "property_overrides", serialized)
      : removeFrontmatterKey(note.content, "property_overrides");
    await api.updateNote(noteId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    queryClient.invalidateQueries({ queryKey: ["type", tagName] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

  // Mutate one property's override entry (created on demand, keyed by the
  // existing case or the canonical property name for a fresh entry).
  function setOverrideField(
    map: Record<string, RawPropOverride>,
    propName: string,
    apply: (e: RawPropOverride) => void,
  ) {
    const lower = propName.toLowerCase();
    let key: string | null = null;
    for (const k of Object.keys(map)) {
      if (k.toLowerCase() === lower) {
        key = k;
        break;
      }
    }
    if (key === null) {
      key = propName;
      map[key] = {};
    }
    apply(map[key]);
  }

  // --- Choices override editor ---
  let newChoiceInput = $state<Record<string, string>>({});

  async function addOverrideChoice(propName: string) {
    const val = (newChoiceInput[propName] ?? "").trim();
    if (!val) return;
    await writeOverrides((map) => {
      setOverrideField(map, propName, (e) => {
        const list = e.choices ? [...e.choices] : [];
        if (!list.some((c) => c.toLowerCase() === val.toLowerCase())) list.push(val);
        e.choices = list;
      });
    });
    newChoiceInput[propName] = "";
  }

  async function removeOverrideChoice(propName: string, choice: string) {
    await writeOverrides((map) => {
      setOverrideField(map, propName, (e) => {
        e.choices = (e.choices ?? []).filter((c) => c !== choice);
      });
    });
  }

  async function setOverrideShow(propName: string, show: Visibility | "") {
    await writeOverrides((map) => {
      setOverrideField(map, propName, (e) => {
        if (show === "") delete e.show;
        else e.show = show;
      });
    });
  }

  async function setOverrideDefault(propName: string, value: string) {
    await writeOverrides((map) => {
      setOverrideField(map, propName, (e) => {
        if (value.trim() === "") delete e.default;
        else e.default = value;
      });
    });
  }

  // --- Icon + plural write-back (top-level Tag frontmatter keys) ---
  async function setIcon(name: string) {
    iconPickerOpen = false;
    iconSearch = "";
    const note = await api.getNote(noteId);
    const newContent = name
      ? updateFrontmatterKey(note.content, "icon", `"${name}"`)
      : removeFrontmatterKey(note.content, "icon");
    await api.updateNote(noteId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    queryClient.invalidateQueries({ queryKey: ["type", tagName] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

  async function savePlural(value: string) {
    editingPlural = false;
    const note = await api.getNote(noteId);
    const trimmed = value.trim();
    const newContent = trimmed
      ? updateFrontmatterKey(note.content, "plural", `"${trimmed}"`)
      : removeFrontmatterKey(note.content, "plural");
    await api.updateNote(noteId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    queryClient.invalidateQueries({ queryKey: ["type", tagName] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

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

  // Phase 4 — curated per-choice color palette. GLOBAL per-property (lives on
  // the Property page), not a per-type override. Values are hex so they render
  // identically in both themes; DisplayChip mixes them into a translucent
  // background + saturated text via color-mix.
  const CHOICE_PALETTE: string[] = [
    "#7CB342", // green
    "#43A047", // deep green
    "#6B9AE0", // blue
    "#A98BE0", // violet
    "#E8A33D", // amber
    "#E8697F", // coral / red
    "#62B8CE", // cyan
    "#8A909C", // muted / grey
  ];

  /**
   * Set (or clear, when `color === ""`) one choice's color on the Property
   * page's `choice_colors` frontmatter map. Mirrors `togglePropertyVisibilityFlag`'s
   * write path (read the Property page → updateFrontmatterKey → save), but for
   * the whole `choice_colors` map serialized as single-line FLOW YAML / JSON.
   * Removes the key entirely when the map empties so we never write
   * `choice_colors: {}`. GLOBAL to the property (every type using it), per spec
   * §4 Phase 4.
   */
  async function setChoiceColor(propName: string, choice: string, color: string) {
    const propPageId = propName.toLowerCase();
    let propNote;
    try {
      propNote = await api.getNote(propPageId);
    } catch {
      // No Property page → nothing to colour (colors only matter for a
      // configured select property, which always has a page).
      return;
    }
    const raw = propNote.metadata.custom.choice_colors;
    const map: Record<string, string> = {};
    if (raw && typeof raw === "object" && !Array.isArray(raw)) {
      for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
        if (typeof v === "string" && v.trim() !== "") map[k] = v;
      }
    }
    if (color === "") delete map[choice];
    else map[choice] = color;
    const serialized = serializeChoiceColors(map);
    const newContent = serialized
      ? updateFrontmatterKey(propNote.content, "choice_colors", serialized)
      : removeFrontmatterKey(propNote.content, "choice_colors");
    await api.updateNote(propPageId, newContent);
    queryClient.invalidateQueries({ queryKey: ["note", propPageId] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
    queryClient.invalidateQueries({ queryKey: ["type", tagName] });
  }

  // Which (property, choice) swatch picker is currently open. Encoded
  // `prop choice` so a choice value can contain anything.
  let openColorPicker = $state<string | null>(null);
  function colorKey(prop: string, choice: string): string {
    return `${prop} ${choice}`;
  }

  // Read the currently-stored color for a choice off the live registry def.
  function choiceColorFor(propName: string, choice: string): string | null {
    const def = propertyRegistry.get(propName.toLowerCase());
    if (!def) return null;
    return def.choice_colors[choice.toLowerCase()] ?? null;
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
  <!-- Type metadata: icon + plural -->
  <div class="flex items-center gap-2 text-[11px] mb-1">
    <span class="text-[10px] uppercase tracking-widest font-medium text-muted-foreground/40">Icon</span>
    <button
      class="flex items-center justify-center w-6 h-6 rounded hover:bg-muted/50 transition-colors text-[14px]"
      title="Set type icon"
      onclick={() => { iconPickerOpen = !iconPickerOpen; iconSearch = ""; }}
    >
      {#if resolvedTagIcon.component}
        {@const IconComp = resolvedTagIcon.component as any}
        <IconComp size={15} />
      {:else if resolvedTagIcon.emoji}
        {resolvedTagIcon.emoji}
      {:else}
        <span class="text-muted-foreground/30">＋</span>
      {/if}
    </button>
    {#if tagIcon}
      <span class="text-[10px] text-muted-foreground/40">{tagIcon}</span>
      <button
        class="text-[10px] text-muted-foreground/30 hover:text-destructive transition-colors"
        title="Clear icon"
        onclick={() => setIcon("")}
      >×</button>
    {/if}
  </div>

  {#if iconPickerOpen}
    <div class="space-y-1 mb-2 p-2 rounded border border-border/30 bg-muted/20">
      <input
        type="text"
        placeholder="Search icons…"
        bind:value={iconSearch}
        autofocus
        class="w-full text-[11px] bg-muted/50 rounded px-2 py-1 outline-none border border-transparent focus:border-ring/30"
      />
      <div class="grid grid-cols-8 gap-1 max-h-40 overflow-y-auto">
        {#each filteredIcons as name (name)}
          {@const IconComp = TABLER_ICONS[name] as any}
          <button
            class="flex items-center justify-center w-7 h-7 rounded hover:bg-accent transition-colors {tagIcon.toLowerCase() === name ? 'bg-accent ring-1 ring-ring/40' : ''}"
            title={name}
            onclick={() => setIcon(name)}
          >
            <IconComp size={15} />
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Plural row -->
  <div class="flex items-center gap-2 text-[11px] text-muted-foreground/50 mb-1">
    <span class="text-[10px] uppercase tracking-widest font-medium text-muted-foreground/40">Plural</span>
    {#if editingPlural}
      <input
        class="flex-1 text-[11px] bg-muted/50 rounded px-2 py-0.5 outline-none border border-ring/30"
        placeholder="Plural name…"
        bind:value={pluralInput}
        autofocus
        onkeydown={(e) => { if (e.key === "Enter") savePlural(pluralInput); if (e.key === "Escape") editingPlural = false; }}
        onblur={() => savePlural(pluralInput)}
      />
    {:else}
      <button
        class="flex-1 text-left text-[11px] px-1 -mx-1 rounded hover:bg-muted/50 transition-colors {tagPlural ? 'text-foreground/70' : 'text-muted-foreground/30 italic'}"
        onclick={() => { pluralInput = tagPlural; editingPlural = true; }}
      >
        {tagPlural || `${tagName} (default)`}
      </button>
    {/if}
  </div>

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
            {@const ov = rawOverrideFor(prop.name)}
            {@const ovChoices = ov.entry.choices ?? []}
            {@const ovShow = ov.entry.show ?? null}
            {@const ovDefault = ov.entry.default ?? ""}
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

              <!-- Per-type override editor (choices REPLACE / show / default) -->
              <div class="space-y-2 pt-1 border-t border-border/30">
                <div class="text-[10px] text-muted-foreground/50 uppercase tracking-widest">
                  Override for #{tagName}
                </div>

                <!-- SHOW: 3-way on_new / on_set / hidden (+ inherit) -->
                <div class="flex items-center gap-2">
                  <span class="text-[10px] text-muted-foreground/50 w-12">Show</span>
                  <select
                    class="text-[11px] bg-muted/50 rounded px-1.5 py-0.5 outline-none border border-transparent focus:border-ring/30"
                    value={ovShow ?? ""}
                    onchange={(e) => setOverrideShow(prop.name, (e.currentTarget as HTMLSelectElement).value as Visibility | "")}
                  >
                    <option value="">inherit</option>
                    <option value="on_new">on_new</option>
                    <option value="on_set">on_set</option>
                    <option value="hidden">hidden</option>
                  </select>
                </div>

                <!-- DEFAULT -->
                <div class="flex items-center gap-2">
                  <span class="text-[10px] text-muted-foreground/50 w-12">Default</span>
                  <input
                    type="text"
                    placeholder="inherit"
                    value={ovDefault}
                    class="flex-1 text-[11px] bg-muted/50 rounded px-2 py-0.5 outline-none border border-transparent focus:border-ring/30"
                    onkeydown={(e) => { if (e.key === "Enter") setOverrideDefault(prop.name, (e.currentTarget as HTMLInputElement).value); }}
                    onblur={(e) => setOverrideDefault(prop.name, (e.currentTarget as HTMLInputElement).value)}
                  />
                </div>

                <!-- CHOICES REPLACE: editable list; empty = inherit global -->
                <div class="space-y-1">
                  <div class="text-[10px] text-muted-foreground/50">
                    Choices {ovChoices.length === 0 ? "(inherits global)" : "(replaces global)"}
                  </div>
                  {#if ovChoices.length > 0}
                    <div class="flex flex-wrap gap-1">
                      {#each ovChoices as choice}
                        <span class="flex items-center gap-1 text-[11px] bg-muted/50 rounded px-1.5 py-0.5">
                          {choice}
                          <button
                            class="text-muted-foreground/40 hover:text-destructive transition-colors"
                            onclick={() => removeOverrideChoice(prop.name, choice)}
                          >×</button>
                        </span>
                      {/each}
                    </div>
                  {/if}
                  <div class="flex gap-1">
                    <input
                      type="text"
                      placeholder="Add choice…"
                      value={newChoiceInput[prop.name] ?? ""}
                      oninput={(e) => (newChoiceInput[prop.name] = (e.currentTarget as HTMLInputElement).value)}
                      onkeydown={(e) => { if (e.key === "Enter") addOverrideChoice(prop.name); }}
                      class="flex-1 text-[11px] bg-muted/50 rounded px-2 py-0.5 outline-none border border-transparent focus:border-ring/30"
                    />
                    <button
                      class="text-[10px] px-2 py-0.5 rounded bg-accent hover:bg-accent/80 text-accent-foreground transition-colors"
                      onclick={() => addOverrideChoice(prop.name)}
                    >Add</button>
                  </div>
                </div>
              </div>

              <!-- Choices (only for select-type properties) -->
              {#if hasChoices && def}
                <div class="space-y-0.5 pt-1 border-t border-border/30">
                  <div class="text-[10px] text-muted-foreground/50 mb-1">
                    Choices — hide per #{tagName}, color is global:
                  </div>
                  {#each def.choices as choice}
                    {@const isHidden = hidden.some((h) => h.toLowerCase() === choice.toLowerCase())}
                    {@const curColor = choiceColorFor(prop.name, choice)}
                    {@const pickerOpen = openColorPicker === colorKey(prop.name, choice)}
                    <div class="flex items-center gap-2 group/choice relative">
                      <label class="flex items-center gap-2 cursor-pointer flex-1">
                        <input
                          type="checkbox"
                          checked={isHidden}
                          class="w-3 h-3 accent-primary"
                          onchange={() => toggleHiddenChoice(prop.name, choice, isHidden)}
                        />
                        <span class="text-[11px] {isHidden ? 'line-through text-muted-foreground/40' : ''}">{choice}</span>
                      </label>
                      <!-- Color swatch / picker (GLOBAL per-property) -->
                      <button
                        class="w-4 h-4 rounded-full border border-border/50 shrink-0 transition-transform hover:scale-110"
                        style={curColor ? `background:${curColor}` : ""}
                        title={curColor ? `Choice color: ${curColor}` : "Set choice color"}
                        onclick={() => { openColorPicker = pickerOpen ? null : colorKey(prop.name, choice); }}
                      >
                        {#if !curColor}<span class="text-[9px] text-muted-foreground/40 leading-none">＋</span>{/if}
                      </button>
                      {#if pickerOpen}
                        <div class="absolute right-0 top-5 z-50 p-1.5 rounded border border-border bg-popover shadow-md flex items-center gap-1">
                          {#each CHOICE_PALETTE as c}
                            <button
                              class="w-4 h-4 rounded-full border {curColor === c ? 'ring-1 ring-ring/60' : 'border-border/40'}"
                              style="background:{c}"
                              title={c}
                              onclick={() => { setChoiceColor(prop.name, choice, c); openColorPicker = null; }}
                            ></button>
                          {/each}
                          {#if curColor}
                            <button
                              class="text-[11px] text-muted-foreground/40 hover:text-destructive px-1"
                              title="Clear color"
                              onclick={() => { setChoiceColor(prop.name, choice, ""); openColorPicker = null; }}
                            >×</button>
                          {/if}
                        </div>
                      {/if}
                    </div>
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

<script lang="ts">
  import { useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import {
    PROPERTY_TYPE_LABELS,
    updateFrontmatterKey,
    removeFrontmatterKey,
    serializeStringArray,
  } from "$lib/property-registry";
  import type { PropertyType } from "$lib/property-registry";

  let { note }: { note: Note } = $props();

  const queryClient = useQueryClient();

  const ALL_TYPES: PropertyType[] = [
    "text", "number", "select", "multi-select",
    "date", "checkbox", "url", "email", "phone", "object",
  ];

  const valueType = $derived<PropertyType>((note.metadata.custom.value_type as PropertyType) ?? "text");
  const choices = $derived<string[]>(
    Array.isArray(note.metadata.custom.choices) ? (note.metadata.custom.choices as string[]) : [],
  );

  let newChoice = $state("");
  let addingChoice = $state(false);
  let editingChoiceIdx = $state<number | null>(null);
  let editingChoiceValue = $state("");

  async function setValueType(type: PropertyType) {
    let updated = updateFrontmatterKey(note.content, "value_type", `"${type}"`);
    if (type !== "select" && type !== "multi-select") {
      updated = removeFrontmatterKey(updated, "choices");
    }
    await api.updateNote(note.id, updated);
    queryClient.invalidateQueries({ queryKey: ["note", note.id] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

  async function saveChoices(newChoices: string[]) {
    const updated = updateFrontmatterKey(
      note.content,
      "choices",
      serializeStringArray(newChoices),
    );
    await api.updateNote(note.id, updated);
    queryClient.invalidateQueries({ queryKey: ["note", note.id] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

  async function addChoice() {
    const c = newChoice.trim();
    if (!c || choices.includes(c)) return;
    await saveChoices([...choices, c]);
    newChoice = "";
    addingChoice = false;
  }

  async function removeChoice(idx: number) {
    await saveChoices(choices.filter((_, i) => i !== idx));
  }

  async function commitEditChoice(idx: number) {
    const trimmed = editingChoiceValue.trim();
    if (!trimmed || trimmed === choices[idx]) {
      editingChoiceIdx = null;
      return;
    }
    const updated = choices.map((c, i) => (i === idx ? trimmed : c));
    await saveChoices(updated);
    editingChoiceIdx = null;
  }

  function startEditChoice(idx: number) {
    editingChoiceIdx = idx;
    editingChoiceValue = choices[idx];
  }
</script>

<div class="space-y-4">
  <div>
    <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">
      Property Type
    </div>
    <div class="flex flex-wrap gap-1.5">
      {#each ALL_TYPES as t}
        <button
          class="text-[11px] px-2.5 py-0.5 rounded-full border transition-all {valueType === t
            ? 'bg-primary/10 border-primary/40 text-primary font-medium'
            : 'border-border/60 text-muted-foreground/60 hover:border-border hover:text-foreground'}"
          onclick={() => setValueType(t)}
        >
          {PROPERTY_TYPE_LABELS[t]}
        </button>
      {/each}
    </div>
  </div>

  {#if valueType === "select" || valueType === "multi-select"}
    <div>
      <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">
        Options
      </div>

      {#if choices.length === 0}
        <div class="text-[11px] text-muted-foreground/40 italic mb-2">No options yet</div>
      {:else}
        <div class="space-y-1 mb-2">
          {#each choices as choice, idx}
            <div class="flex items-center gap-1.5 group">
              {#if editingChoiceIdx === idx}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  autofocus
                  class="flex-1 text-[12px] bg-muted/60 border border-primary/40 rounded px-2 py-0.5 outline-none"
                  bind:value={editingChoiceValue}
                  onblur={() => commitEditChoice(idx)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") { e.preventDefault(); commitEditChoice(idx); }
                    if (e.key === "Escape") { editingChoiceIdx = null; }
                  }}
                />
              {:else}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span
                  class="flex-1 text-[12px] px-2 py-0.5 rounded bg-muted/30 cursor-text hover:bg-muted/60 transition-colors"
                  onclick={() => startEditChoice(idx)}
                >{choice}</span>
              {/if}
              <button
                class="text-[11px] text-muted-foreground/30 hover:text-destructive opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                onclick={() => removeChoice(idx)}
                title="Remove option"
              >×</button>
            </div>
          {/each}
        </div>
      {/if}

      {#if addingChoice}
        <div class="flex gap-1">
          <!-- svelte-ignore a11y_autofocus -->
          <input
            autofocus
            type="text"
            placeholder="Option name…"
            bind:value={newChoice}
            class="flex-1 text-[12px] bg-muted/50 rounded px-2 py-0.5 outline-none border border-transparent focus:border-ring/30"
            onkeydown={(e) => {
              if (e.key === "Enter") { e.preventDefault(); addChoice(); }
              if (e.key === "Escape") { addingChoice = false; newChoice = ""; }
            }}
          />
          <button
            class="text-[11px] px-2 py-0.5 rounded bg-accent hover:bg-accent/80 text-accent-foreground transition-colors"
            onclick={addChoice}
          >Add</button>
        </div>
      {:else}
        <button
          class="text-[11px] text-muted-foreground/40 hover:text-foreground transition-colors"
          onclick={() => { addingChoice = true; }}
        >+ Add option</button>
      {/if}
    </div>
  {/if}
</div>

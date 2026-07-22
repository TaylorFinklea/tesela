<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import {
    PROPERTY_TYPE_LABELS,
    updateFrontmatterKey,
    removeFrontmatterKey,
    serializeStringArray,
    buildRegistry,
  } from "$lib/property-registry";
  import type { PropertyType } from "$lib/property-registry";
  import {
    SLASH_RESERVED_CHORDS,
    DRAWER_RESERVED_CHORDS,
    BUILTIN_SLASH_CHORDS,
  } from "$lib/chord-keys";

  let { note }: { note: Note } = $props();

  const queryClient = useQueryClient();

  // For conflict detection, fetch all notes to see what other property
  // pages have claimed. Reused query key so we share cache with other
  // surfaces (BottomDrawer, BlockEditor, etc.).
  // Raised 500→5000 (tesela-sclr.1): 500 silently missed conflicting claims
  // from notes past #500.
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const allNotes = $derived((allNotesQuery.data ?? []) as Note[]);
  const propertyRegistry = $derived(buildRegistry(allNotes));

  const ALL_TYPES: PropertyType[] = [
    "text", "number", "select", "multi-select",
    "date", "datetime", "checkbox", "url", "email", "phone", "object", "node",
  ];

  const valueType = $derived<PropertyType>((note.metadata.custom.value_type as PropertyType) ?? "text");
  const choices = $derived<string[]>(
    Array.isArray(note.metadata.custom.choices) ? (note.metadata.custom.choices as string[]) : [],
  );

  let newChoice = $state("");
  let addingChoice = $state(false);
  let editingChoiceIdx = $state<number | null>(null);
  let editingChoiceValue = $state("");

  // ── Chord-key config ──────────────────────────────────────────────
  // Property page declares `chord_key:` (single letter) and, for select
  // properties, `value_chord_keys: { choice: letter, ... }`. Both honor
  // case (Shift+T is a different chord from t). Conflict detection
  // walks the registry and surfaces the other property's name when two
  // pages declare the same letter.
  const chordKey = $derived<string>(
    typeof note.metadata.custom.chord_key === "string"
      ? note.metadata.custom.chord_key as string
      : "",
  );
  const valueChordKeys = $derived<Record<string, string>>(
    note.metadata.custom.value_chord_keys && typeof note.metadata.custom.value_chord_keys === "object"
      ? note.metadata.custom.value_chord_keys as Record<string, string>
      : {},
  );
  const chordKeyIssue = $derived(checkPropertyChord(chordKey || null));

  /**
   * Reasons a chord_key declaration would silently fail. Surfaced inline
   * so the user knows why their pick won't take effect.
   */
  type ChordIssue =
    | { kind: "reserved"; surface: "slash" | "drawer"; reason: string }
    | { kind: "builtin"; verb: string }
    | { kind: "property"; otherName: string };

  function checkPropertyChord(letter: string | null): ChordIssue | null {
    if (!letter) return null;
    if (SLASH_RESERVED_CHORDS.has(letter)) {
      return {
        kind: "reserved",
        surface: "slash",
        reason: "The slash menu uses this key to open its filter input — no chord can claim it.",
      };
    }
    const verb = BUILTIN_SLASH_CHORDS.get(letter);
    if (verb) return { kind: "builtin", verb };
    const ownName = note.title.toLowerCase();
    for (const [name, def] of propertyRegistry) {
      if (name === ownName) continue;
      if (def.chord_key === letter) return { kind: "property", otherName: def.name };
    }
    return null;
  }

  /**
   * For per-choice value chords. Conflicts with the drawer's reserved nav
   * keys would break j/k/h/l/x/g navigation. We don't check against the
   * builtin slash verbs here — value chords only fire inside a value
   * submenu (where `t` for Task is irrelevant).
   */
  type ValueChordIssue =
    | { kind: "reserved"; reason: string }
    | { kind: "duplicate"; otherChoice: string };

  function checkValueChord(choice: string, letter: string | null): ValueChordIssue | null {
    if (!letter) return null;
    if (DRAWER_RESERVED_CHORDS.has(letter)) {
      return {
        kind: "reserved",
        reason: "The bottom drawer reserves this key for nav (j/k/h/l/x/g). Pick another letter.",
      };
    }
    for (const [otherChoice, otherLetter] of Object.entries(valueChordKeys)) {
      if (otherChoice === choice.toLowerCase()) continue;
      if (otherLetter === letter) return { kind: "duplicate", otherChoice };
    }
    return null;
  }

  // Capture-mode state — when true, the next keystroke replaces the chord
  // key (instead of typing into a normal text input). Mirrors how editor
  // shortcut configurators work; avoids the user having to type their key
  // and then dismiss a popover separately.
  let capturingChordFor = $state<string | null>(null); // null | "" (top-level) | "<choice>"

  function startCapture(target: string) {
    capturingChordFor = target;
  }

  async function captureKey(e: KeyboardEvent) {
    if (capturingChordFor === null) return;
    if (e.key === "Escape") {
      e.preventDefault();
      capturingChordFor = null;
      return;
    }
    // Single printable letter only. Shift+letter passes through as the
    // capital letter; modifier-only keys (Shift, Ctrl, …) are ignored.
    if (e.key.length !== 1 || !/^[A-Za-z]$/.test(e.key)) {
      e.preventDefault();
      return;
    }
    e.preventDefault();
    const letter = e.key;
    const target = capturingChordFor;
    capturingChordFor = null;
    if (target === "") {
      await saveChordKey(letter);
    } else {
      await saveValueChordKey(target, letter);
    }
  }

  async function saveChordKey(letter: string | null) {
    let updated: string;
    if (letter === null || letter === "") {
      updated = removeFrontmatterKey(note.content, "chord_key");
    } else {
      updated = updateFrontmatterKey(note.content, "chord_key", `"${letter}"`);
    }
    await api.updateNote(note.id, updated);
    queryClient.invalidateQueries({ queryKey: ["note", note.id] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

  /** Inline YAML object serializer for `value_chord_keys`. Quotes choice
   *  keys when they contain hyphens or other YAML-significant chars; the
   *  letter values are always quoted to avoid YAML interpreting `n` as
   *  null or `y` as true. */
  function serializeValueChordKeys(map: Record<string, string>): string {
    const entries = Object.entries(map);
    if (entries.length === 0) return "{}";
    const pairs = entries.map(([k, v]) => {
      const safeKey = /^[A-Za-z_][A-Za-z0-9_]*$/.test(k) ? k : `"${k}"`;
      return `${safeKey}: "${v}"`;
    });
    return `{ ${pairs.join(", ")} }`;
  }

  async function saveValueChordKey(choice: string, letter: string | null) {
    const next: Record<string, string> = { ...valueChordKeys };
    if (letter === null || letter === "") {
      delete next[choice];
    } else {
      next[choice] = letter;
    }
    let updated: string;
    if (Object.keys(next).length === 0) {
      updated = removeFrontmatterKey(note.content, "value_chord_keys");
    } else {
      updated = updateFrontmatterKey(note.content, "value_chord_keys", serializeValueChordKeys(next));
    }
    await api.updateNote(note.id, updated);
    queryClient.invalidateQueries({ queryKey: ["note", note.id] });
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }

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

<svelte:window onkeydown={captureKey} />

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

  <!-- Chord key for the property itself. Click to capture; Esc to cancel.
       Conflict warning shown inline when another property page declares
       the same letter. -->
  <div>
    <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">
      Chord key
    </div>
    <div class="flex items-center gap-2">
      {#if capturingChordFor === ""}
        <span class="inline-flex items-center px-2 py-0.5 rounded border border-primary/60 bg-primary/10 text-primary text-[11px] font-mono">
          press a letter…
        </span>
        <span class="text-[10px] text-muted-foreground/50">esc to cancel</span>
      {:else if chordKey}
        <kbd class="px-1.5 py-0.5 rounded border border-border bg-muted/40 text-[11px] font-mono font-semibold text-primary">{chordKey}</kbd>
        <button
          class="text-[10px] text-muted-foreground/50 hover:text-foreground/70"
          onclick={() => startCapture("")}
        >change</button>
        <button
          class="text-[10px] text-muted-foreground/40 hover:text-destructive"
          onclick={() => saveChordKey(null)}
        >clear</button>
      {:else}
        <button
          class="text-[11px] px-2 py-0.5 rounded border border-dashed border-border/60 text-muted-foreground/60 hover:text-foreground hover:border-border"
          onclick={() => startCapture("")}
        >+ set chord</button>
      {/if}
      {#if chordKeyIssue}
        {#if chordKeyIssue.kind === "reserved"}
          <span
            class="text-[10px] px-1.5 py-0.5 rounded border border-destructive/40 bg-destructive/10 text-destructive"
            title={chordKeyIssue.reason}
          >reserved · won't fire</span>
        {:else if chordKeyIssue.kind === "builtin"}
          <span
            class="text-[10px] px-1.5 py-0.5 rounded border border-destructive/40 bg-destructive/10 text-destructive"
            title="The slash menu's built-in '{chordKeyIssue.verb}' verb owns this key. Your property will fall back to first-letter at the top level."
          >taken by {chordKeyIssue.verb} (builtin)</span>
        {:else}
          <span
            class="text-[10px] px-1.5 py-0.5 rounded border border-destructive/40 bg-destructive/10 text-destructive"
            title="Property page '{chordKeyIssue.otherName}' also declares chord_key: '{chordKey}'. The chord menu will fall back to first-letter for whichever property loads later."
          >taken by {chordKeyIssue.otherName}</span>
        {/if}
      {/if}
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
              <!-- Per-choice chord. Click to capture; click again to clear. -->
              {#if capturingChordFor === choice.toLowerCase()}
                <span class="text-[10px] px-1.5 py-0.5 rounded border border-primary/60 bg-primary/10 text-primary font-mono">press…</span>
              {:else if valueChordKeys[choice.toLowerCase()]}
                {@const vIssue = checkValueChord(choice, valueChordKeys[choice.toLowerCase()])}
                <button
                  class="px-1.5 py-0.5 rounded border {vIssue ? 'border-destructive/40 bg-destructive/10 text-destructive' : 'border-border bg-muted/40 text-primary'} text-[11px] font-mono font-semibold hover:border-primary/60 transition-colors"
                  onclick={() => startCapture(choice.toLowerCase())}
                  title={vIssue ? (vIssue.kind === "reserved" ? vIssue.reason : `Same chord as '${vIssue.otherChoice}'.`) : "Click to change. Esc to cancel."}
                >{valueChordKeys[choice.toLowerCase()]}</button>
                {#if vIssue}
                  <span class="text-[10px] text-destructive/80">
                    {vIssue.kind === "reserved" ? "reserved" : `dupe: ${vIssue.otherChoice}`}
                  </span>
                {/if}
              {:else}
                <button
                  class="text-[10px] px-1.5 py-0.5 rounded border border-dashed border-border/40 text-muted-foreground/40 hover:text-foreground/70 hover:border-border transition-colors"
                  onclick={() => startCapture(choice.toLowerCase())}
                  title="Set a chord key for this option"
                >set</button>
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

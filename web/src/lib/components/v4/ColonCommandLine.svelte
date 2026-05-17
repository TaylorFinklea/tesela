<script lang="ts">
  /*
   * `:` ex-mode command line with autocomplete.
   *
   * As the user types, a small dropdown above the input shows matching
   * verbs (with their glyph + label + shortcut). Arrows navigate; Tab
   * accepts the highlighted suggestion (replacing the input text with
   * the verb); Enter runs the highlighted suggestion (or the typed verb
   * verbatim if nothing's highlighted).
   */
  import { onMount, tick } from "svelte";
  import {
    closeColonMode,
    isColonModeOpen,
  } from "$lib/stores/colon-mode.svelte";
  import {
    buildV4Commands,
    findCommandByVerb,
    matchesV4Command,
    type V4Command,
  } from "$lib/v4/commands";
  import { openPeek } from "$lib/stores/peek.svelte";
  import { openFullscreenGraph } from "$lib/stores/fullscreen-overlay.svelte";

  const open = $derived(isColonModeOpen());

  let value = $state("");
  let error = $state<string | null>(null);
  let inputEl = $state<HTMLInputElement | undefined>();
  let highlightedIdx = $state(0);

  // The full registry of verbs, computed once per open.
  const allCommands = $derived.by(() => (open ? buildV4Commands() : []));

  // Built-in non-registry verbs (peek, graph) surface as autocomplete rows too.
  const BUILTINS = [
    { verb: "peek", label: "Open Peek popover", glyph: "i", shortcut: "⌘I" },
    { verb: "graph", label: "Fullscreen graph", glyph: "✦", shortcut: "⌘G" },
  ];

  type Row = {
    verb: string;
    label: string;
    glyph: string;
    shortcut?: string;
    cmd?: V4Command;
  };

  const suggestions = $derived.by<Row[]>(() => {
    if (!open) return [];
    const q = value.trim();
    const rows: Row[] = [];
    for (const b of BUILTINS) {
      if (!q || b.verb.includes(q.toLowerCase())) {
        rows.push({
          verb: b.verb,
          label: b.label,
          glyph: b.glyph,
          shortcut: b.shortcut,
        });
      }
    }
    for (const cmd of allCommands) {
      if (!cmd.verb) continue;
      if (matchesV4Command(cmd, q)) {
        rows.push({
          verb: cmd.verb,
          label: cmd.label,
          glyph: cmd.glyph,
          shortcut: cmd.shortcut,
          cmd,
        });
      }
    }
    return rows.slice(0, 12);
  });

  $effect(() => {
    if (open) {
      value = "";
      error = null;
      highlightedIdx = 0;
      tick().then(() => inputEl?.focus());
    }
  });

  // Keep highlight in range as the suggestion list changes.
  $effect(() => {
    const n = suggestions.length;
    if (n === 0) {
      highlightedIdx = 0;
    } else if (highlightedIdx >= n) {
      highlightedIdx = n - 1;
    }
  });

  async function runVerb(verb: string, arg?: string) {
    if (verb === "peek") {
      openPeek((arg ?? "backlinks-of-page") as Parameters<typeof openPeek>[0]);
      closeColonMode();
      return;
    }
    if (verb === "graph") {
      openFullscreenGraph();
      closeColonMode();
      return;
    }
    const cmd = findCommandByVerb(verb);
    if (!cmd) {
      error = `unknown verb: :${verb}`;
      return;
    }
    closeColonMode();
    try {
      await cmd.run(arg);
    } catch (e) {
      console.error("v4 colon: command failed", verb, e);
    }
  }

  async function run() {
    const raw = value.trim();
    if (!raw) {
      // If the input is empty but a suggestion is highlighted, run it.
      const pick = suggestions[highlightedIdx];
      if (pick) {
        await runVerb(pick.verb);
        return;
      }
      closeColonMode();
      return;
    }
    // If the user typed a verb that matches exactly, just run it. Otherwise
    // prefer the highlighted suggestion.
    const [typedVerb, ...rest] = raw.split(/\s+/);
    const arg = rest.join(" ").trim() || undefined;
    const exact = findCommandByVerb(typedVerb);
    if (exact || typedVerb === "peek" || typedVerb === "graph") {
      await runVerb(typedVerb, arg);
      return;
    }
    const pick = suggestions[highlightedIdx];
    if (pick) {
      await runVerb(pick.verb, arg);
      return;
    }
    error = `unknown verb: :${typedVerb}`;
  }

  function accept() {
    // Tab → fill input with the highlighted suggestion's verb so the
    // user can keep typing args.
    const pick = suggestions[highlightedIdx];
    if (!pick) return;
    value = pick.verb + " ";
    tick().then(() => inputEl?.focus());
  }

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeColonMode();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      void run();
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      accept();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      e.stopPropagation();
      if (suggestions.length > 0) {
        highlightedIdx = (highlightedIdx + 1) % suggestions.length;
      }
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      if (suggestions.length > 0) {
        highlightedIdx =
          (highlightedIdx - 1 + suggestions.length) % suggestions.length;
      }
      return;
    }
  }

  onMount(() => {
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  });
</script>

{#if open}
  {#if suggestions.length > 0}
    <ul class="v4-colon-suggestions" role="listbox">
      {#each suggestions as row, i (row.verb)}
        <li
          class:active={i === highlightedIdx}
          role="option"
          aria-selected={i === highlightedIdx}
        >
          <span class="glyph">{row.glyph}</span>
          <span class="verb">:{row.verb}</span>
          <span class="label">{row.label}</span>
          {#if row.shortcut}
            <span class="shortcut">{row.shortcut}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  <div class="v4-colon" role="dialog" aria-label="vim ex command">
    <span class="v4-colon-prompt">:</span>
    <input
      bind:this={inputEl}
      bind:value
      class="v4-colon-input"
      placeholder="type a verb · Tab to complete · Enter to run · Esc to cancel"
      spellcheck={false}
      autocorrect="off"
      autocapitalize="off"
      oninput={() => {
        error = null;
      }}
    />
    {#if error}
      <span class="v4-colon-error">{error}</span>
    {/if}
  </div>
{/if}

<style>
  .v4-colon {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 26px; /* sits just above the status bar */
    background: var(--v4-bg);
    border-top: 1px solid var(--v4-accent-dim);
    padding: 6px 14px;
    display: flex;
    align-items: center;
    gap: 8px;
    z-index: 80;
  }
  .v4-colon-suggestions {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 60px; /* sits above the input strip */
    margin: 0;
    padding: 4px 0;
    list-style: none;
    background: var(--v4-bg);
    border-top: 1px solid var(--v4-hair);
    z-index: 80;
    max-height: 280px;
    overflow: auto;
  }
  .v4-colon-suggestions li {
    display: grid;
    grid-template-columns: 18px 130px 1fr auto;
    align-items: baseline;
    gap: 10px;
    padding: 3px 18px;
    font-family: var(--v4-mono);
    font-size: 12px;
    color: var(--v4-ink2);
  }
  .v4-colon-suggestions li.active {
    background: color-mix(in srgb, var(--v4-accent) 14%, transparent);
    color: var(--v4-ink);
  }
  .v4-colon-suggestions .glyph {
    color: var(--v4-ink5);
    text-align: center;
  }
  .v4-colon-suggestions .verb {
    color: var(--v4-accent);
  }
  .v4-colon-suggestions .label {
    color: var(--v4-ink3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v4-colon-suggestions .shortcut {
    color: var(--v4-ink5);
    font-size: 10.5px;
  }
  .v4-colon-prompt {
    color: var(--v4-accent);
    font-family: var(--v4-mono);
    font-size: 14px;
  }
  .v4-colon-input {
    flex: 1;
    background: transparent;
    border: 0;
    color: var(--v4-ink);
    font-family: var(--v4-mono);
    font-size: 13px;
    outline: none;
  }
  .v4-colon-input::placeholder {
    color: var(--v4-ink6);
  }
  .v4-colon-error {
    color: #f87171;
    font-family: var(--v4-mono);
    font-size: 11px;
  }
</style>

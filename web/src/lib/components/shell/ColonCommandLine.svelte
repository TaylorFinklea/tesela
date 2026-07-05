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
    getColonPriorPaneId,
    isColonModeOpen,
  } from "$lib/stores/colon-mode.svelte";
  import {
    commandRegistry,
    effectiveShortcut,
    matchesCommand,
    type Command,
    type CommandContext,
  } from "$lib/command-registry.svelte";
  import * as keybindings from "$lib/stores/keybindings.svelte";
  import { focusLeaf } from "$lib/buffer/state.svelte";
  import type { LeafId } from "$lib/buffer/types";
  // Uses app role tokens from app.css; Graphite bridges those roles in
  // graphite/tokens.css so the fixed strip inherits the active chrome.

  interface Props {
    ctx: CommandContext;
  }
  let { ctx }: Props = $props();

  const open = $derived(isColonModeOpen());

  let value = $state("");
  let error = $state<string | null>(null);
  let inputEl = $state<HTMLInputElement | undefined>();
  let highlightedIdx = $state(0);

  // The context-filtered registry of verbs, computed once per open.
  const allCommands = $derived.by(() =>
    open ? commandRegistry.availableOn('colon', ctx, keybindings.snapshot()) : [],
  );

  type Row = {
    verb: string;
    label: string;
    glyph: string;
    shortcut?: string;
    cmd?: Command;
  };

  const suggestions = $derived.by<Row[]>(() => {
    if (!open) return [];
    const q = value.trim();
    const rows: Row[] = [];
    const overrides = keybindings.snapshot();
    for (const cmd of allCommands) {
      if (!cmd.verb) continue;
      if (matchesCommand(cmd, q)) {
        rows.push({
          verb: cmd.verb,
          label: cmd.label,
          glyph: cmd.glyph,
          shortcut: effectiveShortcut(cmd, overrides),
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

  function restoreFocus() {
    const prior = getColonPriorPaneId();
    if (prior) focusLeaf(prior as LeafId);
  }

  async function runVerb(verb: string, arg?: string) {
    const cmd = commandRegistry.findByVerb(verb);
    if (!cmd) {
      error = `unknown verb: :${verb}`;
      return;
    }
    // Most verbs operate on the focused pane — restore that focus first.
    restoreFocus();
    closeColonMode();
    try {
      await cmd.run(arg, ctx);
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
    const exact = commandRegistry.findByVerb(typedVerb);
    if (exact) {
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
      restoreFocus();
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
    /* Sits just above the status bar. v4's status row is 26px; Graphite's is
       30px — the chrome overrides `--tesela-colon-bottom` (graphite/tokens.css). */
    bottom: var(--tesela-colon-bottom, 26px);
    background: var(--bg);
    border-top: 1px solid var(--accent-spark-dim);
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
    /* Sits above the ~34px input strip. */
    bottom: calc(var(--tesela-colon-bottom, 26px) + 34px);
    margin: 0;
    padding: 4px 0;
    list-style: none;
    background: var(--bg);
    border-top: 1px solid var(--line-soft);
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
    font-family: var(--theme-font-mono);
    font-size: 12px;
    color: var(--fg-muted);
  }
  .v4-colon-suggestions li.active {
    background: color-mix(in srgb, var(--accent-spark) 14%, transparent);
    color: var(--fg-default);
  }
  .v4-colon-suggestions .glyph {
    color: var(--fg-faint);
    text-align: center;
  }
  .v4-colon-suggestions .verb {
    color: var(--accent-spark);
  }
  .v4-colon-suggestions .label {
    color: var(--fg-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v4-colon-suggestions .shortcut {
    color: var(--fg-faint);
    font-size: 10.5px;
  }
  .v4-colon-prompt {
    color: var(--accent-spark);
    font-family: var(--theme-font-mono);
    font-size: 14px;
  }
  .v4-colon-input {
    flex: 1;
    background: transparent;
    border: 0;
    color: var(--fg-default);
    font-family: var(--theme-font-mono);
    font-size: 13px;
    outline: none;
  }
  .v4-colon-input::placeholder {
    color: var(--fg-faint);
  }
  .v4-colon-error {
    color: #f87171;
    font-family: var(--theme-font-mono);
    font-size: 11px;
  }
</style>

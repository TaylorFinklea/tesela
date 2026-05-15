<script lang="ts">
  /*
   * Prism v4 — `:` ex-mode command line.
   *
   * Slim input that slides up over the status bar when `:` is pressed
   * outside an editor (the layout's onKey handler arms us). Parses the
   * first word as a verb, dispatches via the v4 commands registry, and
   * passes the rest as the verb's argument. Mirrors vim semantics:
   *
   *     :vsplit
   *     :jump 2026-05-15
   *     :peek backlinks
   *     :graph
   *
   * Bail-out: `:` *inside* a focused cm-editor goes to cm-vim's own
   * ex-mode and never reaches this component. That's by design — the
   * layout's keymap explicitly skips when focus is in `.cm-editor`.
   */
  import { onMount, tick } from "svelte";
  import {
    closeColonMode,
    isColonModeOpen,
  } from "$lib/stores/colon-mode.svelte";
  import { findCommandByVerb } from "$lib/v4/commands";
  import { openPeek } from "$lib/stores/peek.svelte";
  import { openFullscreenGraph } from "$lib/stores/fullscreen-overlay.svelte";

  const open = $derived(isColonModeOpen());

  let value = $state("");
  let error = $state<string | null>(null);
  let inputEl = $state<HTMLInputElement | undefined>();

  $effect(() => {
    if (open) {
      value = "";
      error = null;
      tick().then(() => inputEl?.focus());
    }
  });

  async function run() {
    const raw = value.trim();
    if (!raw) {
      closeColonMode();
      return;
    }
    const [verb, ...rest] = raw.split(/\s+/);
    const arg = rest.join(" ").trim() || undefined;

    // Built-in verbs that don't live in the v4 commands registry —
    // they open Phase 5 surfaces (peek, fullscreen graph) rather than
    // mutating the pane tree.
    if (verb === "peek") {
      const kind = (arg ?? "backlinks") as Parameters<typeof openPeek>[0];
      openPeek(kind);
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

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeColonMode();
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      void run();
    }
  }

  onMount(() => {
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  });
</script>

{#if open}
  <div class="v4-colon" role="dialog" aria-label="vim ex command">
    <span class="v4-colon-prompt">:</span>
    <input
      bind:this={inputEl}
      bind:value
      class="v4-colon-input"
      placeholder="vsplit · hsplit · jump <slug> · peek <kind> · graph · …"
      spellcheck={false}
      autocorrect="off"
      autocapitalize="off"
      oninput={() => (error = null)}
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

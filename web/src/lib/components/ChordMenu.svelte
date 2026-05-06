<script module lang="ts">
  /**
   * Generic spacemacs/which-key style chord menu.
   *
   * Shape: tree of `ChordNode`s — each node is either a leaf (`action`)
   * or a group (`children`). Pressing a node's `key` either runs the
   * action and closes the menu, or descends into the group's children.
   *
   * Triggers (set up by the caller, e.g. `+layout.svelte`):
   *   - `Space` from NORMAL mode (vim swallows space otherwise)
   *   - `Ctrl+,` from any mode (works inside cm-editor INSERT)
   *
   * Behavior:
   *   - Every keystroke is a chord; no arrow nav, no filter typing.
   *   - `Esc` / `Backspace` ascends one level; closes at root.
   *   - Click outside or click on row also descends/runs.
   *   - Capture-phase keydown listener so cm-vim doesn't consume the keys.
   */
  export type ChordNode = {
    key: string;
    label: string;
    action?: () => void;
    children?: ChordNode[];
    /**
     * Phase 10.4 — switch the popover into single-text-input mode when this
     * leaf is selected. Used by `/p` for text/number/url-typed properties:
     * the user picks the property key from a chord submenu, then types the
     * value directly in the popover. Enter calls `onSubmit(value)` and
     * closes; Esc backs up to the parent chord level so the user can pick a
     * different key without retyping `/p`.
     */
    input?: {
      placeholder?: string;
      /** Pre-filled value (e.g. existing property value when re-editing). */
      initial?: string;
      onSubmit: (value: string) => void;
    };
    /**
     * Phase 10.2 follow-up — optional alternative-path hint rendered as a
     * faint right-aligned chip. Used to advertise the alternative way to
     * reach the same action: a vim NORMAL chord (e.g. `gp` for "Toggle
     * props"), a global keyboard shortcut (e.g. `⌘K` for "Search palette"),
     * or a destination URL (e.g. `/p/tasks` for "Go to Tasks"). Free-form
     * string — render as-is in a kbd chip. Doubles the menu as a discovery
     * surface for whatever path the user might prefer next time.
     */
    hint?: string;
  };
</script>

<script lang="ts">
  import { onMount } from "svelte";

  let {
    tree,
    onclose,
    initialPath = [],
    position,
    headLabel = "SPC",
  }: {
    tree: ChordNode[];
    onclose: () => void;
    /**
     * Phase 10.2 follow-up — open the menu pre-descended into a specific
     * sub-tree. Pass an array of group labels matching `ChordNode.label`
     * (e.g. `["Go to"]`). Used by the `g` chord shortcut so the user lands
     * in the Go-to submenu without typing `Space g` first.
     */
    initialPath?: string[];
    /**
     * Phase 10.3 — anchor the popover at a fixed cursor position instead
     * of the centered modal placement. Used by the in-block `/` slash menu
     * which opens at the typing caret. When omitted (default), the menu
     * renders centered at top: 30% (the global leader presentation).
     */
    position?: { x: number; y: number };
    /**
     * Prefix label shown in the breadcrumb header. Defaults to `SPC` for
     * the global leader; set to `/` for the in-block slash menu so the
     * user sees the trigger that opened it.
     */
    headLabel?: string;
  } = $props();

  let breadcrumb = $state<string[]>(initialPath);
  /**
   * Phase 10.4 — when set, the popover is in single-input mode. `node` is
   * the leaf the user selected (we keep a reference so we can read its
   * `onSubmit` and label for the breadcrumb). `value` is the live text
   * input. Esc clears `inputNode` and returns to the chord-list view at
   * the same `breadcrumb` level so the user can pick a different leaf.
   */
  let inputNode = $state<ChordNode | null>(null);
  let inputValue = $state("");
  let inputEl = $state<HTMLInputElement | null>(null);

  let currentLevel = $derived.by((): ChordNode[] => {
    let level = tree;
    for (const name of breadcrumb) {
      const found = level.find((n) => n.label === name);
      if (found?.children) level = found.children;
      else break;
    }
    return level;
  });

  function handleSelect(node: ChordNode) {
    if (node.children) {
      breadcrumb = [...breadcrumb, node.label];
    } else if (node.input) {
      inputNode = node;
      inputValue = node.input.initial ?? "";
      // Focus the input next tick — it's not yet in the DOM when this fires.
      setTimeout(() => inputEl?.focus(), 0);
    } else if (node.action) {
      node.action();
      onclose();
    }
  }

  function ascend() {
    if (inputNode) {
      inputNode = null;
      inputValue = "";
      return;
    }
    if (breadcrumb.length > 0) {
      breadcrumb = breadcrumb.slice(0, -1);
    } else {
      onclose();
    }
  }

  function commitInput() {
    if (!inputNode?.input) return;
    inputNode.input.onSubmit(inputValue);
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || (e.key === "Backspace" && !inputNode)) {
      e.preventDefault();
      e.stopPropagation();
      ascend();
      return;
    }
    if (inputNode) {
      // In input mode: let the input handle most keys. Capture only Enter
      // (commit) here. Backspace is delegated to the input field for
      // intra-value editing — Esc is the back-up affordance instead.
      if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        commitInput();
        return;
      }
      // All other keys flow to the <input> normally — do NOT swallow.
      return;
    }
    // Match by exact key (case-sensitive) — Shift+letter is a different chord.
    const match = currentLevel.find((n) => n.key === e.key);
    if (match) {
      e.preventDefault();
      e.stopPropagation();
      handleSelect(match);
      return;
    }
    // Swallow EVERY other keystroke while the menu is open — modal behavior.
    // Without this, arrows would still move the cm-editor caret behind the
    // popover, vim chords like `dd` would still execute, etc. Modifier keys
    // alone (Shift/Ctrl/Alt) are harmless to swallow — they have no effect
    // until paired with another key. Cmd+letter combos are intentionally
    // swallowed too: the user must Esc out before invoking ⌘K from here.
    e.preventDefault();
    e.stopPropagation();
  }

  onMount(() => {
    document.addEventListener("keydown", handleKeydown, true);
    return () => document.removeEventListener("keydown", handleKeydown, true);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="chord-overlay" onclick={onclose}></div>
<div
  class="chord-pop {position ? 'chord-pop--anchored' : ''}"
  role="menu"
  style={position ? `left: ${position.x}px; top: ${position.y}px;` : undefined}
>
  <div class="chord-head">
    <span class="chord-prefix">{headLabel}</span>
    {#each breadcrumb as crumb}
      <span class="chord-sep">›</span>
      <span class="chord-crumb">{crumb}</span>
    {/each}
    {#if inputNode}
      <span class="chord-sep">›</span>
      <span class="chord-crumb">{inputNode.label}</span>
    {/if}
  </div>
  {#if inputNode}
    <div class="chord-input-wrap">
      <input
        bind:this={inputEl}
        bind:value={inputValue}
        class="chord-input"
        type="text"
        placeholder={inputNode.input?.placeholder ?? ""}
        autocomplete="off"
        spellcheck="false"
      />
      <div class="chord-input-help">
        <kbd class="chord-key">↵</kbd> set
        <kbd class="chord-key">Esc</kbd> back
      </div>
    </div>
  {:else}
    <div class="chord-list">
      {#each currentLevel as node (node.key)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="chord-row" onclick={() => handleSelect(node)}>
          <kbd class="chord-key">{node.key}</kbd>
          <span class="chord-label">{node.label}</span>
          {#if node.hint}
            <kbd class="chord-hint" title="Alternative path">{node.hint}</kbd>
          {/if}
          {#if node.children || node.input}
            <span class="chord-more">›</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .chord-overlay { position: fixed; inset: 0; z-index: 49; }
  .chord-pop {
    position: fixed;
    left: 50%;
    top: 30%;
    transform: translateX(-50%);
    z-index: 50;
    min-width: 240px;
    max-width: 360px;
    background: var(--popover, var(--v9-bg-2));
    color: var(--popover-foreground, var(--foreground));
    border: 1px solid var(--border, var(--v9-line));
    border-radius: 8px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 6px;
    font-family: var(--v9-mono);
    font-size: 12px;
  }
  /* Phase 10.3 — cursor-anchored mode (slash menu in cm-editor). The inline
     `style` overrides set left/top from caret coords; we drop the centering
     transform and tighten the min-width so the popover sits flush to the
     typing position. */
  .chord-pop.chord-pop--anchored {
    top: auto;
    left: auto;
    transform: none;
    min-width: 220px;
  }
  .chord-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 8px;
    border-bottom: 1px solid var(--v9-line);
    margin-bottom: 4px;
  }
  .chord-prefix {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--primary);
    font-weight: 600;
    letter-spacing: 0.08em;
  }
  .chord-sep { color: var(--v9-ink-faint); font-size: 11px; }
  .chord-crumb { color: var(--v9-ink-faint); font-size: 11px; }
  .chord-list { display: flex; flex-direction: column; gap: 1px; }
  .chord-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
  }
  .chord-row:hover { background: color-mix(in srgb, var(--primary) 12%, transparent); }
  .chord-key {
    display: inline-block;
    min-width: 20px;
    padding: 1px 6px;
    text-align: center;
    background: color-mix(in srgb, var(--foreground) 8%, transparent);
    color: var(--primary);
    border: 1px solid var(--v9-line);
    border-radius: 3px;
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
  }
  .chord-label { color: var(--foreground); flex: 1; }
  .chord-hint {
    display: inline-block;
    padding: 1px 5px;
    background: transparent;
    color: var(--v9-ink-faint);
    border: 1px solid var(--v9-line);
    border-radius: 3px;
    font-family: inherit;
    font-size: 10px;
    font-weight: 500;
    opacity: 0.85;
    max-width: 14ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chord-more { color: var(--v9-ink-faint); font-size: 11px; }
  .chord-input-wrap {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px;
  }
  .chord-input {
    width: 100%;
    padding: 6px 8px;
    background: var(--background, var(--v9-bg-1));
    color: var(--foreground);
    border: 1px solid var(--primary);
    border-radius: 4px;
    font-family: inherit;
    font-size: 12px;
    outline: none;
  }
  .chord-input:focus { border-color: var(--primary); box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 25%, transparent); }
  .chord-input-help { display: flex; gap: 8px; align-items: center; padding: 0 4px; color: var(--v9-ink-faint); font-size: 10px; }
</style>

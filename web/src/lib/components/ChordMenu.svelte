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
     * Phase 10.2 follow-up — optional vim/keymap equivalent rendered as a
     * faint right-aligned chip. Used to advertise the NORMAL-mode shortcut
     * for the same action (e.g. `gp` for "Toggle props"), so the leader
     * menu doubles as a discovery surface for the keyboard shortcuts.
     * Free-form string — render as-is in a kbd chip.
     */
    vimChord?: string;
  };
</script>

<script lang="ts">
  import { onMount } from "svelte";

  let {
    tree,
    onclose,
  }: {
    tree: ChordNode[];
    onclose: () => void;
  } = $props();

  let breadcrumb = $state<string[]>([]);
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
    } else if (node.action) {
      node.action();
      onclose();
    }
  }

  function ascend() {
    if (breadcrumb.length > 0) {
      breadcrumb = breadcrumb.slice(0, -1);
    } else {
      onclose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === "Backspace") {
      e.preventDefault();
      e.stopPropagation();
      ascend();
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
    // Swallow other single-char keys so they don't bubble (e.g. into vim).
    if (e.key.length === 1 && !e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  onMount(() => {
    document.addEventListener("keydown", handleKeydown, true);
    return () => document.removeEventListener("keydown", handleKeydown, true);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="chord-overlay" onclick={onclose}></div>
<div class="chord-pop" role="menu">
  <div class="chord-head">
    <span class="chord-prefix">SPC</span>
    {#each breadcrumb as crumb}
      <span class="chord-sep">›</span>
      <span class="chord-crumb">{crumb}</span>
    {/each}
  </div>
  <div class="chord-list">
    {#each currentLevel as node (node.key)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="chord-row" onclick={() => handleSelect(node)}>
        <kbd class="chord-key">{node.key}</kbd>
        <span class="chord-label">{node.label}</span>
        {#if node.vimChord}
          <kbd class="chord-vim" title="Vim NORMAL equivalent">{node.vimChord}</kbd>
        {/if}
        {#if node.children}
          <span class="chord-more">›</span>
        {/if}
      </div>
    {/each}
  </div>
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
  .chord-vim {
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
  }
  .chord-more { color: var(--v9-ink-faint); font-size: 11px; }
</style>

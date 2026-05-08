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
    /**
     * Phase 12.2 — when this node's preferred chord_key was already claimed
     * by a sibling, the assigner falls back to first-letter and records the
     * collision here. Surfaced as a small "taken by X" warning so the user
     * knows to fix the property page's `chord_key:` declaration.
     */
    conflictWith?: string;
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

  /**
   * Phase 12.2 — search/filter mode. Pressing `i` (reserved at chord
   * assignment time so it never doubles as a chord key) opens a search
   * input that filters the current level's nodes by label substring.
   * Filtered results get Cmd+1..Cmd+9 quick-select shortcuts; j/k still
   * navigate; Enter commits the highlighted match. Esc backs out to chord
   * mode at the same level.
   */
  let searchOpen = $state(false);
  let searchValue = $state("");
  let searchIdx = $state(0);
  let searchEl = $state<HTMLInputElement | null>(null);

  let currentLevel = $derived.by((): ChordNode[] => {
    let level = tree;
    for (const name of breadcrumb) {
      const found = level.find((n) => n.label === name);
      if (found?.children) level = found.children;
      else break;
    }
    return level;
  });

  /**
   * Recursive flattening for search mode. Walks every descendant of the
   * current breadcrumb level so typing "dude" surfaces `Status › Dude`
   * (a value buried two levels deep), not just top-level matches. Each
   * entry carries its parent path so we can render a "Path › Label"
   * label and, on select, descend into groups or fire leaf actions
   * directly (skipping intermediate breadcrumb hops).
   */
  type FlatEntry = { node: ChordNode; path: string[]; fullLabel: string };
  function flattenForSearch(level: ChordNode[], path: string[] = []): FlatEntry[] {
    const out: FlatEntry[] = [];
    for (const node of level) {
      const fullLabel = path.length === 0 ? node.label : `${path.join(" › ")} › ${node.label}`;
      out.push({ node, path: [...path], fullLabel });
      if (node.children) out.push(...flattenForSearch(node.children, [...path, node.label]));
    }
    return out;
  }
  let allEntries = $derived.by((): FlatEntry[] => {
    if (!searchOpen) return [];
    return flattenForSearch(currentLevel);
  });
  let filteredEntries = $derived.by((): FlatEntry[] => {
    if (!searchOpen) return [];
    const q = searchValue.trim().toLowerCase();
    if (!q) return allEntries;
    return allEntries.filter((e) => e.fullLabel.toLowerCase().includes(q));
  });
  // Clamp the highlight when the filter narrows.
  $effect(() => {
    if (searchIdx >= filteredEntries.length) searchIdx = Math.max(0, filteredEntries.length - 1);
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
    // Selection always exits search mode so the user sees the post-action
    // state (descended chord level / popover closed). Idempotent if search
    // was never opened.
    if (searchOpen) {
      searchOpen = false;
      searchValue = "";
      searchIdx = 0;
    }
  }
  /**
   * Phase 12.2 — selection target for a flattened search entry. For groups
   * we descend the breadcrumb to the entry's parent path before opening
   * the group, so the resulting view matches what the user expects (e.g.
   * search "Status" and select → land inside Status's value list, not at
   * the top level with the breadcrumb out of sync). For leaves we just
   * fire the action; the breadcrumb stays where it was since the menu
   * closes either way.
   */
  function handleSelectEntry(entry: FlatEntry) {
    if (entry.node.children) {
      breadcrumb = [...breadcrumb, ...entry.path, entry.node.label];
      searchOpen = false;
      searchValue = "";
      searchIdx = 0;
    } else {
      handleSelect(entry.node);
    }
  }

  function ascend() {
    if (inputNode) {
      inputNode = null;
      inputValue = "";
      return;
    }
    if (searchOpen) {
      searchOpen = false;
      searchValue = "";
      searchIdx = 0;
      return;
    }
    if (breadcrumb.length > 0) {
      breadcrumb = breadcrumb.slice(0, -1);
    } else {
      onclose();
    }
  }

  function openSearch() {
    searchOpen = true;
    searchValue = "";
    searchIdx = 0;
    setTimeout(() => searchEl?.focus(), 0);
  }

  function commitInput() {
    if (!inputNode?.input) return;
    inputNode.input.onSubmit(inputValue);
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || (e.key === "Backspace" && !inputNode && !searchOpen)) {
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
    if (searchOpen) {
      // Cmd+1..Cmd+9 quick-select Nth filtered match.
      if ((e.metaKey || e.ctrlKey) && /^[1-9]$/.test(e.key)) {
        const i = Number(e.key) - 1;
        const entry = filteredEntries[i];
        if (entry) {
          e.preventDefault();
          e.stopPropagation();
          handleSelectEntry(entry);
        }
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        const entry = filteredEntries[searchIdx];
        if (entry) handleSelectEntry(entry);
        return;
      }
      if (e.key === "ArrowDown" || (e.key === "j" && (e.metaKey || e.ctrlKey))) {
        e.preventDefault();
        e.stopPropagation();
        searchIdx = Math.min(filteredEntries.length - 1, searchIdx + 1);
        return;
      }
      if (e.key === "ArrowUp" || (e.key === "k" && (e.metaKey || e.ctrlKey))) {
        e.preventDefault();
        e.stopPropagation();
        searchIdx = Math.max(0, searchIdx - 1);
        return;
      }
      // All other keys flow to the search <input> for typing.
      return;
    }
    // `i` opens search/filter mode. Reserved at chord-assignment time so no
    // chord node ever owns this letter.
    if (e.key === "i") {
      e.preventDefault();
      e.stopPropagation();
      openSearch();
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
  {:else if searchOpen}
    <div class="chord-input-wrap">
      <input
        bind:this={searchEl}
        bind:value={searchValue}
        class="chord-input"
        type="text"
        placeholder="Filter…"
        autocomplete="off"
        spellcheck="false"
      />
      <div class="chord-input-help">
        <kbd class="chord-key">↵</kbd> select
        <kbd class="chord-key">⌘1-9</kbd> jump
        <kbd class="chord-key">Esc</kbd> back
      </div>
    </div>
    <div class="chord-list">
      {#each filteredEntries as entry, idx (entry.path.join("/") + "/" + entry.node.label)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="chord-row {idx === searchIdx ? 'chord-row--selected' : ''}"
          onclick={() => handleSelectEntry(entry)}
        >
          {#if idx < 9}
            <kbd class="chord-key">⌘{idx + 1}</kbd>
          {:else}
            <kbd class="chord-key">{entry.node.key}</kbd>
          {/if}
          <span class="chord-label">
            {#if entry.path.length > 0}
              <span class="chord-path">{entry.path.join(" › ")} ›</span>
            {/if}
            {entry.node.label}
          </span>
          {#if entry.node.hint}
            <kbd class="chord-hint" title="Alternative path">{entry.node.hint}</kbd>
          {/if}
          {#if entry.node.children || entry.node.input}
            <span class="chord-more">›</span>
          {/if}
        </div>
      {/each}
      {#if filteredEntries.length === 0}
        <div class="chord-empty">No matches</div>
      {/if}
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
          {#if node.conflictWith}
            <span
              class="chord-conflict"
              title="Preferred chord key was already claimed by '{node.conflictWith}'. Edit one of the property pages' chord_key: to resolve."
            >taken by {node.conflictWith}</span>
          {/if}
          {#if node.children || node.input}
            <span class="chord-more">›</span>
          {/if}
        </div>
      {/each}
      <div class="chord-search-hint">
        <kbd class="chord-key">i</kbd> filter
      </div>
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
  .chord-row--selected { background: color-mix(in srgb, var(--primary) 18%, transparent); }
  .chord-path {
    color: var(--v9-ink-faint);
    font-size: 11px;
    margin-right: 4px;
  }
  .chord-empty {
    padding: 6px 8px;
    color: var(--v9-ink-faint);
    font-style: italic;
  }
  .chord-search-hint {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 4px;
    padding: 4px 8px 0;
    border-top: 1px solid var(--v9-line);
    margin-top: 4px;
    color: var(--v9-ink-faint);
    font-size: 10px;
  }
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
  .chord-conflict {
    display: inline-block;
    padding: 1px 5px;
    background: color-mix(in srgb, var(--destructive, #c75) 14%, transparent);
    color: var(--destructive, #c75);
    border: 1px solid color-mix(in srgb, var(--destructive, #c75) 35%, transparent);
    border-radius: 3px;
    font-family: inherit;
    font-size: 10px;
    font-weight: 500;
  }
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

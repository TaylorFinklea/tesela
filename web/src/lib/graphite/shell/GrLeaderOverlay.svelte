<!-- web/src/lib/graphite/shell/GrLeaderOverlay.svelte -->
<script lang="ts">
  /*
   * Graphite leader (Space) chord overlay — NEW presentation over the
   * EXISTING chord tree + behavior. This mirrors
   * web/src/lib/components/ChordMenu.svelte's traversal exactly:
   *   - open/close + initial path come from the leader-tree store
   *     (isLeaderOpen / closeLeader / getLeaderInitialPath / getLeaderTree)
   *   - `currentLevel` is derived by walking the breadcrumb, matching nodes
   *     by `label` (same as ChordMenu's $derived currentLevel)
   *   - a keystroke matching a node's `key` either descends (push label) or
   *     runs node.action() + closeLeader(); node.input descends into a text
   *     input whose Enter calls onSubmit + closes (ChordMenu's handleSelect)
   *   - Esc / Backspace ascends one level, closing at root (ChordMenu.ascend)
   *   - capture-phase document keydown so cm-vim doesn't consume the keys
   * Only the markup + CSS (the mockup's `.gr-leader`) is new.
   */
  import { onMount } from 'svelte';
  import type { ChordNode } from '$lib/components/ChordMenu.svelte';
  import {
    isLeaderOpen,
    closeLeader,
    getLeaderInitialPath,
    getLeaderTree,
  } from '$lib/v5/leader-tree.svelte';

  const open = $derived(isLeaderOpen());
  const tree = $derived(getLeaderTree());

  // Seed the breadcrumb from the store's initial path each time it opens.
  let breadcrumb = $state<string[]>([]);
  let inputNode = $state<ChordNode | null>(null);
  let inputValue = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);

  let lastOpen = false;
  $effect(() => {
    if (open && !lastOpen) {
      breadcrumb = getLeaderInitialPath();
      inputNode = null;
      inputValue = '';
    }
    lastOpen = open;
  });

  const currentLevel = $derived.by((): ChordNode[] => {
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
      inputValue = node.input.initial ?? '';
      setTimeout(() => inputEl?.focus(), 0);
    } else if (node.action) {
      node.action();
      closeLeader();
    }
  }

  function commitInput() {
    if (!inputNode?.input) return;
    inputNode.input.onSubmit(inputValue);
    closeLeader();
  }

  function ascend() {
    if (inputNode) {
      inputNode = null;
      inputValue = '';
      return;
    }
    if (breadcrumb.length > 0) {
      breadcrumb = breadcrumb.slice(0, -1);
    } else {
      closeLeader();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape' || (e.key === 'Backspace' && !inputNode)) {
      e.preventDefault();
      e.stopPropagation();
      ascend();
      return;
    }
    if (inputNode) {
      if (e.key === 'Enter') {
        e.preventDefault();
        e.stopPropagation();
        commitInput();
        return;
      }
      // Other keys flow to the <input> normally.
      return;
    }
    // Match by exact key (case-sensitive) — Shift+letter is a distinct chord.
    const match = currentLevel.find((n) => n.key === e.key);
    if (match) {
      e.preventDefault();
      e.stopPropagation();
      handleSelect(match);
      return;
    }
    // Swallow every other keystroke while the menu is open — modal behavior,
    // identical to ChordMenu (arrows/vim chords must not leak to the editor).
    e.preventDefault();
    e.stopPropagation();
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown, true);
    return () => document.removeEventListener('keydown', handleKeydown, true);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="gr-scrim leader" onclick={() => closeLeader()}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="gr-leader" onclick={(e) => e.stopPropagation()}>
      <div class="gr-leader-head">
        <kbd>Space</kbd>
        {#each breadcrumb as crumb}
          <span class="sep">›</span>
          <span class="crumb">{crumb}</span>
        {/each}
        {#if inputNode}
          <span class="sep">›</span>
          <span class="crumb">{inputNode.label}</span>
        {/if}
      </div>

      {#if inputNode}
        <div class="gr-leader-input">
          <!-- svelte-ignore a11y_autofocus -->
          <input
            bind:this={inputEl}
            bind:value={inputValue}
            type="text"
            placeholder={inputNode.input?.placeholder ?? ''}
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      {:else}
        <div class="gr-leader-body">
          {#each currentLevel as node (node.key)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="gr-chord" onclick={() => handleSelect(node)}>
              <span class="key">{node.key}</span>
              <span class="cl">{node.label}</span>
              {#if node.children || node.input}
                <span class="more">›</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

      <div class="gr-leader-foot">
        {#if inputNode}
          ↵ set · esc back
        {:else}
          esc {breadcrumb.length > 0 ? 'back' : 'close'} · press a key
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .gr-scrim {
    position: absolute;
    inset: 0;
    z-index: 40;
    background: rgba(8, 9, 12, 0.58);
    backdrop-filter: blur(3px);
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .gr-scrim.leader {
    justify-content: flex-end;
  }
  .gr-leader {
    width: min(460px, 92%);
    margin-top: auto;
    margin-bottom: 46px;
    background: var(--raised);
    border: 1px solid var(--line-2);
    border-radius: 14px;
    box-shadow: 0 28px 90px rgba(0, 0, 0, 0.55);
    overflow: hidden;
  }
  .gr-leader-head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 13px 16px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--subtle);
  }
  .gr-leader-head kbd {
    color: var(--coral);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 2px 7px;
  }
  .gr-leader-head .sep {
    color: var(--faint);
  }
  .gr-leader-head .crumb {
    color: var(--fg2);
  }
  .gr-leader-body {
    padding: 8px;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
  }
  .gr-chord {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 9px 11px;
    border-radius: 9px;
    cursor: pointer;
  }
  .gr-chord:hover {
    background: var(--raised-2);
  }
  .gr-chord .key {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: var(--surface);
    border: 1px solid var(--line-2);
    font-family: var(--mono);
    font-size: 12px;
    color: var(--coral);
    flex-shrink: 0;
    font-weight: 600;
  }
  .gr-chord .cl {
    flex: 1;
    font-size: 13px;
    color: var(--fg2);
  }
  .gr-chord .more {
    color: var(--faint);
  }
  .gr-leader-input {
    padding: 12px 16px;
  }
  .gr-leader-input input {
    width: 100%;
    padding: 8px 10px;
    background: var(--bg);
    border: 1px solid var(--coral-line);
    border-radius: 8px;
    color: var(--fg);
    font-family: var(--sans);
    font-size: 13px;
    outline: none;
  }
  .gr-leader-foot {
    padding: 9px 16px;
    border-top: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
  }
</style>

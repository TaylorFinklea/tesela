<script lang="ts">
  let {
    initialDsl,
    onSave,
    onCancel,
  }: {
    initialDsl: string;
    onSave: (dsl: string) => void;
    onCancel: () => void;
  } = $props();

  let draft = $state(initialDsl);

  function commit() {
    const trimmed = draft.trim();
    if (trimmed.length === 0) return; // refuse empty — would match everything
    onSave(trimmed);
  }

  function handleKey(e: KeyboardEvent) {
    // ⌘↵ commits; Esc cancels — mirrors the rest of the app's sheet
    // affordances. Plain Enter inserts a newline (textarea default)
    // so the user can format multi-line drafts during a heavy edit.
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm"
  onclick={onCancel}
>
  <div
    class="bg-card border border-border rounded-lg shadow-xl w-[min(640px,92vw)] p-4"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="flex items-center justify-between mb-3">
      <h2 class="text-[13px] font-semibold">Edit Inbox query</h2>
      <span class="text-[10px] text-muted-foreground font-mono">⌘↵ save · Esc cancel</span>
    </header>
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      class="w-full h-32 p-2 rounded border border-border bg-background text-[13px] font-mono outline-none focus:border-accent resize-y"
      bind:value={draft}
      onkeydown={handleKey}
      autofocus
      spellcheck="false"
    ></textarea>
    <div class="mt-2 text-[11px] text-muted-foreground/70 leading-relaxed">
      <p>
        JQL-style. <code>kind:block</code> implicit. Example:
        <code>status != done AND type IN (task, issue) AND scheduled IS NOT NULL ORDER BY scheduled DESC</code>.
      </p>
      <details class="mt-1.5">
        <summary class="cursor-pointer hover:text-foreground select-none">grammar reference</summary>
        <div class="mt-1.5 grid grid-cols-[max-content_1fr] gap-x-3 gap-y-0.5 font-mono text-[10.5px]">
          <span class="text-foreground/80">combinators</span>
          <code>AND  OR  NOT  ( )</code>
          <span class="text-foreground/80">compare</span>
          <code>=  !=  &lt;  &lt;=  &gt;  &gt;=</code>
          <span class="text-foreground/80">membership</span>
          <code>key IN (a, b, c)   key NOT IN (…)</code>
          <span class="text-foreground/80">presence</span>
          <code>key IS NULL   key IS NOT NULL   (EMPTY alias)</code>
          <span class="text-foreground/80">range</span>
          <code>key BETWEEN a AND b   (inclusive)</code>
          <span class="text-foreground/80">pattern</span>
          <code>text LIKE "wood%"   key NOT LIKE "…"   (% any, _ one)</code>
          <span class="text-foreground/80">sort</span>
          <code>ORDER BY key [ASC|DESC] [, key2 …]</code>
          <span class="text-foreground/80">keys</span>
          <code>tag/type · status · has · is · on · page · block · text · &lt;property&gt;</code>
          <span class="text-foreground/80">legacy</span>
          <code>key:value · -key:value · has:foo · tag-in:a,b,c</code>
        </div>
      </details>
    </div>
    <footer class="mt-3 flex justify-end gap-2">
      <button
        type="button"
        class="px-3 py-1 rounded border border-muted-foreground/20 text-[12px] hover:border-muted-foreground/40"
        onclick={onCancel}
      >Cancel</button>
      <button
        type="button"
        class="px-3 py-1 rounded bg-accent text-accent-foreground text-[12px] hover:opacity-90"
        onclick={commit}
      >Save</button>
    </footer>
  </div>
</div>

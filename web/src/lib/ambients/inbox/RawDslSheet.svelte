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
    <p class="mt-2 text-[11px] text-muted-foreground/70">
      DSL clauses, whitespace-separated. <code>kind:block</code> is implicit.
      Try <code>-has:status -is:heading -on:daily-page -tag:reference</code>.
    </p>
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

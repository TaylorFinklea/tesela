<!-- web/src/lib/components/QueryInput.svelte — the ONE shared JQL authoring
     widget (tesela-vp9.2; design source: `.docs/ai/phases/2026-07-07-jql-
     authoring-spec.md`, decisions 1 + 4). Mounted by GrInbox's saved-view
     editor, QueryBlock's inline query editor, and RawDslSheet's modal
     textarea — replacing each surface's own divergent input/popup wiring.

     Syntax highlighting is the overlay technique: a transparent-text real
     `<input>`/`<textarea>` (captures typing, caret, selection) stacked
     exactly on top of a colored-span underlay built from `tokenize()`
     output (`query-input/overlay-spans.ts`). Diagnostics
     (`parseQueryWithDiagnostics`, debounced ~150ms) add underline spans to
     that same underlay and populate the hint row below. Completion is a
     three-tier popup (key / operator / value — `query-input/caret-
     context.ts` + `query-input/completion.ts`) rendered via the repo's
     existing hand-rolled `AutocompleteMenu`. -->
<script lang="ts">
  import { tick } from "svelte";
  import { parseQueryWithDiagnostics, type Diagnostic } from "$lib/query-language";
  import { buildOverlaySpans } from "$lib/query-input/overlay-spans";
  import { caretContext, type CaretContext } from "$lib/query-input/caret-context";
  import { buildCompletions, type CompletionSources } from "$lib/query-input/completion";
  import { createDebouncer } from "$lib/query-input/debounce";
  import AutocompleteMenu, { type AutocompleteItem } from "./AutocompleteMenu.svelte";

  const DIAGNOSTICS_DEBOUNCE_MS = 150;
  const EMPTY_CTX: CaretContext = { tier: "none", from: 0, to: 0, prefix: "", key: null };
  const EMPTY_SOURCES: CompletionSources = { properties: [], types: [] };

  let {
    value = $bindable(""),
    placeholder = "",
    multiline = false,
    compact = false,
    autofocus = false,
    sources = EMPTY_SOURCES,
    oncommit,
    oncancel,
    onblur,
  }: {
    /** Two-way bound source text — `bind:value`, same as a plain `<input>`. */
    value?: string;
    placeholder?: string;
    /** Renders a `<textarea>` (RawDslSheet) instead of a single-line
     *  `<input>` (GrInbox, QueryBlock). Also swaps the commit key from
     *  plain Enter to Cmd/Ctrl+Enter so Enter can insert a newline. */
    multiline?: boolean;
    /** Smaller font/padding profile — QueryBlock's inline editing UX. */
    compact?: boolean;
    autofocus?: boolean;
    /** Property/type sources for the value-tier completion (spec decision
     *  4). Each host fetches these from `api.listProperties()` /
     *  `api.listTypes()`. */
    sources?: CompletionSources;
    /** Fires on the commit key (Enter, or Cmd/Ctrl+Enter when multiline)
     *  when the completion popup isn't consuming it. The host owns
     *  save-gating (e.g. GrInbox's `canSave`) — QueryInput always fires. */
    oncommit?: (value: string) => void;
    /** Fires on Escape once the completion popup is closed/absent. */
    oncancel?: () => void;
    /** Fires on blur (after a short delay so a popup-item click can land
     *  first). Only QueryBlock uses this today (commit-on-blur). */
    onblur?: (value: string) => void;
  } = $props();

  let inputEl = $state<HTMLInputElement | HTMLTextAreaElement | undefined>();
  let overlayEl = $state<HTMLDivElement | undefined>();
  let autocompleteRef = $state<AutocompleteMenu | null>(null);

  /** Exposed for hosts that programmatically insert text (e.g. GrInbox's
   *  chip-as-inserter row) and want to return focus to the field. */
  export function focus() {
    inputEl?.focus();
  }

  // ── syntax highlighting (synchronous — tokenize() is cheap) ───────────
  let debouncedDiagnostics = $state<Diagnostic[]>([]);
  const diagnosticsDebouncer = createDebouncer((v: string) => {
    debouncedDiagnostics = parseQueryWithDiagnostics(v).diagnostics;
  }, DIAGNOSTICS_DEBOUNCE_MS);
  $effect(() => {
    diagnosticsDebouncer.call(value);
    return () => diagnosticsDebouncer.cancel();
  });
  const spans = $derived(buildOverlaySpans(value, debouncedDiagnostics));
  const hint = $derived(debouncedDiagnostics[0]?.hint ?? null);

  // ── completion popup ───────────────────────────────────────────────────
  let showCompletion = $state(false);
  let ctx = $state<CaretContext>(EMPTY_CTX);
  let items = $state<AutocompleteItem[]>([]);
  let popupPos = $state({ x: 0, y: 0 });

  function closeCompletion() {
    showCompletion = false;
  }

  function refreshCompletion() {
    if (!inputEl) {
      closeCompletion();
      return;
    }
    const cursor = inputEl.selectionStart ?? value.length;
    const c = caretContext(value, cursor);
    if (c.tier === "none") {
      closeCompletion();
      return;
    }
    const built = buildCompletions(c, sources);
    if (built.length === 0) {
      closeCompletion();
      return;
    }
    ctx = c;
    items = built;
    // Anchored at the input's own box (not per-caret) — simple and robust
    // for a short single-line/short-multiline widget; BlockEditor's
    // CodeMirror-based autocomplete tracks the caret via `coordsAtPos`,
    // which has no plain-`<input>`/`<textarea>` equivalent without a
    // mirror-div measurement hack. Matches GrInbox's pre-vp9.2 `gr-vsuggest`
    // popup, which also anchored below the whole input.
    const rect = inputEl.getBoundingClientRect();
    popupPos = { x: rect.left, y: rect.bottom + 4 };
    showCompletion = true;
  }

  async function acceptItem(item: AutocompleteItem) {
    const before = value.slice(0, ctx.from);
    const after = value.slice(ctx.to);
    const insert = `${item.label} `;
    value = before + insert + after;
    const caretAfter = before.length + insert.length;
    closeCompletion();
    await tick();
    inputEl?.focus();
    inputEl?.setSelectionRange(caretAfter, caretAfter);
  }

  // ── input wiring ────────────────────────────────────────────────────────
  function syncScroll() {
    if (!inputEl || !overlayEl) return;
    overlayEl.scrollLeft = inputEl.scrollLeft;
    overlayEl.scrollTop = inputEl.scrollTop;
  }

  function handleInput() {
    syncScroll();
    refreshCompletion();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (showCompletion && autocompleteRef?.handleKeydown(e)) return;
    if (e.key === "Escape") {
      e.preventDefault();
      oncancel?.();
      return;
    }
    if (multiline) {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        oncommit?.(value);
      }
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      oncommit?.(value);
    }
  }

  function handleBlur() {
    // Deferred: a click on the popup moves focus (blur) before its own
    // "click" fires — give that a chance to land before tearing down
    // state or firing the host's commit-on-blur.
    setTimeout(() => {
      closeCompletion();
      onblur?.(value);
    }, 120);
  }
</script>

<div class="qi-wrap" class:qi-multiline={multiline} class:qi-compact={compact}>
  <div class="qi-overlay" bind:this={overlayEl} aria-hidden="true">
    {#each spans as s (s.start)}
      {#if s.role === "text"}<span>{s.text}</span
        >{:else}<span class="qi-tok qi-{s.role}" class:qi-diag={s.diagnostic}>{s.text}</span
        >{/if}
    {/each}
  </div>
  {#if multiline}
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      bind:this={inputEl}
      class="qi-input"
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
      {placeholder}
      {autofocus}
      bind:value
      oninput={handleInput}
      onclick={refreshCompletion}
      onkeydown={handleKeydown}
      onblur={handleBlur}
      onscroll={syncScroll}
    ></textarea>
  {:else}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      bind:this={inputEl}
      type="text"
      class="qi-input"
      spellcheck="false"
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      {placeholder}
      {autofocus}
      bind:value
      oninput={handleInput}
      onclick={refreshCompletion}
      onkeydown={handleKeydown}
      onblur={handleBlur}
      onscroll={syncScroll}
    />
  {/if}
  {#if showCompletion && items.length > 0}
    <AutocompleteMenu
      bind:this={autocompleteRef}
      {items}
      filter={ctx.prefix}
      position={popupPos}
      onselect={acceptItem}
      onclose={closeCompletion}
    />
  {/if}
</div>
{#if hint}
  <div class="qi-hint">{hint}</div>
{/if}

<style>
  /* Chrome (background/border/focus ring) lives on the WRAPPER, never on
     `.qi-overlay` or `.qi-input` — both of those must stay background-
     transparent so the overlay's colored text shows through the input
     stacked on top of it. (An opaque `.qi-input` background would hide
     the underlay completely.) */
  .qi-wrap {
    position: relative;
    width: 100%;
    border-radius: 8px;
    background: var(--raised, var(--surface-2));
    border: 1px solid var(--border, var(--line-2));
  }
  .qi-wrap:focus-within {
    border-color: var(--primary, var(--coral-line));
  }
  .qi-overlay,
  .qi-input {
    box-sizing: border-box;
    margin: 0;
    width: 100%;
    padding: 0 10px;
    border: none;
    background: transparent;
    font-family: var(--theme-font-mono, var(--mono, ui-monospace, monospace));
    font-size: 12px;
    line-height: 28px;
    white-space: pre;
    overflow: hidden;
  }
  .qi-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    user-select: none;
  }
  .qi-input {
    position: relative;
    height: 28px;
    color: transparent;
    caret-color: var(--foreground, var(--fg));
    outline: none;
  }
  .qi-input::placeholder {
    color: var(--muted-foreground, var(--faint));
  }

  .qi-multiline .qi-overlay,
  .qi-multiline .qi-input {
    white-space: pre-wrap;
    word-wrap: break-word;
    height: auto;
    min-height: 128px;
    line-height: 1.5;
    overflow: auto;
  }

  .qi-compact .qi-overlay,
  .qi-compact .qi-input {
    font-size: 11px;
    line-height: 18px;
    padding: 2px 6px;
  }
  .qi-compact .qi-input {
    height: 18px;
  }
  .qi-compact {
    border-radius: 4px;
  }

  .qi-tok.qi-key {
    color: var(--type-project, #6a8fdc);
  }
  .qi-tok.qi-keyword {
    color: var(--primary, var(--coral, #e07a5f));
    font-weight: 600;
  }
  .qi-tok.qi-operator {
    color: var(--type-event, #6dbacc);
  }
  .qi-tok.qi-value {
    color: var(--foreground, var(--fg));
  }
  .qi-tok.qi-paren,
  .qi-tok.qi-comma {
    color: var(--muted-foreground, var(--subtle));
  }
  .qi-tok.qi-diag {
    text-decoration: underline wavy var(--destructive, var(--task, #db6c83));
    text-decoration-thickness: 1.5px;
    text-underline-offset: 2px;
  }

  .qi-hint {
    margin-top: 4px;
    font-family: var(--theme-font-mono, var(--mono, monospace));
    font-size: 11px;
    color: var(--destructive, var(--task, #db6c83));
  }
</style>

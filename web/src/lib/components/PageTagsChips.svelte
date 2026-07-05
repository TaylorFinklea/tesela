<script lang="ts">
  /**
   * Page-level tag chips strip (tag-system Phase 12).
   *
   * Renders the focused page's frontmatter `tags: [...]` as removable
   * chips at the top of the page. A trailing `+` button opens a small
   * picker:
   *   - filtered list of existing `type: tag` pages
   *   - "Create new tag" row when the filter has any text and no match
   *
   * On any change, rewrites the page's `tags:` frontmatter line and
   * dispatches `onContentChange(newFullContent)`.
   *
   * Used by `NoteRenderer` above the per-type renderer for note/daily/
   * scratch/tag pages. Hidden for query/property pages (which manage
   * their own frontmatter elsewhere).
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";

  let {
    note,
    onContentChange,
  }: {
    note: Note;
    onContentChange: (newContent: string) => void;
  } = $props();

  // ── derive the current tags + an editable copy ───────────────────────────
  // The chip strip is the source of truth between edits. Each tag add/remove
  // mutates `editing` first, then writes back via the frontmatter rewrite.

  const tags = $derived(note.metadata.tags ?? []);

  // ── tag autocomplete data ────────────────────────────────────────────────
  // Pulls the full notes list and filters to `type: tag` pages. Same
  // contract as the `#` autocomplete in BlockEditor; kept independent so
  // this chip strip can mount without the cm-editor.
  // Raised 500→5000 (tesela-sclr.1): a 500 cap silently hid `type: tag`
  // pages created after the 500th note from tag autocomplete.
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const allNotes = $derived((notesQuery.data ?? []) as Note[]);
  const tagPages = $derived(
    allNotes.filter((n) => (n.metadata.note_type ?? "").toLowerCase() === "tag"),
  );

  // ── picker state ──────────────────────────────────────────────────────────
  let pickerOpen = $state(false);
  let pickerFilter = $state("");
  let pickerInputEl = $state<HTMLInputElement | null>(null);

  /** Filtered + sorted suggestions for the picker. */
  const suggestions = $derived.by(() => {
    const filter = pickerFilter.trim().toLowerCase();
    const activeSet = new Set(tags.map((t) => t.toLowerCase()));
    return tagPages
      .filter((n) => !activeSet.has(n.title.toLowerCase()))
      .filter((n) =>
        filter ? n.title.toLowerCase().includes(filter) : true,
      )
      .slice(0, 20)
      .map((n) => n.title);
  });

  const showCreateRow = $derived.by(() => {
    const f = pickerFilter.trim().toLowerCase();
    if (!f) return false;
    return !tagPages.some((n) => n.title.toLowerCase() === f);
  });

  function openPicker() {
    pickerOpen = true;
    pickerFilter = "";
    queueMicrotask(() => pickerInputEl?.focus());
  }

  function closePicker() {
    pickerOpen = false;
    pickerFilter = "";
  }

  // ── frontmatter rewrite ──────────────────────────────────────────────────

  /** Replace the `tags:` line in `content`'s YAML frontmatter with the
   *  new array. If no `tags:` line exists, insert one right after the
   *  opening `---`. */
  function withRewrittenTags(content: string, nextTags: string[]): string {
    const yamlLine = nextTags.length === 0
      ? "tags: []"
      : `tags: [${nextTags.map((t) => `"${t}"`).join(", ")}]`;

    if (!content.startsWith("---\n") && !content.startsWith("---\r\n")) {
      // No frontmatter at all — synthesize a minimal one.
      return `---\n${yamlLine}\n---\n${content}`;
    }

    // Find the `---` closer.
    const afterOpen = content.indexOf("\n") + 1;
    const rest = content.slice(afterOpen);
    const closerOffset = rest.indexOf("\n---");
    if (closerOffset < 0) {
      // Malformed frontmatter; bail.
      return content;
    }
    const fmEnd = afterOpen + closerOffset;
    const headLines = content.slice(0, fmEnd).split("\n");
    const tail = content.slice(fmEnd);

    let replaced = false;
    const newHead = headLines.map((line) => {
      if (replaced) return line;
      if (/^tags\s*:/.test(line.trim())) {
        replaced = true;
        return yamlLine;
      }
      return line;
    });

    if (!replaced) {
      // Insert right after the opening `---` line.
      newHead.splice(1, 0, yamlLine);
    }

    return newHead.join("\n") + tail;
  }

  function addTag(name: string) {
    const clean = name.trim().toLowerCase();
    if (!clean) return;
    if (tags.some((t) => t.toLowerCase() === clean)) {
      closePicker();
      return;
    }
    const next = [...tags, clean];
    onContentChange(withRewrittenTags(note.content, next));
    closePicker();
  }

  function removeTag(name: string) {
    const target = name.toLowerCase();
    const next = tags.filter((t) => t.toLowerCase() !== target);
    if (next.length === tags.length) return;
    onContentChange(withRewrittenTags(note.content, next));
  }

  function openTagPage(name: string) {
    // Same routing as the sidebar TagsSurface: the tag's NoteId is its
    // lowercased slug. Phase 1's BufferShell handler picks it up.
    document.dispatchEvent(
      new CustomEvent("tesela:open-tag", { detail: { value: name.toLowerCase() } }),
    );
  }

  function onPickerKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closePicker();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (suggestions.length > 0) {
        addTag(suggestions[0]);
      } else if (showCreateRow) {
        addTag(pickerFilter);
      }
    }
  }
</script>

<div class="page-tags-strip">
  {#each tags as t (t)}
    <span class="chip">
      <button
        type="button"
        class="chip-name"
        title="open #{t}"
        onclick={() => openTagPage(t)}
      >#{t}</button>
      <button
        type="button"
        class="chip-remove"
        title="remove #{t} from page"
        onclick={() => removeTag(t)}
      >×</button>
    </span>
  {/each}

  <div class="picker-wrap">
    <button
      type="button"
      class="add-button"
      title="add tag"
      onclick={() => (pickerOpen ? closePicker() : openPicker())}
    >+</button>

    {#if pickerOpen}
      <div class="picker">
        <input
          type="text"
          class="picker-input"
          placeholder="filter or create…"
          bind:value={pickerFilter}
          bind:this={pickerInputEl}
          onkeydown={onPickerKey}
        />
        <ul class="picker-list">
          {#each suggestions as s (s)}
            <li>
              <button type="button" onclick={() => addTag(s)}>#{s}</button>
            </li>
          {/each}
          {#if showCreateRow}
            <li class="create-row">
              <button
                type="button"
                onclick={() => addTag(pickerFilter)}
              >+ create "{pickerFilter.trim()}"</button>
            </li>
          {/if}
          {#if suggestions.length === 0 && !showCreateRow}
            <li class="muted">no matching tags</li>
          {/if}
        </ul>
      </div>
    {/if}
  </div>
</div>

<style>
  .page-tags-strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 8px 0 12px 0;
    font-family: var(--theme-font-mono);
    font-size: 11px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    background: color-mix(in srgb, var(--primary) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--primary) 30%, transparent);
    border-radius: 10px;
    overflow: hidden;
  }
  .chip-name {
    background: transparent;
    border: 0;
    color: var(--primary);
    padding: 1px 4px 1px 10px;
    cursor: pointer;
    font-family: inherit;
    font-size: inherit;
  }
  .chip-name:hover {
    text-decoration: underline;
  }
  .chip-remove {
    background: transparent;
    border: 0;
    color: var(--primary);
    opacity: 0.5;
    padding: 1px 8px 1px 4px;
    cursor: pointer;
    font-size: 13px;
    line-height: 1;
  }
  .chip-remove:hover {
    opacity: 1;
  }
  .picker-wrap {
    position: relative;
    display: inline-block;
  }
  .add-button {
    background: transparent;
    border: 1px dashed var(--line);
    color: var(--fg-faint);
    border-radius: 10px;
    padding: 1px 10px;
    cursor: pointer;
    font-family: inherit;
    font-size: inherit;
  }
  .add-button:hover {
    border-color: var(--primary);
    color: var(--primary);
  }
  .picker {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 50;
    background: var(--v9-bg, var(--bg, white));
    border: 1px solid var(--line-soft);
    border-radius: 6px;
    padding: 4px;
    width: 220px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
  }
  .picker-input {
    width: 100%;
    box-sizing: border-box;
    background: transparent;
    border: 1px solid var(--line-soft);
    border-radius: 4px;
    padding: 3px 6px;
    color: var(--fg-default);
    font-family: inherit;
    font-size: 11px;
    margin-bottom: 4px;
  }
  .picker-input:focus {
    outline: none;
    border-color: var(--primary);
  }
  .picker-list {
    list-style: none;
    padding: 0;
    margin: 0;
    max-height: 180px;
    overflow-y: auto;
  }
  .picker-list li {
    display: block;
  }
  .picker-list button {
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    color: var(--fg-muted);
    padding: 3px 6px;
    cursor: pointer;
    font-family: inherit;
    font-size: 11px;
    border-radius: 3px;
  }
  .picker-list button:hover {
    background: var(--bg-2);
    color: var(--fg-default);
  }
  .create-row button {
    color: var(--primary);
  }
  .picker-list .muted {
    color: var(--fg-faint);
    font-size: 11px;
    padding: 3px 6px;
  }
</style>

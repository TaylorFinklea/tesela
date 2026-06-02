<!-- web/src/lib/graphite/views/GrPage.svelte — Part A, Task A3.
     Page / project outliner. REUSES BlockOutliner (the CodeMirror editing
     engine) untouched, fetching + saving the note via the same TanStack
     pattern BufferShell uses (createQuery + 500ms-debounced api.updateNote
     with optimistic setQueryData). The Graphite block look comes from A1's
     variable remap + decoration overrides (graphite-editor.css).

     Layout: a focus pane (title + GrTypeTag + meta head, `.gr-outline`
     body hosting BlockOutliner) beside a side pane of linked references
     (`.gr-refcard`s from api.getBacklinks) + a page-properties list
     (`.gr-proplist`). BlockOutliner, the API, and the buffer store are
     imported READ-ONLY. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import GrTypeTag from "$lib/graphite/GrTypeTag.svelte";
  import GrIcon from "$lib/graphite/GrIcon.svelte";

  let { pageId, paneId }: { pageId: string; paneId?: string } = $props();

  const queryClient = useQueryClient();

  // ── note fetch (mirrors BufferShell) ──────────────────────────────────
  const noteQuery = createQuery(() => ({
    queryKey: ["note", pageId] as const,
    queryFn: () => api.getNote(pageId),
    enabled: !!pageId,
  }));
  const note = $derived(noteQuery.data as Note | undefined);

  // ── backlinks for the linked-refs side pane ───────────────────────────
  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", pageId] as const,
    queryFn: () => api.getBacklinks(pageId),
    enabled: !!pageId,
  }));
  const backlinks = $derived((backlinksQuery.data ?? []) as Link[]);

  // ── frontmatter / body split (mirrors NoteRenderer.splitContent) ───────
  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return {
      frontmatter: content.slice(0, fmEnd) + "\n",
      body: afterFm.slice(bodyStart),
    };
  }
  const split = $derived(note ? splitContent(note.content) : { frontmatter: "", body: "" });

  const noteType = $derived((note?.metadata.note_type ?? "note").toLowerCase());
  const tags = $derived(note?.metadata.tags ?? []);

  // Page-level properties for the side `.gr-proplist`. Pulls the flat
  // string fields from frontmatter `custom` (the freeform property bag).
  const pageProps = $derived.by<Array<{ k: string; v: string }>>(() => {
    const custom = note?.metadata.custom ?? {};
    const out: Array<{ k: string; v: string }> = [];
    for (const [k, raw] of Object.entries(custom)) {
      if (raw == null) continue;
      const v = typeof raw === "string" ? raw : JSON.stringify(raw);
      out.push({ k, v });
    }
    return out;
  });

  // ── debounced save (mirrors BufferShell) ───────────────────────────────
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: AbortController | null = null;
  let pending: string | null = null;
  // Edit BASE for the pending save (body the outliner last reseeded from),
  // sent as `base_content` so the server diffs the author's real changes and
  // never re-asserts an untouched block over a concurrent peer edit. First
  // base of the window wins; cleared on flush.
  let pendingBase: string | undefined = undefined;

  function handleContentChange(fullContent: string, baseContent?: string) {
    pending = fullContent;
    if (pendingBase === undefined) pendingBase = baseContent;
    setSaving();
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flushSave(), 500);
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (pending === null || !pageId) return;
    const content = pending;
    pending = null;
    const base = pendingBase;
    pendingBase = undefined;
    if (inFlight) inFlight.abort();
    const controller = new AbortController();
    inFlight = controller;
    if (note) queryClient.setQueryData(["note", pageId], { ...note, content });
    try {
      const updated = await api.updateNote(pageId, content, base, controller.signal);
      if (controller.signal.aborted) return;
      queryClient.setQueryData(["note", pageId], updated);
      setSaved();
    } catch (e) {
      if ((e as { name?: string })?.name === "AbortError") return;
      // Surface the failure instead of swallowing it. Keep the optimistic
      // cache (the user's only live copy of the unsaved edit) — do NOT roll
      // back, which would feed pre-edit content into the live editor and
      // destroy in-progress work. Mirrors BufferShell's setSaveError path.
      console.error("GrPage save failed:", e);
      setSaveError(e instanceof Error ? e.message : "Unknown error");
      toast("Failed to save page", "error");
    } finally {
      if (inFlight === controller) inFlight = null;
    }
  }

  async function cancelAndFlush(fullContent: string, baseContent?: string) {
    pending = fullContent;
    if (baseContent !== undefined) pendingBase = baseContent;
    await flushSave();
  }

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
    if (inFlight) inFlight.abort();
  });

  function openRef(target: string) {
    openPageInFocused(asPageId(target));
  }
</script>

<div class="gr-pane focus">
  <div class="gr-pane-head">
    <span class="ttl">{note?.title || pageId || "Untitled page"}</span>
    <GrTypeTag type={noteType === "tag" ? "person" : "project"}>{noteType}</GrTypeTag>
    <span class="sp"></span>
    {#if note}
      <span class="meta">{tags.length ? tags.map((t) => `#${t}`).join(" ") : ""}</span>
    {/if}
  </div>

  <div class="gr-outline">
    {#if noteQuery.isLoading}
      <div class="gr-empty">loading…</div>
    {:else if noteQuery.isError}
      <div class="gr-empty">could not load {pageId}</div>
    {:else if note}
      {#key pageId}
        <BlockOutliner
          noteId={note.id}
          body={split.body}
          frontmatter={split.frontmatter}
          {paneId}
          onContentChange={handleContentChange}
          onCancelAndFlush={cancelAndFlush}
        />
      {/key}
    {/if}
  </div>
</div>

<div class="gr-pane side">
  <div class="gr-pane-head">
    <span class="ttl side-ttl">References</span>
    <span class="sp"></span>
    <span class="meta">{backlinks.length}</span>
  </div>
  <div class="gr-side-body">
    {#if backlinks.length === 0}
      <div class="gr-empty">No linked references</div>
    {:else}
      {#each backlinks as ref (ref.target + ":" + ref.position)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="gr-refcard" onclick={() => openRef(ref.target)}>
          <div class="src">
            <GrIcon name="link" size={13} />
            <span>{ref.target}</span>
          </div>
          {#if ref.text}<div class="snip">{ref.text}</div>{/if}
        </div>
      {/each}
    {/if}

    {#if pageProps.length > 0}
      <div class="gr-proplist">
        <div class="ph">Properties</div>
        {#each pageProps as p (p.k)}
          <div class="gr-prow">
            <span class="chord"></span>
            <span class="k">{p.k}</span>
            <span class="v">{p.v}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Panes mirror GrPane's `.focus` / `.side` flex ratios so GrPage drops
     into the shell's `.gr-main` flex row as a focus+side pair. */
  .gr-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    min-height: 0;
  }
  .gr-pane.focus {
    flex: 1.7;
  }
  .gr-pane.side {
    flex: 1;
    background: var(--surface);
    border-left: 1px solid var(--line);
    max-width: 420px;
  }
  .gr-pane-head {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 14px 18px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .gr-pane-head .ttl {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gr-pane-head .side-ttl {
    font-size: 13px;
    color: var(--fg2);
  }
  .gr-pane-head .sp {
    flex: 1;
  }
  .gr-pane-head .meta {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    white-space: nowrap;
  }
  .gr-outline {
    flex: 1;
    overflow: auto;
    padding: 14px 18px;
    min-height: 0;
  }
  .gr-empty {
    color: var(--faint);
    font-family: var(--mono);
    font-size: 12px;
    padding: 8px 2px;
  }

  /* Linked-refs side pane (verbatim Graphite CSS). */
  .gr-side-body {
    flex: 1;
    overflow: auto;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .gr-refcard {
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--raised);
    border: 1px solid var(--line);
    cursor: pointer;
  }
  .gr-refcard .src {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 11px;
    color: var(--fg2);
  }
  .gr-refcard .snip {
    font-size: 12.5px;
    color: var(--muted);
    margin-top: 5px;
    line-height: 1.4;
  }
  .gr-refcard .snip :global(em) {
    background: var(--coral-dim);
    color: var(--coral);
    font-style: normal;
    padding: 0 2px;
    border-radius: 3px;
  }

  /* Properties list (verbatim Graphite CSS). */
  .gr-proplist .ph {
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
    padding: 4px 2px 7px;
  }
  .gr-prow {
    display: grid;
    grid-template-columns: 18px 84px 1fr;
    align-items: center;
    gap: 8px;
    padding: 6px 7px;
    border-radius: 7px;
  }
  .gr-prow:hover {
    background: var(--raised);
  }
  .gr-prow .chord {
    font-family: var(--mono);
    font-size: 9.5px;
    text-align: center;
    color: var(--subtle);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 2px 0;
  }
  .gr-prow .k {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--subtle);
  }
  .gr-prow .v {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg2);
  }
</style>

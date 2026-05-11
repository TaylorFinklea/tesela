<script lang="ts">
  /**
   * Phase 9.6 — Logseq-style continuous journal.
   *
   * Renders a vertical scroll of daily-tagged notes (today on top, older days
   * below). Each section is its own editable BlockOutliner with a per-noteId
   * save handler. On mount, scrolls to `anchorDate` so /p/<YYYY-MM-DD>
   * lands the user at that date with surrounding context above and below.
   *
   * Today is always rendered, even if `notes/<today>.md` doesn't exist yet —
   * an upfront `getDailyNote()` call auto-creates it. Same for any
   * non-default `anchorDate` that doesn't yet exist on disk.
   *
   * Initial fetch loads the most recent ~30 daily notes; the rest reveal as
   * the user scrolls past the bottom (in-memory pagination, no extra API
   * call required because `listNotes({ tag: "daily" })` returns the full set
   * up to its limit).
   */
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { untrack } from "svelte";
  import { api } from "$lib/api-client";
  import BlockOutliner, { markNextFocusAsCrossNav } from "$lib/components/BlockOutliner.svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";
  import type { Note } from "$lib/types/Note";

  let { anchorDate }: { anchorDate: string } = $props();

  const queryClient = useQueryClient();

  // Use the user's LOCAL date — toISOString() is UTC, so in evening PST
  // it would already roll over to the next day's daily and surface as
  // "Today" before the user's wall-clock midnight.
  const todayStr = (() => {
    const d = new Date();
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  })();

  // Server-side fetch limit grows lazily as the user scrolls. We start
  // at ~60 (2x the on-screen window) so the very first render only
  // has to parse / mount that many notes — critical for graphs with
  // hundreds of imported daily entries where `limit: 500` was loading
  // tens of MB of JSON + spinning up hundreds of CodeMirror editors
  // before the page could even render.
  let fetchLimit = $state(60);
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { tag: "daily", limit: fetchLimit }] as const,
    queryFn: () => api.listNotes({ tag: "daily", limit: fetchLimit }),
  }));

  // Sort descending by title (which is the YYYY-MM-DD date for dailies).
  const dailies: Note[] = $derived(
    ((notesQuery.data ?? []) as Note[])
      .filter((n) => /^\d{4}-\d{2}-\d{2}$/.test(n.title))
      .sort((a, b) => b.title.localeCompare(a.title)),
  );

  // Make sure both today and the anchor are present even if the file doesn't
  // exist on disk yet. Also ensures today ends with an empty trailing bullet
  // block so the auto-focus path always lands on a "ready to type" block
  // instead of the front of an existing block. This fires once per
  // (anchor, today) combo.
  let ensuredFor = $state<string>("");
  $effect(() => {
    const need = `${anchorDate}|${todayStr}`;
    if (ensuredFor === need) return;
    if (!notesQuery.data) return;
    untrack(() => {
      ensuredFor = need;
      const haveToday = dailies.some((n) => n.title === todayStr);
      const haveAnchor = dailies.some((n) => n.title === anchorDate);
      const tasks: Promise<unknown>[] = [];
      if (!haveToday) tasks.push(api.getDailyNote(todayStr));
      if (!haveAnchor && anchorDate !== todayStr && /^\d{4}-\d{2}-\d{2}$/.test(anchorDate)) {
        tasks.push(api.getDailyNote(anchorDate));
      }
      Promise.all(tasks)
        .then(() => ensureTrailingEmpty(todayStr))
        .then((didChange) => {
          if (tasks.length > 0 || didChange) {
            queryClient.invalidateQueries({ queryKey: ["notes"] });
            // Re-arm the anchor scroll/focus effect so it re-runs after the
            // new trailing block lands in the DOM. Without this, the focus
            // path raced the PUT and landed on the previous last block
            // (e.g. "dude") instead of the newly appended empty bullet.
            scrolledForAnchor = "";
          }
        })
        .catch((e) => console.error("Failed to ensure dailies:", e));
    });
  });

  /**
   * Append a `- ` empty bullet block at end of today's body if the body
   * doesn't already end with one. Returns true when disk was modified so
   * the caller can invalidate the notes query.
   *
   * "Trailing empty bullet" detection: the LAST non-blank line of the body
   * is `-` or `- ` (whitespace only after the dash). This keeps us from
   * pinging disk every page load when the user already left a trailing
   * empty block from a previous session.
   */
  async function ensureTrailingEmpty(noteId: string): Promise<boolean> {
    const note = await api.getNote(noteId);
    const body = (note.body ?? "").replace(/\n+$/, "");
    const lastLine = body.split("\n").pop() ?? "";
    if (/^\s*-\s*$/.test(lastLine)) return false;
    const newBody = (body.length > 0 ? body + "\n" : "") + "- \n";
    const fmEnd = note.content.startsWith("---") ? note.content.indexOf("---", 3) : -1;
    const splitAt = fmEnd >= 0 ? fmEnd + 3 + (note.content[fmEnd + 3] === "\n" ? 1 : 0) : 0;
    const newContent = note.content.slice(0, splitAt) + newBody;
    await api.updateNote(noteId, newContent);
    return true;
  }

  // Visible window — start with 30 most recent, expand on scroll.
  const PAGE = 30;
  let visibleCount = $state(PAGE);
  // Always include the anchor in the visible window even if it's past the
  // current paging horizon.
  const visibleDailies = $derived.by((): Note[] => {
    const pool = dailies.slice(0, visibleCount);
    if (pool.some((n) => n.title === anchorDate)) return pool;
    const idx = dailies.findIndex((n) => n.title === anchorDate);
    if (idx < 0) return pool;
    // Extend visibleCount so the anchor is on screen.
    return dailies.slice(0, Math.max(visibleCount, idx + 1));
  });
  const hasMore = $derived(visibleDailies.length < dailies.length);

  function loadMore() {
    if (!hasMore) {
      // Already showing everything we've fetched — try fetching more
      // from the server. The queryKey change triggers a refetch via
      // TanStack.
      fetchLimit = fetchLimit + PAGE * 2;
      return;
    }
    visibleCount = Math.min(dailies.length, visibleCount + PAGE);
    // Pre-fetch the next batch when we're approaching the end of what
    // we have locally, so the next loadMore is instant.
    if (dailies.length - visibleCount < PAGE) {
      fetchLimit = fetchLimit + PAGE * 2;
    }
  }

  // ----- Per-note debounced save handlers -----

  type SaveState = {
    timer: ReturnType<typeof setTimeout> | null;
    pending: string | null;
    inFlight: AbortController | null;
  };
  const saveStates = new Map<string, SaveState>();

  function getState(noteId: string): SaveState {
    let s = saveStates.get(noteId);
    if (!s) {
      s = { timer: null, pending: null, inFlight: null };
      saveStates.set(noteId, s);
    }
    return s;
  }

  function handleContentChange(noteId: string, fullContent: string) {
    const s = getState(noteId);
    s.pending = fullContent;
    if (s.timer) clearTimeout(s.timer);
    setSaving();
    s.timer = setTimeout(() => { void flushSave(noteId); }, 500);
  }

  async function flushSave(noteId: string) {
    const s = getState(noteId);
    if (s.timer) { clearTimeout(s.timer); s.timer = null; }
    if (s.pending === null) return;
    const content = s.pending;
    s.pending = null;
    if (s.inFlight) s.inFlight.abort();
    const controller = new AbortController();
    s.inFlight = controller;
    // Phase 9.7 — optimistic pre-set so undo/cancelAndFlush wins WS-echo races.
    const cached = queryClient.getQueryData<Note>(["note", noteId]);
    if (cached) queryClient.setQueryData(["note", noteId], { ...cached, content });
    try {
      const updated = await api.updateNote(noteId, content, controller.signal);
      if (controller.signal.aborted) return;
      queryClient.setQueryData(["note", noteId], updated);
      setSaved();
    } catch (e) {
      if ((e as { name?: string })?.name === "AbortError") return;
      const msg = e instanceof Error ? e.message : "Unknown error";
      setSaveError(msg);
      console.error(`Daily save failed for ${noteId}:`, e);
    } finally {
      if (s.inFlight === controller) s.inFlight = null;
    }
  }

  function cancelAndFlush(noteId: string, fullContent: string) {
    const s = getState(noteId);
    s.pending = fullContent;
    if (s.timer) { clearTimeout(s.timer); s.timer = null; }
    if (s.inFlight) { s.inFlight.abort(); s.inFlight = null; }
    void flushSave(noteId);
  }

  // ----- Anchor scroll -----

  let scrollContainer = $state<HTMLElement | undefined>();
  let scrolledForAnchor = $state<string>("");

  $effect(() => {
    const a = anchorDate;
    if (!a) return;
    if (scrolledForAnchor === a) return;
    if (visibleDailies.length === 0) return;
    if (!visibleDailies.some((n) => n.title === a)) return;
    untrack(() => {
      scrolledForAnchor = a;
      // Defer to next paint so the section nodes exist.
      requestAnimationFrame(() => {
        const el = scrollContainer?.querySelector(`[data-daily="${a}"]`) as HTMLElement | null;
        el?.scrollIntoView({ block: "start", behavior: "auto" });
        // Phase 9.9 — also focus the section's cm-editor so the user can
        // start typing immediately without a mouse click. Two RAFs because
        // the cm6 editor mounts on the next tick after the section appears.
        requestAnimationFrame(() => {
          // Phase 10.1 follow-up — focus the LAST cm-editor in today's
          // section (the trailing empty bullet that `ensureTrailingEmpty`
          // guarantees). Landing at the end means the user can type new
          // entries without overwriting the start of an existing block.
          const cms = el?.querySelectorAll<HTMLElement>(".cm-editor .cm-content");
          const cm = cms?.[cms.length - 1];
          if (!cm) return;
          cm.focus();
          // Phase 9.9 follow-up #2 — DOM .focus() alone leaves cm-vim in
          // NORMAL with cm-editor's `.cm-focused` class lagging until the
          // next real keystroke. Dispatch a synthetic `i` keydown so vim
          // enters INSERT and the user can type immediately. We dispatch
          // through cm-content (the contenteditable) so cm-vim's keydown
          // handler — registered via domEventHandlers — sees it.
          requestAnimationFrame(() => {
            cm.dispatchEvent(new KeyboardEvent("keydown", {
              key: "i", code: "KeyI",
              bubbles: true, cancelable: true,
            }));
          });
        });
      });
    });
  });

  // ----- Cross-day j/k -----
  // Phase 12.X — when a day's BlockOutliner runs out of room (j on its
  // last block, k on its first), it dispatches `tesela:cross-outliner-nav`.
  // Hop to the sibling day and focus its first/last cm-content. We focus
  // by dispatching `focus()` on the cm-content; the editor's onFocus
  // handler wires vimCtx so the next j/k targets the new outliner.
  //
  // Empty stub days (`ensureTrailingEmpty` only ran for today) have zero
  // cm-content rendered. We don't want to silently skip those — the user
  // might be trying to land on an empty day to start writing. So when a
  // sibling has no cm-content, we fire ensureTrailingEmpty for that day's
  // note id and retry on the next animation frame once the WS update has
  // re-rendered it.
  function focusCmInDay(day: HTMLElement, direction: "up" | "down"): boolean {
    const cms = day.querySelectorAll<HTMLElement>(".cm-editor .cm-content");
    if (cms.length === 0) return false;
    const cm = direction === "down" ? cms[0] : cms[cms.length - 1];
    // Going up, the target is the day above. Scroll its header into view
    // first so the date label stays visible — otherwise the cursor lands
    // flush against the viewport top and the day-head is cut off above.
    // Going down is fine with `nearest` because the cm-content sits below
    // its day-head, so the header is already in the scrolled-past region.
    if (direction === "up") {
      const head = day.querySelector<HTMLElement>(".day-head");
      (head ?? day).scrollIntoView({ block: "start", behavior: "auto" });
    } else {
      cm.scrollIntoView({ block: "nearest", behavior: "auto" });
    }
    // Arm the cross-nav flag so the target outliner lands in NORMAL even
    // on empty blocks (otherwise the auto-INSERT-on-empty heuristic
    // dumps the user into INSERT after every hop).
    markNextFocusAsCrossNav();
    cm.focus();
    return true;
  }
  $effect(() => {
    const root = scrollContainer;
    if (!root) return;
    const handler = (ev: Event) => {
      const direction = (ev as CustomEvent).detail?.direction as "up" | "down" | undefined;
      if (direction !== "up" && direction !== "down") return;
      const sourceDay = (ev.target as HTMLElement | null)?.closest?.(".day");
      if (!sourceDay) return;
      const days = [...root.querySelectorAll(".day")];
      const idx = days.indexOf(sourceDay as HTMLElement);
      if (idx < 0) return;
      const step = direction === "down" ? 1 : -1;
      const nextIdx = idx + step;
      if (nextIdx < 0) return; // ran off the top
      if (nextIdx >= days.length) {
        if (direction === "down" && hasMore) loadMore();
        return;
      }
      const target = days[nextIdx] as HTMLElement;
      if (focusCmInDay(target, direction)) return;
      // Empty target — backfill a trailing empty bullet so it has a
      // landing spot, then focus once the re-render mounts the cm-editor.
      const dailyId = target.getAttribute("data-daily");
      if (!dailyId) return;
      void ensureTrailingEmpty(dailyId).then(() => {
        const tryFocus = (attempts: number) => {
          if (focusCmInDay(target, direction)) return;
          if (attempts <= 0) return;
          requestAnimationFrame(() => tryFocus(attempts - 1));
        };
        // Up to ~30 frames (~500ms) for the WS round-trip to refresh the
        // section's BlockOutliner with the newly-added empty bullet.
        requestAnimationFrame(() => tryFocus(30));
      });
    };
    root.addEventListener("tesela:cross-outliner-nav", handler);
    return () => root.removeEventListener("tesela:cross-outliner-nav", handler);
  });

  // ----- Infinite scroll: sentinel intersection -----

  let sentinel = $state<HTMLElement | undefined>();
  $effect(() => {
    const node = sentinel;
    if (!node) return;
    const obs = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (e.isIntersecting) loadMore();
      }
    }, { rootMargin: "200px" });
    obs.observe(node);
    return () => obs.disconnect();
  });

  // ----- Helpers -----

  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return { frontmatter: content.slice(0, fmEnd) + "\n", body: afterFm.slice(bodyStart) };
  }

  function formatDate(dateStr: string): string {
    try {
      const d = new Date(dateStr + "T00:00:00");
      return d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
    } catch {
      return dateStr;
    }
  }

  function formatYear(dateStr: string): string {
    return dateStr.slice(0, 4);
  }
</script>

<div bind:this={scrollContainer} class="journal">
  {#if notesQuery.isLoading}
    <div class="journal-meta">Loading journal…</div>
  {:else}
    {#each visibleDailies as note (note.id)}
      {@const split = splitContent(note.content)}
      {@const isToday = note.title === todayStr}
      {@const isAnchor = note.title === anchorDate}
      <section class="day" data-daily={note.title} class:is-today={isToday} class:is-anchor={isAnchor}>
        <header class="day-head">
          <h2 class="day-title">{formatDate(note.title)}</h2>
          {#if isToday}
            <span class="day-pill">Today</span>
          {/if}
          <span class="day-year">{formatYear(note.title)}</span>
        </header>
        <BlockOutliner
          noteId={note.id}
          body={split.body}
          frontmatter={split.frontmatter}
          onContentChange={(content) => handleContentChange(note.id, content)}
          onCancelAndFlush={(content) => cancelAndFlush(note.id, content)}
          onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
        />
      </section>
    {/each}
    {#if hasMore}
      <div bind:this={sentinel} class="journal-sentinel">
        <button class="journal-load-more" type="button" onclick={loadMore}>Load older entries</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .journal { display: flex; flex-direction: column; gap: 28px; padding-block: 16px; }
  .journal-meta { font-family: var(--v9-mono); font-size: 11px; color: var(--v9-ink-faint); padding: 12px 0; }
  /* Line above the date, à la Logseq's daily journal — divider sits in
     the gap, date title immediately follows. First section has no rule
     so the page doesn't open with a stray top border.
     Padding-bottom gives ~3 blocks of breathing room after the last
     entry; users who want to focus on a single day can drill into it. */
  .day { padding-top: 14px; padding-bottom: 80px; border-top: 1px solid var(--v9-line); }
  .day:first-child { border-top: 0; padding-top: 0; }
  .day:last-child { padding-bottom: 0; }
  .day-head { display: flex; align-items: baseline; gap: 12px; margin-bottom: 12px; }
  .day-title { font-family: var(--v9-display, var(--v9-sans)); font-size: 18px; font-weight: 600; letter-spacing: -0.01em; color: var(--foreground); }
  .day.is-today .day-title { color: var(--primary); }
  .day-pill { font-size: 10px; padding: 2px 8px; border-radius: 9999px; background: var(--primary); color: var(--primary-foreground); font-weight: 500; }
  .day-year { font-family: var(--v9-mono); font-size: 11px; color: var(--v9-ink-faint); margin-left: auto; }
  .day.is-anchor::before { content: ""; display: block; height: 0; }
  .journal-sentinel { display: flex; justify-content: center; padding: 20px 0 60px; }
  .journal-load-more {
    background: transparent;
    border: 1px solid var(--v9-line);
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 11px;
    padding: 6px 12px;
    border-radius: 9999px;
    cursor: pointer;
  }
  .journal-load-more:hover { border-color: var(--primary); color: var(--primary); }
</style>

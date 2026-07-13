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
  import { onMount, tick, untrack } from "svelte";
  import { api, ApiError } from "$lib/api-client";
  import BlockOutliner, {
    BLOCK_MOVE_PREPARE_EVENT,
    lastActiveOutlinerIsWithin,
    markNextFocusAsCrossNav,
    type RelocationBindings,
  } from "$lib/components/BlockOutliner.svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";
  import { getFocusedBlock, setFocusedBlock } from "$lib/stores/current-block.svelte";
  import { clearToast, getToast, toast } from "$lib/stores/toast.svelte";
  import { bodyHasTrailingEmpty, appendTrailingEmpty } from "$lib/ensure-trailing-empty";
  import { prevDate, dailyWalkDates, filterDisplayableDailies } from "$lib/journal-dates";
  import { previewLines } from "$lib/journal-preview";
  import { isClientMintedId } from "$lib/block-ops";
  import {
    BLOCK_MOVE_MIME,
    IDLE_BLOCK_MOVE_SESSION,
    createFocusRestorationController,
    decodeBlockMoveDragPayload,
    encodeBlockMoveDragPayload,
    reduceBlockMoveSession,
    type BlockMoveRequest,
    type BlockMoveSession,
    type BlockMoveSessionAction,
    type MovePlacement,
  } from "$lib/block-tree-move";
  import { settleNoteDocsAtServer } from "$lib/loro/note-doc-registry.svelte";
  import type { Note } from "$lib/types/Note";

  let { anchorDate }: { anchorDate: string } = $props();

  const queryClient = useQueryClient();

  let moveSession = $state<BlockMoveSession>({ ...IDLE_BLOCK_MOVE_SESSION });
  const moveActive = $derived(moveSession.phase !== "idle");
  const moveFrozen = $derived(moveSession.phase === "pending" || moveSession.phase === "retryable");
  const touchedSyntheticNotes = new Set<string>();
  let moveToastId: number | null = null;
  let moveUiDisposed = false;
  const focusRestoration = createFocusRestorationController();

  function showMoveToast(message: string, tone: "info" | "warn", durationMs: number) {
    if (moveUiDisposed) return;
    toast(message, tone, durationMs);
    moveToastId = getToast()?.id ?? null;
  }

  function clearMoveToast() {
    const current = getToast();
    if (moveToastId !== null && current?.id === moveToastId) clearToast();
    moveToastId = null;
  }

  function dispatchMove(action: BlockMoveSessionAction): BlockMoveSession {
    moveSession = reduceBlockMoveSession(moveSession, action);
    return moveSession;
  }

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
  const displayableDailies: Note[] = $derived(
    filterDisplayableDailies(todayStr, dailies, anchorDate),
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
   * Append a bid-stamped empty bullet block at end of today's body unless the
   * body already contains a focusable trailing-style empty bullet. Returns
   * true when disk was modified so the caller can invalidate the notes query.
   *
   * Position-aware detection (see `bodyHasTrailingEmpty`): we scan EVERY
   * bullet line, not just the last one. The engine can append a fresh end
   * node after a previously-trailing empty, stranding that empty mid-body —
   * a last-line-only check would miss it and accrete a new empty on every
   * mount. Any existing empty bullet means the user already has a focusable
   * line, so we suppress.
   *
   * The appended empty is bid-stamped (`- <!-- bid:UUID -->`) so the server
   * doesn't mint a fresh UUID + re-append a new end node each mount; on the
   * next mount the stamped empty bid-strips back to `- ` and the scan matches.
   */
  async function ensureTrailingEmpty(noteId: string): Promise<boolean> {
    const note = await api.getNote(noteId);
    const body = (note.body ?? "").replace(/\n+$/, "");
    if (bodyHasTrailingEmpty(body)) return false;
    const newContent = appendTrailingEmpty(note.content, body);
    // Base = the freshly-fetched server body this edit started from, so the
    // server base-diffs (we only append one empty bullet) and a concurrent
    // peer edit to an existing block survives.
    await api.updateNote(noteId, newContent, note.content);
    return true;
  }

  // Visible window — start with 30 most recent, expand on scroll.
  const PAGE = 30;
  let visibleCount = $state(PAGE);
  // Days of empty placeholder past the oldest on-disk daily. The user
  // expects an "infinite" calendar going back even when those days have
  // no file on disk yet. Bumped on `loadMore()` once the on-disk list is
  // exhausted.
  let paddingDays = $state(60);

  // Virtual mounting: only sections that have entered the viewport
  // (or are explicit anchor/today) actually mount a BlockOutliner.
  // Without this, 30+ cm-editor instances stand up on every Dailies
  // load — easily 2-5s of frozen UI on large imported graphs. Once a
  // section mounts we keep it mounted; unmounting would risk losing
  // in-flight edits.
  let mountedSections = $state<Set<string>>(new Set());
  function mountAction(node: HTMLElement, noteId: string) {
    if (mountedSections.has(noteId)) return {};
    const obs = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            mountedSections.add(noteId);
            mountedSections = new Set(mountedSections);
            obs.disconnect();
            break;
          }
        }
      },
      { rootMargin: "800px" },
    );
    obs.observe(node);
    return { destroy: () => obs.disconnect() };
  }
  function shouldStartMounted(noteTitle: string): boolean {
    return noteTitle === todayStr || noteTitle === anchorDate;
  }
  /// Force-mount a day section (used by cross-day j/k navigation so
  /// the target day's BlockOutliner can be focused after a tick).
  function ensureMounted(noteId: string) {
    if (mountedSections.has(noteId)) return;
    mountedSections.add(noteId);
    mountedSections = new Set(mountedSections);
  }
  // Always include the anchor in the visible window even if it's past the
  // current paging horizon.
  const onDiskVisible = $derived.by((): Note[] => {
    const pool = displayableDailies.slice(0, visibleCount);
    if (pool.some((n) => n.title === anchorDate)) return pool;
    const idx = displayableDailies.findIndex((n) => n.title === anchorDate);
    if (idx < 0) return pool;
    // Extend visibleCount so the anchor is on screen.
    return displayableDailies.slice(0, Math.max(visibleCount, idx + 1));
  });

  /** Generate a synthetic Note placeholder for a daily that doesn't exist
   *  on disk yet. Used for the "infinite scroll back through the
   *  calendar" UX — even days with no file get a header rendered so the
   *  user can keep scrolling. Clicking into one will create the file via
   *  the existing ensureTrailingEmpty path. */
  function syntheticDaily(dateStr: string): Note {
    return {
      id: dateStr,
      title: dateStr,
      // Carry the daily frontmatter so the BlockOutliner's saved content
      // is a proper daily (tags: [daily]) when the user types — the file
      // is created lazily on first edit (see flushSave). Body stays empty;
      // the outliner seeds one editable blank block.
      content: `---\ntitle: ${dateStr}\ntags: [daily]\ncreated: ${dateStr}T00:00:00Z\n---\n\n`,
      body: "",
      metadata: {
        title: dateStr,
        tags: ["daily"],
        aliases: [],
        note_type: null,
        custom: {},
        created: null,
        modified: null,
      },
      path: `notes/${dateStr}.md`,
      checksum: "",
      created_at: "",
      modified_at: "",
      attachments: [],
    } as unknown as Note;
  }

  /** Visible list = a gap-free descending calendar from today (or the
   *  newest on-disk daily, if a peer created a future-dated one) back to
   *  the oldest on-disk daily (or today), then `paddingDays` of additional
   *  empty days for "scroll into the past" UX. Days that have a real file
   *  get the real note; days without get a synthetic empty placeholder so
   *  the feed renders without visual gaps between non-adjacent entries.
   *
   *  The date walk lives in `dailyWalkDates` (journal-dates.ts) — bounded by
   *  construction, so a future-dated `oldest` (every on-disk daily ahead of
   *  the local clock, e.g. a fresh mosaic whose only daily synced from a
   *  TZ-ahead peer) renders instead of hard-hanging the render in an
   *  unbounded loop, and a lone future "tomorrow" renders instead of being
   *  silently dropped.
   */
  const visibleDailies = $derived.by((): Note[] => {
    const real = onDiskVisible;
    if (real.length === 0) {
      // Nothing on disk in the visible window — fall back to a
      // paddingDays-deep synthetic tail starting at today.
      const synth: Note[] = [];
      let cursor = todayStr;
      for (let i = 0; i < paddingDays; i++) {
        synth.push(syntheticDaily(cursor));
        cursor = prevDate(cursor);
      }
      return synth;
    }

    const byDate = new Map(real.map((n) => [n.title, n]));
    const onDiskDates = new Set(displayableDailies.map((n) => n.title));
    const newest = real[0].title;
    const oldest = real[real.length - 1].title;

    // Step 1: max(newest, today) → min(oldest, today), gap-free. Fill the
    // in-betweens with synthetic empties so a write on a "missed" day still
    // has a place to land. (Earlier behaviour skipped missed days entirely,
    // which left a confusing visual jump from "Today" to whenever the user
    // last wrote.) Every on-disk daily in the window falls inside the walk,
    // so no post-loop append guard is needed.
    const walked = dailyWalkDates(todayStr, newest, oldest);
    const out: Note[] = walked.map((d) => byDate.get(d) ?? syntheticDaily(d));

    // Step 2: pad below the walk's last (oldest) day with `paddingDays` of
    // synthetic empties so infinite scroll keeps revealing more days.
    let tail = prevDate(walked[walked.length - 1]);
    for (let i = 0; i < paddingDays; i++) {
      if (!onDiskDates.has(tail)) {
        out.push(syntheticDaily(tail));
      }
      tail = prevDate(tail);
    }

    return out;
  });
  const hasMore = $derived(onDiskVisible.length < displayableDailies.length);

  function loadMore() {
    if (!hasMore) {
      // No more on-disk dailies to reveal — extend the synthetic tail so
      // the user can keep scrolling back through the calendar.
      paddingDays = paddingDays + 60;
      // Also nudge the server in case it's holding more behind the
      // current fetch limit (rare; mostly defensive).
      fetchLimit = fetchLimit + PAGE * 2;
      return;
    }
    visibleCount = Math.min(displayableDailies.length, visibleCount + PAGE);
    if (displayableDailies.length - visibleCount < PAGE) {
      fetchLimit = fetchLimit + PAGE * 2;
    }
  }

  // ----- Per-note debounced save handlers -----

  type SaveState = {
    timer: ReturnType<typeof setTimeout> | null;
    pending: string | null;
    // The edit BASE for the pending save — the body the outliner last reseeded
    // from. Sent as `base_content` so the server diffs the author's real
    // changes (base→new) and an untouched block is never re-asserted over a
    // concurrent peer edit. Captured from the FIRST change in the debounce
    // window (base doesn't shift mid-burst — the outliner defers external
    // reseeds while typing); cleared on flush.
    base: string | undefined;
    inFlight: AbortController | null;
    inFlightPromise: Promise<void> | null;
    // True when the note has no file on disk yet (a synthetic day the user
    // just typed into) and must be CREATED before/instead of updated. PUT
    // 404s on a missing note, so the first save POSTs the full content.
    needsCreate: boolean;
  };
  const saveStates = new Map<string, SaveState>();

  function getState(noteId: string): SaveState {
    let s = saveStates.get(noteId);
    if (!s) {
      s = {
        timer: null,
        pending: null,
        base: undefined,
        inFlight: null,
        inFlightPromise: null,
        needsCreate: false,
      };
      saveStates.set(noteId, s);
    }
    return s;
  }

  function handleContentChange(
    noteId: string,
    fullContent: string,
    isSynthetic = false,
    baseContent?: string,
  ) {
    const s = getState(noteId);
    s.pending = fullContent;
    // Keep the FIRST base of the window (don't overwrite with a later change's
    // base — they're the same during a typing burst, but first-wins is the
    // safe choice if an external reseed ever lands mid-window).
    if (s.base === undefined) s.base = baseContent;
    if (isSynthetic) {
      s.needsCreate = true;
      touchedSyntheticNotes.add(noteId);
    }
    if (s.timer) clearTimeout(s.timer);
    setSaving();
    s.timer = setTimeout(() => { void flushSave(noteId).catch(() => {}); }, 500);
  }

  function flushSave(noteId: string): Promise<void> {
    const s = getState(noteId);
    if (s.timer) { clearTimeout(s.timer); s.timer = null; }
    if (s.pending === null) return s.inFlightPromise ?? Promise.resolve();
    if (s.inFlightPromise) {
      const predecessor = s.inFlightPromise;
      return predecessor.then(
        () => flushSave(noteId),
        async (error) => {
          // A failed predecessor must not strand content typed behind it.
          // Drain that successor first, then preserve a failed completion so
          // relocation preflight still fails closed.
          await flushSave(noteId);
          throw error;
        },
      );
    }
    const content = s.pending;
    s.pending = null;
    const base = s.base;
    s.base = undefined;
    const controller = new AbortController();
    s.inFlight = controller;
    // Phase 9.7 — optimistic pre-set so undo/cancelAndFlush wins WS-echo races.
    const cached = queryClient.getQueryData<Note>(["note", noteId]);
    if (cached) queryClient.setQueryData(["note", noteId], { ...cached, content });
    const completion = (async () => {
      try {
        // Lazy-create: a synthetic day's first edit POSTs the full content
        // (which already carries the daily frontmatter), then the journal
        // refetch re-renders it as a real day. Claim needsCreate up front so
        // a coalesced double-flush doesn't double-create.
        if (s.needsCreate) {
          s.needsCreate = false;
          try {
            const created = await api.createNote(noteId, content);
            queryClient.setQueryData(["note", noteId], created);
            await queryClient.invalidateQueries({ queryKey: ["notes"] });
            setSaved();
            return;
          } catch (createErr) {
            // Already exists (race) or create failed — fall through to PUT.
            console.warn(`Daily lazy-create fell back to update for ${noteId}:`, createErr);
          }
        }
        const updated = await api.updateNote(noteId, content, base, controller.signal);
        if (controller.signal.aborted) return;
        queryClient.setQueryData(["note", noteId], updated);
        setSaved();
      } catch (e) {
        if ((e as { name?: string })?.name === "AbortError") return;
        const msg = e instanceof Error ? e.message : "Unknown error";
        setSaveError(msg);
        console.error(`Daily save failed for ${noteId}:`, e);
        throw e;
      } finally {
        if (s.inFlight === controller) {
          s.inFlight = null;
          s.inFlightPromise = null;
        }
      }
    })();
    s.inFlightPromise = completion;
    return completion;
  }

  async function settleJournalSave(noteId: string): Promise<void> {
    const s = getState(noteId);
    let failed = false;
    let firstFailure: unknown;
    while (true) {
      if (s.inFlightPromise) {
        try {
          await s.inFlightPromise;
        } catch (error) {
          if (!failed) firstFailure = error;
          failed = true;
        }
        continue;
      }
      if (s.pending === null) {
        if (failed) throw firstFailure;
        return;
      }
      try {
        await flushSave(noteId);
      } catch (error) {
        if (!failed) firstFailure = error;
        failed = true;
      }
    }
  }

  function cancelAndFlush(noteId: string, fullContent: string, baseContent?: string): Promise<void> {
    const s = getState(noteId);
    s.pending = fullContent;
    if (baseContent !== undefined) s.base = baseContent;
    if (s.timer) { clearTimeout(s.timer); s.timer = null; }
    // Preserve any live request as an ordering predecessor. `settleJournalSave`
    // awaits it, then flushes this newer body and loops until the queue is quiet.
    const completion = settleJournalSave(noteId);
    void completion.catch(() => {});
    return completion;
  }

  // ----- Anchor scroll -----

  let scrollContainer = $state<HTMLElement | undefined>();
  let scrolledForAnchor = $state<string>("");

  function selectorValue(value: string): string {
    return CSS.escape(value);
  }

  function daySection(noteId: string): HTMLElement | null {
    return scrollContainer?.querySelector<HTMLElement>(
      `.day[data-note-id="${selectorValue(noteId)}"]`,
    ) ?? null;
  }

  function autoScrollMove(event: DragEvent) {
    const outline = scrollContainer?.closest<HTMLElement>(".gr-outline");
    if (!outline) return;
    const rect = outline.getBoundingClientRect();
    const edge = Math.min(72, rect.height / 4);
    if (event.clientY < rect.top + edge) outline.scrollBy({ top: -24, behavior: "auto" });
    else if (event.clientY > rect.bottom - edge) outline.scrollBy({ top: 24, behavior: "auto" });
  }

  function carriesInternalMove(event: DragEvent): boolean {
    return Array.from(event.dataTransfer?.types ?? []).includes(BLOCK_MOVE_MIME);
  }

  function payloadMatchesSession(event: DragEvent): boolean {
    const transfer = event.dataTransfer;
    const request = moveSession.request;
    if (!transfer || !request) return false;
    const payload = decodeBlockMoveDragPayload(
      Array.from(transfer.types),
      transfer.getData(BLOCK_MOVE_MIME),
    );
    return !!payload
      && payload.move_id === request.move_id
      && payload.source_note_id === request.source_note_id
      && payload.root_bid === request.root_bid;
  }

  function beginPointerMove(event: DragEvent, noteId: string, sourceBid: string) {
    const transfer = event.dataTransfer;
    if (!transfer || moveSession.phase !== "idle") {
      event.preventDefault();
      return;
    }
    const moveId = crypto.randomUUID();
    transfer.clearData();
    transfer.setData(BLOCK_MOVE_MIME, encodeBlockMoveDragPayload({
      move_id: moveId,
      source_note_id: noteId,
      root_bid: sourceBid,
    }));
    transfer.effectAllowed = "move";
    dispatchMove({
      type: "start",
      request: {
        move_id: moveId,
        source_note_id: noteId,
        root_bid: sourceBid,
        destination_note_id: noteId,
        target_bid: null,
        placement: "append",
      },
    });
  }

  function targetMove(noteId: string, bid: string | null, placement: MovePlacement): BlockMoveSession {
    const next = reduceBlockMoveSession(moveSession, {
      type: "target",
      noteId,
      bid,
      placement,
    });
    moveSession = next;
    ensureMounted(noteId);
    return next;
  }

  function hoverBlockTarget(event: DragEvent, noteId: string, bid: string, placement: Exclude<MovePlacement, "append">) {
    if (moveSession.phase !== "selecting") return;
    autoScrollMove(event);
    targetMove(noteId, bid, placement);
  }

  function dropOnBlock(event: DragEvent, noteId: string, bid: string, placement: Exclude<MovePlacement, "append">) {
    if (moveSession.phase !== "selecting" || !payloadMatchesSession(event)) return;
    const targeted = targetMove(noteId, bid, placement);
    void submitSelectedMove(targeted);
  }

  function handleDayDragOver(event: DragEvent, noteId: string) {
    if (moveSession.phase !== "selecting" || !carriesInternalMove(event)) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    ensureMounted(noteId);
    autoScrollMove(event);
    targetMove(noteId, null, "append");
  }

  function handleDayDrop(event: DragEvent, noteId: string) {
    if (!carriesInternalMove(event)) return;
    if (moveSession.phase !== "selecting" || !payloadMatchesSession(event)) {
      event.stopPropagation();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const targeted = targetMove(noteId, null, "append");
    void submitSelectedMove(targeted);
  }

  function cancelSelectingMove() {
    if (moveSession.phase !== "selecting") return;
    const request = moveSession.request;
    dispatchMove({ type: "cancel" });
    clearMoveToast();
    if (request) void focusBlockBid(request.source_note_id, request.root_bid);
  }

  async function prepareOutliner(
    noteId: string,
    addressedBids: string[],
    required: boolean,
    expandInsideBid: string | null,
  ): Promise<void> {
    await tick();
    const root = daySection(noteId)?.querySelector<HTMLElement>(
      `[data-block-outliner][data-note-id="${selectorValue(noteId)}"]`,
    ) ?? null;
    if (!root) {
      if (required) throw new Error(`Move source ${noteId} is not mounted`);
      return;
    }
    let response: Promise<boolean> | null = null;
    root.dispatchEvent(new CustomEvent(BLOCK_MOVE_PREPARE_EVENT, {
      detail: {
        noteId,
        addressedBids,
        expandInsideBid,
        respond: (promise: Promise<boolean>) => { response = promise; },
      },
    }));
    if (!response || !(await response)) {
      throw new Error(`Wait for ${noteId} to finish saving, then retry the move`);
    }
  }

  async function prepareMove(request: BlockMoveRequest): Promise<void> {
    ensureMounted(request.destination_note_id);
    await tick();

    const destinationDay = daySection(request.destination_note_id);
    const untouchedSyntheticDestination =
      request.destination_note_id !== request.source_note_id
      && request.placement === "append"
      && destinationDay?.classList.contains("synthetic") === true
      && !touchedSyntheticNotes.has(request.destination_note_id);

    const addressed = new Map<string, {
      bids: Set<string>;
      required: boolean;
      skipOutliner: boolean;
      expandInsideBid: string | null;
    }>();
    const source = {
      bids: new Set([request.root_bid]),
      required: true,
      skipOutliner: false,
      expandInsideBid: null,
    };
    addressed.set(request.source_note_id, source);
    const destination = addressed.get(request.destination_note_id) ?? {
      bids: new Set<string>(),
      required: false,
      skipOutliner: untouchedSyntheticDestination,
      expandInsideBid: null,
    };
    if (request.target_bid) destination.bids.add(request.target_bid);
    destination.skipOutliner = destination.skipOutliner || untouchedSyntheticDestination;
    destination.expandInsideBid = request.placement === "inside"
      ? request.target_bid
      : null;
    addressed.set(request.destination_note_id, destination);

    for (const [noteId, entry] of addressed) {
      await settleJournalSave(noteId);
      if (!entry.skipOutliner) {
        await prepareOutliner(
          noteId,
          [...entry.bids],
          entry.required,
          entry.expandInsideBid,
        );
      }
      // A failed block-op settle may schedule its whole-body fallback through
      // this Journal. Drain that queue before advancing to the Loro barrier.
      await settleJournalSave(noteId);
    }

    const barrierNotes = [...addressed.keys()].filter(
      (noteId) => !(untouchedSyntheticDestination && noteId === request.destination_note_id),
    );
    await settleNoteDocsAtServer(barrierNotes);
  }

  function apiErrorDetail(error: unknown): { message: string; retrySafe: boolean; moveId: string | null } {
    if (!(error instanceof ApiError)) {
      return {
        message: error instanceof Error ? error.message : "Block move failed",
        retrySafe: false,
        moveId: null,
      };
    }
    try {
      const parsed = JSON.parse(error.body) as {
        error?: unknown;
        retry_safe?: unknown;
        move_id?: unknown;
      };
      return {
        message: typeof parsed.error === "string" ? parsed.error : error.message,
        retrySafe: error.status === 503 && parsed.retry_safe === true,
        moveId: typeof parsed.move_id === "string" ? parsed.move_id : null,
      };
    } catch {
      return { message: error.message, retrySafe: false, moveId: null };
    }
  }

  async function settleMoveResponse(request: BlockMoveRequest) {
    const response = await api.relocateBlockSubtree(request);
    if (response.move_id !== request.move_id) {
      throw new Error("Move response id did not match the submitted request");
    }
    for (const note of response.notes) {
      queryClient.setQueryData(["note", note.id], note);
    }
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["notes"] }),
      ...response.notes.map((note) =>
        queryClient.invalidateQueries({ queryKey: ["note", note.id], exact: true })
      ),
    ]);
  }

  async function executeMove(request: BlockMoveRequest, withPreflight: boolean) {
    try {
      if (withPreflight) await prepareMove(request);
      await settleMoveResponse(request);
      if (moveUiDisposed) return;
      dispatchMove({ type: "success" });
      clearMoveToast();
      await focusBlockBid(request.destination_note_id, request.root_bid);
    } catch (error) {
      if (moveUiDisposed) return;
      const detail = apiErrorDetail(error);
      if (
        error instanceof ApiError
        && detail.retrySafe
        && detail.moveId === request.move_id
      ) {
        dispatchMove({ type: "recoverable-error" });
        showMoveToast(`${detail.message} · Press R or Enter to retry safely`, "warn", 0);
        return;
      }
      clearMoveToast();
      dispatchMove({ type: "ordinary-error" });
      toast(detail.message, "error", 6000);
      await focusBlockBid(request.source_note_id, request.root_bid);
    }
  }

  async function submitSelectedMove(selected: BlockMoveSession) {
    if (
      selected.phase !== "selecting"
      || !selected.request
      || !selected.targetNoteId
      || !selected.placement
    ) return;
    clearMoveToast();
    moveSession = reduceBlockMoveSession(selected, { type: "submit" });
    await executeMove(selected.request, true);
  }

  function submitSameNoteMove(request: BlockMoveRequest) {
    if (moveSession.phase !== "idle") return;
    let selected = reduceBlockMoveSession(IDLE_BLOCK_MOVE_SESSION, { type: "start", request });
    selected = reduceBlockMoveSession(selected, {
      type: "target",
      noteId: request.destination_note_id,
      bid: request.target_bid,
      placement: request.placement,
    });
    moveSession = selected;
    void submitSelectedMove(selected);
  }

  function retryMove() {
    if (moveSession.phase !== "retryable" || !moveSession.request) return;
    const request = moveSession.request;
    clearMoveToast();
    dispatchMove({ type: "submit" });
    void executeMove(request, false);
  }

  async function focusBlockBid(noteId: string, bid: string): Promise<void> {
    if (moveUiDisposed) return;
    ensureMounted(noteId);
    let firstLookup = true;
    await focusRestoration.restore({
      maxAttempts: 60,
      findTarget: async () => {
        if (firstLookup) {
          firstLookup = false;
          await tick();
        }
        if (moveUiDisposed) return null;
        const row = daySection(noteId)?.querySelector<HTMLElement>(
          `[data-block-bid="${selectorValue(bid)}"]`,
        ) ?? null;
        const editor = row?.querySelector<HTMLElement>(".cm-editor .cm-content") ?? null;
        return row && editor ? { row, editor } : null;
      },
      waitForRetry: () => new Promise<void>(
        (resolve) => requestAnimationFrame(() => resolve()),
      ),
      focusTarget: ({ row, editor }) => {
        if (moveUiDisposed) return;
        row.scrollIntoView({ block: "nearest", behavior: "auto" });
        editor.focus();
      },
    });
  }

  function relocationBindings(noteId: string): RelocationBindings {
    const request = moveSession.request;
    const isSource = request?.source_note_id === noteId;
    const isTarget = moveSession.targetNoteId === noteId;
    const affected = isSource || request?.destination_note_id === noteId;
    return {
      sourceBid: isSource ? request?.root_bid ?? null : null,
      targetBid: isTarget ? moveSession.targetBid : null,
      placement: isTarget ? moveSession.placement : null,
      pending: moveFrozen && affected,
      onDragStart: (event, sourceBid) => beginPointerMove(event, noteId, sourceBid),
      onDragOver: (event, targetBid, placement) => hoverBlockTarget(event, noteId, targetBid, placement),
      onDrop: (event, targetBid, placement) => dropOnBlock(event, noteId, targetBid, placement),
      onCancel: cancelSelectingMove,
    };
  }

  function moveTargetElements(): HTMLElement[] {
    return [...(scrollContainer?.querySelectorAll<HTMLElement>("[data-move-key-target]") ?? [])];
  }

  function currentMoveTargetIndex(elements: HTMLElement[]): number {
    if (moveSession.targetNoteId) {
      return elements.findIndex((element) => {
        if (element.dataset.noteId !== moveSession.targetNoteId) return false;
        return moveSession.targetBid
          ? element.dataset.blockBid === moveSession.targetBid
          : element.dataset.moveDayTarget === "true";
      });
    }
    const request = moveSession.request;
    if (!request) return -1;
    return elements.findIndex(
      (element) => element.dataset.noteId === request.source_note_id
        && element.dataset.blockBid === request.root_bid,
    );
  }

  async function navigateMoveTarget(direction: "up" | "down") {
    let elements = moveTargetElements();
    let index = currentMoveTargetIndex(elements);
    const step = direction === "down" ? 1 : -1;
    for (let cursor = index + step; cursor >= 0 && cursor < elements.length; cursor += step) {
      const element = elements[cursor];
      if (element.dataset.moveInvalid === "true") continue;
      const noteId = element.dataset.noteId;
      if (!noteId) continue;
      if (element.dataset.moveDayTarget === "true") {
        ensureMounted(noteId);
        targetMove(noteId, null, "append");
      } else {
        const bid = element.dataset.blockBid;
        if (!bid) continue;
        targetMove(noteId, bid, "after");
      }
      element.scrollIntoView({ block: "nearest", behavior: "auto" });
      return;
    }
    if (direction === "down") {
      loadMore();
      await tick();
      elements = moveTargetElements();
      index = currentMoveTargetIndex(elements);
      if (index >= 0 && index < elements.length - 1) await navigateMoveTarget(direction);
    }
  }

  function commitKeyboardMove(key: "b" | "i" | "a") {
    if (moveSession.phase !== "selecting" || !moveSession.targetNoteId) return;
    const placement: MovePlacement = moveSession.targetBid === null
      ? "append"
      : key === "b" ? "before" : key === "i" ? "inside" : "after";
    const targeted = targetMove(moveSession.targetNoteId, moveSession.targetBid, placement);
    void submitSelectedMove(targeted);
  }

  function startCommandMove() {
    if (moveSession.phase !== "idle" || !lastActiveOutlinerIsWithin(scrollContainer ?? null)) return;
    const block = getFocusedBlock();
    if (!block?.bid || !block.note_id || isClientMintedId(block.id)) {
      toast("Wait for the focused block to finish saving before moving it", "warn");
      return;
    }
    const row = daySection(block.note_id)?.querySelector(
      `[data-block-bid="${selectorValue(block.bid)}"]`,
    );
    if (!row) return;
    dispatchMove({
      type: "start",
      request: {
        move_id: crypto.randomUUID(),
        source_note_id: block.note_id,
        root_bid: block.bid,
        destination_note_id: block.note_id,
        target_bid: null,
        placement: "append",
      },
    });
    showMoveToast("Move mode · J/K target · B/I/A place · Esc cancel", "info", 5000);
  }

  onMount(() => {
    moveUiDisposed = false;
    const commandHandler = () => startCommandMove();
    const revokeFocusRestoration = () => focusRestoration.revoke();
    const keyHandler = (event: KeyboardEvent) => {
      if (!moveActive) return;
      const key = event.key.toLowerCase();
      const handled = key === "escape"
        || key === "j"
        || key === "k"
        || key === "b"
        || key === "i"
        || key === "a"
        || key === "r"
        || event.key === "Enter";
      if (!handled) return;
      event.preventDefault();
      event.stopPropagation();

      if (moveSession.phase === "pending") return;
      if (moveSession.phase === "retryable") {
        if (key === "r" || event.key === "Enter") retryMove();
        return;
      }
      if (key === "escape") {
        cancelSelectingMove();
        return;
      }
      if (key === "j" || key === "k") {
        void navigateMoveTarget(key === "j" ? "down" : "up");
        return;
      }
      if (key === "b" || key === "i" || key === "a") commitKeyboardMove(key);
    };
    window.addEventListener("tesela:start-block-move", commandHandler);
    document.addEventListener("pointerdown", revokeFocusRestoration, true);
    document.addEventListener("keydown", revokeFocusRestoration, true);
    document.addEventListener("keydown", keyHandler, true);
    return () => {
      moveUiDisposed = true;
      focusRestoration.dispose();
      window.removeEventListener("tesela:start-block-move", commandHandler);
      document.removeEventListener("pointerdown", revokeFocusRestoration, true);
      document.removeEventListener("keydown", revokeFocusRestoration, true);
      document.removeEventListener("keydown", keyHandler, true);
      clearMoveToast();
    };
  });

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
      const dailyId = target.getAttribute("data-daily");
      const isSynthetic = target.classList.contains("synthetic");

      // Synthetic target (no file on disk yet) — keyboard-first hop into a
      // calendar gap. Create the daily, invalidate the query so the
      // section re-renders as a real BlockOutliner, then focus.
      if (isSynthetic && dailyId) {
        void api.getDailyNote(dailyId).then(() => {
          queryClient.invalidateQueries({ queryKey: ["notes"] });
          const tryFocus = (attempts: number) => {
            // After invalidation, the DOM gets a fresh `.day` element at the
            // same date — re-query rather than trust the stale ref.
            const refreshed = scrollContainer?.querySelector(
              `.day[data-daily="${dailyId}"]`,
            ) as HTMLElement | null;
            if (refreshed && focusCmInDay(refreshed, direction)) return;
            if (attempts <= 0) return;
            requestAnimationFrame(() => tryFocus(attempts - 1));
          };
          requestAnimationFrame(() => tryFocus(60));
        });
        return;
      }

      // If the target day is still a virtualization placeholder (no
      // cm-editor mounted), force it to upgrade and retry focus next
      // frame once the editor exists.
      const targetNote = dailyId && dailies.find((n) => n.title === dailyId);
      if (targetNote && !mountedSections.has(targetNote.id)) {
        ensureMounted(targetNote.id);
        const tryFocus = (attempts: number) => {
          if (focusCmInDay(target, direction)) return;
          if (attempts <= 0) return;
          requestAnimationFrame(() => tryFocus(attempts - 1));
        };
        requestAnimationFrame(() => tryFocus(30));
        return;
      }

      if (focusCmInDay(target, direction)) return;
      // Empty target — backfill a trailing empty bullet so it has a
      // landing spot, then focus once the re-render mounts the cm-editor.
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

<div
  bind:this={scrollContainer}
  class="journal"
  data-move-mode={moveActive ? moveSession.phase : undefined}
>
  {#if moveActive}
    <div class="move-mode-banner" data-move-status={moveSession.phase}>
      {#if moveSession.phase === "selecting"}
        Move subtree · J/K choose target · B before · I inside · A after · Esc cancel
      {:else if moveSession.phase === "pending"}
        Moving subtree…
      {:else}
        Recovery is safe · press R or Enter to retry this exact move
      {/if}
    </div>
  {/if}
  {#if notesQuery.isLoading}
    <div class="journal-meta">Loading journal…</div>
  {:else}
    {#each visibleDailies as note (note.id)}
      {@const split = splitContent(note.content)}
      {@const isToday = note.title === todayStr}
      {@const isAnchor = note.title === anchorDate}
      {@const isSynthetic = note.checksum === ""}
      {@const isMounted = mountedSections.has(note.id) || shouldStartMounted(note.title)}
      <section
        class="day"
        class:synthetic={isSynthetic}
        data-daily={note.title}
        data-note-id={note.id}
        data-drop-placement={moveSession.targetNoteId === note.id && moveSession.targetBid === null ? "append" : undefined}
        class:is-today={isToday}
        class:is-anchor={isAnchor}
        use:mountAction={note.id}
        ondragover={(event) => handleDayDragOver(event, note.id)}
        ondrop={(event) => handleDayDrop(event, note.id)}
      >
        <header
          class="day-head"
          data-move-key-target
          data-move-day-target="true"
          data-note-id={note.id}
        >
          <h2 class="day-title">{formatDate(note.title)}</h2>
          {#if isToday}
            <span class="day-pill">Today</span>
          {/if}
          <span class="day-year">{formatYear(note.title)}</span>
        </header>
        {#if isMounted}
          <!-- Every day — even one with no file yet (synthetic) — mounts a
               BlockOutliner with a ready blank block. A synthetic day's
               file is created lazily on the first edit (handleContentChange
               passes isSynthetic → flushSave creates it), so untouched days
               stay zero-byte on disk: no "click to add" placeholder, no
               file pollution. -->
          <BlockOutliner
            noteId={note.id}
            body={split.body}
            frontmatter={split.frontmatter}
            onContentChange={(content, base) => handleContentChange(note.id, content, isSynthetic, base)}
            onCancelAndFlush={(content, base) => cancelAndFlush(note.id, content, base)}
            onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
            onfocusedblockchange={(b) => setFocusedBlock(b)}
            relocation={relocationBindings(note.id)}
            onsamenotemove={submitSameNoteMove}
          />
        {:else}
          <!-- Cheap preview until the section scrolls near the viewport.
               No cm-editor is mounted yet — keeps initial paint fast on
               large imported journals (e.g. 459 Logseq dailies). A
               synthetic day previews as a single empty bullet. -->
          {@const preview = isSynthetic ? [] : previewLines(split.body)}
          <div class="day-placeholder">
            {#if isSynthetic}
              <div class="placeholder-line">•</div>
            {:else if preview.length === 0}
              <!-- Body was only bid stamps / property continuations /
                   malformed metadata — show nothing rather than surface
                   structural noise as a "preview". -->
            {:else}
              {#each preview as line}
                <div class="placeholder-line">• {line.text}</div>
              {/each}
            {/if}
          </div>
        {/if}
      </section>
    {/each}
    <div bind:this={sentinel} class="journal-sentinel">
      <button class="journal-load-more" type="button" onclick={loadMore}
        >Load older entries</button
      >
    </div>
  {/if}
</div>

<style>
  .journal { display: flex; flex-direction: column; gap: 28px; padding-block: 16px; }
  .move-mode-banner {
    position: sticky;
    top: 0;
    z-index: 20;
    align-self: center;
    padding: 6px 12px;
    border: 1px solid color-mix(in srgb, var(--primary) 45%, transparent);
    border-radius: 9999px;
    background: color-mix(in srgb, var(--background) 92%, var(--primary) 8%);
    color: var(--foreground);
    box-shadow: 0 4px 16px color-mix(in srgb, #000 18%, transparent);
    font-family: var(--v9-mono);
    font-size: 11px;
  }
  .day[data-drop-placement="append"] {
    outline: 1px solid color-mix(in srgb, var(--primary) 65%, transparent);
    outline-offset: 5px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--primary) 7%, transparent);
  }
  .journal-meta { font-family: var(--v9-mono); font-size: 11px; color: var(--v9-ink-faint); padding: 12px 0; }
  /* Line above the date, à la Logseq's daily journal — divider sits in
     the gap, date title immediately follows. First section has no rule
     so the page doesn't open with a stray top border.
     Padding-bottom is a deliberate ~1/3-viewport gap so each day reads
     as its own space when scrolling the journal. */
  .day { padding-top: 14px; padding-bottom: 33vh; border-top: 1px solid var(--v9-line); }
  .day:first-child { border-top: 0; padding-top: 0; }
  .day:last-child { padding-bottom: 0; }
  .day.synthetic { padding-bottom: 28px; opacity: 0.62; }
  .day.synthetic .day-title { color: var(--v9-ink-faint); font-weight: 500; }
  .day-create {
    display: block;
    margin-top: 8px;
    background: transparent;
    border: 1px dashed var(--v9-line);
    border-radius: 6px;
    color: var(--v9-ink-faint);
    padding: 8px 12px;
    cursor: pointer;
    font-family: var(--v9-mono);
    font-size: 11px;
    width: 100%;
    text-align: left;
  }
  .day-create:hover {
    border-color: var(--v9-line-soft);
    color: var(--v9-ink-muted);
  }
  .day-head { display: flex; align-items: baseline; gap: 12px; margin-bottom: 12px; }
  .day-title { font-family: var(--theme-font-sans); font-size: 18px; font-weight: 600; letter-spacing: -0.01em; color: var(--fg-default); }
  /* Today still gets a marker, but it's a soft brightening, not the
     primary accent — primary is reserved for active selection now. */
  .day.is-today .day-title { color: var(--fg-default); }
  /* Today's pill uses the brand "spark" (the hotter coral) — one of the
     few places the neon accent is intentionally kept. */
  .day-pill {
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 9999px;
    background: color-mix(in srgb, var(--accent-spark) 14%, transparent);
    color: var(--accent-spark);
    border: 1px solid color-mix(in srgb, var(--accent-spark) 28%, transparent);
    font-weight: 500;
  }
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
  /* Placeholder rendered for off-screen day sections — keeps the
     date header visible (and the row clickable / scrollable-to) while
     avoiding the cost of mounting a CodeMirror editor for every
     imported day. Each line is just plain text, scaled to roughly
     match the editor's typography. */
  .day-placeholder {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 60px;
    padding-left: 20px;
    color: var(--v9-ink-2, var(--muted-foreground));
    font-size: 13px;
    line-height: 1.6;
  }
  .placeholder-line {
    white-space: pre-wrap;
    word-break: break-word;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    -webkit-box-orient: vertical;
    max-width: 100%;
  }
  .placeholder-more { color: var(--v9-ink-faint); font-size: 11px; }
</style>

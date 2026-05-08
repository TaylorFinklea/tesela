<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import {
    getActiveRegion,
    setActiveRegion,
    getBottomTab,
    setBottomTab,
    type BottomTab,
  } from "$lib/stores/pane-state.svelte";
  import { getFocusedBlock } from "$lib/stores/current-block.svelte";
  import { parseBlocks } from "$lib/block-parser";
  import { updateBlockProperty, clearBlockProperty } from "$lib/property-update";
  import {
    buildRegistry,
    buildInheritanceMap,
    resolveTagChain,
    getVisibleChoices,
    parseHiddenChoices,
    updateFrontmatterKey,
  } from "$lib/property-registry";
  import type { PropertyDefinition, PropertyRegistry, InheritanceMap } from "$lib/property-registry";
  import { assignChords } from "$lib/chord-keys";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import HistoryTab from "./HistoryTab.svelte";
  import LinkedTasksTab from "./LinkedTasksTab.svelte";
  import DatePicker from "./DatePicker.svelte";

  const queryClient = useQueryClient();

  const path = $derived(page.url.pathname);
  const noteId = $derived(path.startsWith("/p/") ? decodeURIComponent(path.slice(3)) : "");

  const focused = $derived(getActiveRegion() === "bottom");
  let rootEl = $state<HTMLElement | undefined>();
  let selectedNavIndex = $state(0);
  let panelContext = $state<"page" | "block">("page");

  const focusedBlock = $derived(getFocusedBlock());
  const tab = $derived(getBottomTab());

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: noteId !== "",
  }));
  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);

  // When a block from a different source note is focused (e.g., viewing /p/tasks
  // query page but focusing a block from phase3gqa.md), fetch the block's source
  // note separately. If focusedBlock.note_id === noteId (block from the current page),
  // reuse the main note to avoid double-fetching.
  const blockSourceNoteQuery = createQuery(() => ({
    queryKey: ["note", focusedBlock?.note_id ?? ""] as const,
    queryFn: () => api.getNote(focusedBlock!.note_id),
    enabled: !!focusedBlock && focusedBlock.note_id !== noteId,
  }));
  const blockSourceNote: Note | undefined = $derived(
    focusedBlock && focusedBlock.note_id === noteId
      ? note
      : (blockSourceNoteQuery.data as Note | undefined),
  );

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const allNotes = $derived((allNotesQuery.data ?? []) as Note[]);
  const propertyRegistry: PropertyRegistry = $derived.by(() => buildRegistry(allNotes));
  const inheritanceMap: InheritanceMap = $derived.by(() => buildInheritanceMap(allNotes));

  function hiddenChoicesForTags(tags: string[]): Record<string, string[]> {
    const merged: Record<string, string[]> = {};
    const resolved = new Set<string>();
    for (const tag of tags) {
      for (const t of resolveTagChain(tag, inheritanceMap)) resolved.add(t);
    }
    for (const tagName of resolved) {
      const tagPage = allNotes.find(
        (n) => n.title.toLowerCase() === tagName && n.metadata.note_type === "Tag",
      );
      if (tagPage) {
        const tagHidden = parseHiddenChoices(tagPage.metadata.custom);
        for (const [key, vals] of Object.entries(tagHidden)) {
          merged[key] = [...(merged[key] ?? []), ...vals];
        }
      }
    }
    return merged;
  }

  const hiddenChoices = $derived.by(() => {
    if (!note) return {};
    if (note.metadata.note_type === "Tag") return parseHiddenChoices(note.metadata.custom);
    return hiddenChoicesForTags(note.metadata.tags);
  });
  const blockHiddenChoices = $derived.by(() => {
    if (!focusedBlock) return {};
    const direct = focusedBlock.tags;
    const inherited = focusedBlock.inherited_tags ?? [];
    const allBlockTags = [...new Set([...direct, ...inherited])];
    const tags = allBlockTags.length > 0 ? allBlockTags : (note?.metadata.tags ?? []);
    return hiddenChoicesForTags(tags);
  });

  const HIDDEN_PAGE_KEYS = new Set([
    "extends",
    "tag_properties",
    "value_type",
    "choices",
    "default",
    "hide_by_default",
    "hide_empty",
    "icon",
    "color",
    "title",
  ]);

  const customProperties = $derived.by(() => {
    if (!note) return [];
    const out: { key: string; value: string }[] = [];
    for (const [key, value] of Object.entries(note.metadata.custom)) {
      const lower = key.toLowerCase();
      if (HIDDEN_PAGE_KEYS.has(lower)) continue;
      if (lower.startsWith("hidden_")) continue;
      if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        out.push({ key, value: String(value) });
      }
    }
    return out;
  });
  function extractBody(content: string): string {
    if (!content.startsWith("---")) return content;
    const end = content.indexOf("---", 3);
    if (end === -1) return content;
    const after = content.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  }

  // Re-derive block properties from the block's SOURCE note content, not from the
  // focused-block store snapshot. The store is set when the user focuses a
  // block in the editor; if we read its `properties` directly, an
  // updateBlockProperty save (which only writes the note content + cache)
  // doesn't refresh the chip values until the user re-focuses the block.
  // Looking the block up by id in the freshly-parsed source note body fixes that.
  // blockSourceNote is fetched from focusedBlock.note_id and handles the case
  // where a Query page is viewed but a block from a different source note is focused.
  const blockProperties = $derived.by(() => {
    if (!focusedBlock) return [];
    const sourceNote = blockSourceNote;
    if (!sourceNote) {
      // Source note hasn't loaded yet — fall back to the snapshot. This is rare
      // (only the first render before the query resolves) and matches the
      // pre-fix behavior.
      return Object.entries(focusedBlock.properties).map(([key, value]) => ({ key, value }));
    }
    const live = parseBlocks(sourceNote.id, extractBody(sourceNote.content)).find(
      (b) => b.id === focusedBlock.id,
    );
    const source = live ?? focusedBlock;
    return Object.entries(source.properties).map(([key, value]) => ({ key, value }));
  });

  // Backlinks
  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", noteId] as const,
    queryFn: () => api.getBacklinks(noteId),
    enabled: noteId !== "",
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: noteId !== "",
  }));
  const backlinks: Link[] = $derived((backlinksQuery.data ?? []) as Link[]);
  const edges: GraphEdge[] = $derived((edgesQuery.data ?? []) as GraphEdge[]);
  const incomingFromEdges = $derived(
    edges
      .filter((e) => e.target.toLowerCase() === noteId.toLowerCase() || e.target === noteId)
      .map((e) => e.source),
  );
  const allBacklinkSources = $derived.by(() => {
    const fromApi = new Set(backlinks.map((l) => l.target));
    return [...new Set([...fromApi, ...incomingFromEdges])];
  });

  // Outline = top-level blocks of focused note (or the drilled subtree)
  const noteBody = $derived.by(() => {
    if (!note) return "";
    const c = note.content;
    if (!c.startsWith("---")) return c;
    const end = c.indexOf("---", 3);
    if (end === -1) return c;
    const after = c.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  });
  const outlineBlocks = $derived(note ? parseBlocks(note.id, noteBody) : []);

  // Edit state for properties tab
  let editingKey = $state<string | null>(null);
  let editingValue = $state("");
  let editingBlockKey = $state<string | null>(null);
  let editingBlockValue = $state("");

  // Phase 9.7 — keyboard nav for the Properties tab. j/k cycles through
  // `flatProperties` (the current panel's list — block or page), Enter opens
  // edit mode for the selected chip, Tab commits + advances. Tag chips on
  // the page panel are display-only and not part of the navigation.
  let selectedPropertyIndex = $state(0);
  const flatProperties = $derived(
    panelContext === "block" ? blockProperties : customProperties,
  );
  $effect(() => {
    if (selectedPropertyIndex >= flatProperties.length) {
      selectedPropertyIndex = Math.max(0, flatProperties.length - 1);
    }
  });

  async function savePageProperty(key: string, newValue: string) {
    editingKey = null;
    if (note && newValue.trim() !== "") {
      const serialized = `"${newValue.trim().replace(/"/g, '\\"')}"`;
      const updated = await api.updateNote(noteId, updateFrontmatterKey(note.content, key, serialized));
      queryClient.setQueryData(["note", noteId], updated);
    }
    requestAnimationFrame(() => rootEl?.focus());
  }
  async function saveBlockProperty(key: string, newValue: string) {
    editingBlockKey = null;
    if (focusedBlock && newValue.trim() !== "") {
      await updateBlockProperty({
        block: focusedBlock,
        propKey: key,
        value: newValue.trim(),
        tagName: note?.metadata.note_type === "Tag" ? (note.title ?? "") : "",
        queryClient,
      });
    }
    requestAnimationFrame(() => rootEl?.focus());
  }

  function enterEditOnCurrent() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (
      def?.value_type === "select" ||
      def?.value_type === "multi-select" ||
      def?.value_type === "date" ||
      def?.value_type === "checkbox"
    ) {
      const chip = rootEl?.querySelector(
        `[data-prop-index="${selectedPropertyIndex}"][data-prop-context="${panelContext}"]`,
      );
      const ctrl = chip?.querySelector("select, input") as HTMLElement | null;
      ctrl?.focus();
      return;
    }
    if (panelContext === "block") {
      editingBlockKey = prop.key;
      editingBlockValue = prop.value;
    } else {
      editingKey = prop.key;
      editingValue = prop.value;
    }
    requestAnimationFrame(() => {
      const chip = rootEl?.querySelector(
        `[data-prop-index="${selectedPropertyIndex}"][data-prop-context="${panelContext}"]`,
      );
      const input = chip?.querySelector("input");
      if (input instanceof HTMLInputElement) {
        input.focus();
        input.select();
      }
    });
  }
  // Vim-like one-keystroke editors. `Space` (or h/l) cycles a select's
  // value in NAV mode; `x` clears a block property; `Space` toggles a
  // checkbox. Keeps the drawer keyboard-only — no need to focus the inner
  // <select> or open a popup just to flip a status.
  function cycleSelectValue(direction: 1 | -1) {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (!def || (def.value_type !== "select" && def.value_type !== "multi-select")) return;
    const hidden = panelContext === "block" ? blockHiddenChoices : hiddenChoices;
    const choices = getVisibleChoices(def, hidden);
    if (choices.length === 0) return;
    const currentIdx = choices.indexOf(prop.value);
    const nextIdx = ((currentIdx + direction) % choices.length + choices.length) % choices.length;
    const nextVal = choices[nextIdx];
    if (panelContext === "block") void saveBlockProperty(prop.key, nextVal);
    else void savePageProperty(prop.key, nextVal);
  }

  function toggleCheckboxValue() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (def?.value_type !== "checkbox") return;
    const next = prop.value === "true" ? "false" : "true";
    if (panelContext === "block") void saveBlockProperty(prop.key, next);
    else void savePageProperty(prop.key, next);
  }

  // ── Chord-letter machinery ───────────────────────────────────────
  // Phase 12.2 — chord assignment now goes through `assignChords` so the
  // drawer, slash menu, and any future surface land on the same letters
  // for the same property. Property pages declare `chord_key:` (and
  // `value_chord_keys:` for select choices); the helper resolves
  // collisions and tags conflicts so the chord menu can warn.
  const RESERVED_DRAWER_KEYS: ReadonlySet<string> = new Set([
    "j", "k", "h", "l", "x", "g",
  ]);
  function deriveValueChords(choices: string[], def?: PropertyDefinition): Map<string, string> {
    const items = choices.map((c) => ({
      name: c,
      preferred: def?.value_chord_keys[c.toLowerCase()] ?? null,
    }));
    const map = new Map<string, string>();
    for (const a of assignChords(items, { reserved: RESERVED_DRAWER_KEYS })) {
      map.set(a.name, a.key);
    }
    return map;
  }
  /**
   * Phase 12.2 — chord assignment for the add-property picker. Each name
   * looks up its declared `chord_key:` from the registry; collisions fall
   * back to first-letter. Distinct from `deriveValueChords` (which works
   * within one property's choice list).
   */
  function derivePropertyChords(names: string[]): Map<string, string> {
    const items = names.map((n) => ({
      name: n,
      preferred: propertyRegistry.get(n.toLowerCase())?.chord_key ?? null,
    }));
    const map = new Map<string, string>();
    for (const a of assignChords(items, { reserved: RESERVED_DRAWER_KEYS })) {
      map.set(a.name, a.key);
    }
    return map;
  }
  const propertyChords: Map<string, string> = $derived.by(() => {
    const items = flatProperties.map((p) => ({
      name: p.key,
      preferred: propertyRegistry.get(p.key.toLowerCase())?.chord_key ?? null,
    }));
    const map = new Map<string, string>();
    for (const a of assignChords(items, { reserved: RESERVED_DRAWER_KEYS })) {
      map.set(a.name, a.key);
    }
    return map;
  });

  // ── Picker state ─────────────────────────────────────────────────
  // The Linear-style inline picker. When `pickerOpen`, a list of choices
  // is rendered below the focused property's chip. Letter chord, j/k +
  // Enter, or click commits a value.
  let pickerOpen = $state(false);
  let pickerHighlightIdx = $state(0);

  function openPickerForCurrent() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (!def) return;
    if (def.value_type === "select" || def.value_type === "multi-select") {
      const choices = getVisibleChoices(def, panelContext === "block" ? blockHiddenChoices : hiddenChoices);
      pickerHighlightIdx = Math.max(0, choices.indexOf(prop.value));
      pickerOpen = true;
    } else if (def.value_type === "checkbox") {
      toggleCheckboxValue();
    } else if (def.value_type === "date") {
      const el = rootEl?.querySelector(
        `[data-prop-index="${selectedPropertyIndex}"][data-prop-context="${panelContext}"]`,
      ) as HTMLElement | null;
      openDateEditor(prop.key, el);
    } else {
      enterEditOnCurrent();
    }
  }

  // Date-editor popup state. Reuses the existing DatePicker (NL input +
  // calendar grid) so the user gets natural-language entry plus keyboard
  // grid nav with Tab → hjkl. Anchored to the focused property's chip.
  let dateEditPropKey = $state<string | null>(null);
  let dateEditPosition = $state({ x: 0, y: 0 });

  function openDateEditor(propKey: string, anchorEl: HTMLElement | null) {
    if (anchorEl) {
      const rect = anchorEl.getBoundingClientRect();
      // Estimate the picker's footprint so we can flip it above when the
      // anchor is too close to the bottom edge (the drawer hugs the bottom
      // of the viewport, so this is the common case).
      const PICKER_W = 280;
      // Phase 12.2 added the repeat sub-row + optional custom input,
      // pushing the picker taller. Estimated worst case (custom open).
      const PICKER_H = 400;
      const margin = 8;
      const fitsBelow = rect.bottom + PICKER_H + margin <= window.innerHeight;
      const y = fitsBelow ? rect.bottom + 4 : Math.max(margin, rect.top - PICKER_H - 4);
      const x = Math.min(rect.left, window.innerWidth - PICKER_W - margin);
      dateEditPosition = { x: Math.max(margin, x), y };
    } else {
      dateEditPosition = { x: 200, y: 200 };
    }
    dateEditPropKey = propKey;
  }

  function commitDateEditor(iso: string, time: string | null, recurrence: string | null) {
    if (!dateEditPropKey) return;
    const stored = formatDateValue(iso, time);
    if (panelContext === "block") void saveBlockProperty(dateEditPropKey, stored);
    else void savePageProperty(dateEditPropKey, stored);
    // Phase 12.2 — when the picker emits a recurrence, sync `recurring::`
    // on the same block. Null = explicit "none" → clear if it existed.
    // Page-level recurrence is out of scope for v1.
    if (panelContext === "block" && focusedBlock) {
      const existing = blockProperties.find((p) => p.key.toLowerCase() === "recurring");
      if (recurrence !== null) {
        void saveBlockProperty("recurring", recurrence);
      } else if (existing) {
        void clearBlockProperty({
          block: focusedBlock,
          propKey: "recurring",
          tagName: note?.metadata.note_type === "Tag" ? (note.title ?? "") : "",
          queryClient,
        });
      }
    }
    dateEditPropKey = null;
  }

  function commitPickerValue(propKey: string, choice: string) {
    if (panelContext === "block") void saveBlockProperty(propKey, choice);
    else void savePageProperty(propKey, choice);
    pickerOpen = false;
  }

  // ── Add-property picker ──────────────────────────────────────────
  // Lists tag_properties from the block's (or page's) resolved tag chain
  // that aren't yet set, so the user can add a new property without
  // leaving the drawer. Only addable types (select / multi-select / date
  // / checkbox) appear in v1; text-like types are still added by typing
  // `key:: value` in the editor.
  let addPickerOpen = $state(false);
  let addPickerHighlightIdx = $state(0);
  let pendingAddedKey = $state<string | null>(null);

  function tagPropertiesFromTags(tags: string[]): string[] {
    const resolved = new Set<string>();
    for (const t of tags) for (const r of resolveTagChain(t, inheritanceMap)) resolved.add(r);
    const out = new Set<string>();
    for (const tagName of resolved) {
      const tagPage = allNotes.find(
        (n) => n.title.toLowerCase() === tagName && n.metadata.note_type === "Tag",
      );
      if (!tagPage) continue;
      const tp = tagPage.metadata.custom["tag_properties"];
      if (Array.isArray(tp)) for (const p of tp) if (typeof p === "string") out.add(p);
    }
    return [...out];
  }

  const ADDABLE_TYPES = new Set(["select", "multi-select", "date", "checkbox"]);

  const availableProperties = $derived.by(() => {
    let candidates: string[] = [];
    let alreadySet: Set<string>;
    if (panelContext === "block") {
      if (!focusedBlock) return [];
      const direct = focusedBlock.tags;
      const inherited = focusedBlock.inherited_tags ?? [];
      candidates = tagPropertiesFromTags([...new Set([...direct, ...inherited])]);
      alreadySet = new Set(blockProperties.map((p) => p.key.toLowerCase()));
    } else {
      if (!note) return [];
      candidates = tagPropertiesFromTags(note.metadata.tags);
      alreadySet = new Set(customProperties.map((p) => p.key.toLowerCase()));
    }
    return candidates.filter((p) => {
      if (alreadySet.has(p.toLowerCase())) return false;
      const def = propertyRegistry.get(p.toLowerCase());
      return def && ADDABLE_TYPES.has(def.value_type);
    });
  });

  function todayIsoLocal(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  async function addProperty(key: string) {
    const def = propertyRegistry.get(key.toLowerCase());
    if (!def) return;
    let initialValue = "";
    if (def.value_type === "select" || def.value_type === "multi-select") {
      const hidden = panelContext === "block" ? blockHiddenChoices : hiddenChoices;
      const choices = getVisibleChoices(def, hidden);
      initialValue = choices[0] ?? "";
    } else if (def.value_type === "date") {
      initialValue = `[[${todayIsoLocal()}]]`;
    } else if (def.value_type === "checkbox") {
      initialValue = "false";
    }
    if (!initialValue) return;
    if (panelContext === "block") await saveBlockProperty(key, initialValue);
    else await savePageProperty(key, initialValue);
    addPickerOpen = false;
    pendingAddedKey = key;
  }

  // After save+refetch, focus the newly-added property and (for selects)
  // pop its picker so the user can refine the auto-defaulted value.
  $effect(() => {
    if (!pendingAddedKey) return;
    const idx = flatProperties.findIndex((p) => p.key.toLowerCase() === pendingAddedKey!.toLowerCase());
    if (idx < 0) return;
    selectedPropertyIndex = idx;
    const prop = flatProperties[idx];
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (def && (def.value_type === "select" || def.value_type === "multi-select")) {
      const hidden = panelContext === "block" ? blockHiddenChoices : hiddenChoices;
      const choices = getVisibleChoices(def, hidden);
      pickerHighlightIdx = Math.max(0, choices.indexOf(prop.value));
      pickerOpen = true;
    } else if (def?.value_type === "date") {
      // Auto-open the date editor anchored to the new chip so the user
      // can refine the auto-defaulted today's date.
      requestAnimationFrame(() => {
        const el = rootEl?.querySelector(
          `[data-prop-index="${idx}"][data-prop-context="${panelContext}"]`,
        ) as HTMLElement | null;
        openDateEditor(prop.key, el);
      });
    }
    pendingAddedKey = null;
  });

  function clearCurrentProperty() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    if (panelContext !== "block" || !focusedBlock) return;
    void clearBlockProperty({
      block: focusedBlock,
      propKey: prop.key,
      tagName: note?.metadata.note_type === "Tag" ? (note.title ?? "") : "",
      queryClient,
    });
  }

  function isSelectType(def: PropertyDefinition | undefined): boolean {
    return def?.value_type === "select" || def?.value_type === "multi-select";
  }
  function inputTypeFor(def: PropertyDefinition | undefined): string {
    switch (def?.value_type) {
      case "number": return "number";
      case "url": return "url";
      case "email": return "email";
      case "phone": return "tel";
      case "date": return "date";
      default: return "text";
    }
  }
  /**
   * Phase 10.5 — date-property values are persisted as `[[YYYY-MM-DD]]`
   * wiki-links (so they show in the daily-page backlink calendar). HTML
   * `<input type="date">` only accepts the bare `YYYY-MM-DD` form, so the
   * drawer needs to strip the brackets when reading and re-wrap when
   * writing. This pair was the missing piece behind "drawer date input
   * was empty even though the chip showed Apr 15."
   */
  function stripDateBrackets(v: string): string {
    const m = v.trim().match(/^\[\[(\d{4}-\d{2}-\d{2})\]\]$/);
    return m ? m[1] : v.trim();
  }
  function wrapDateBrackets(v: string): string {
    if (!v) return "";
    return /^\d{4}-\d{2}-\d{2}$/.test(v) ? `[[${v}]]` : v;
  }
  /**
   * Date properties optionally carry a time. Storage format:
   *   `[[YYYY-MM-DD]]`            (date only)
   *   `[[YYYY-MM-DD]] HH:mm`      (with 24-hour time)
   * Both halves round-trip through these helpers.
   */
  function parseDateValue(v: string): { date: string; time: string | null } {
    const m = v.trim().match(/^\[\[(\d{4}-\d{2}-\d{2})\]\](?:\s+(\d{2}:\d{2}))?$/);
    if (!m) return { date: v.trim(), time: null };
    return { date: m[1], time: m[2] ?? null };
  }
  function formatDateValue(date: string, time: string | null): string {
    if (!date) return "";
    const wrapped = /^\d{4}-\d{2}-\d{2}$/.test(date) ? `[[${date}]]` : date;
    return time ? `${wrapped} ${time}` : wrapped;
  }
  function format12h(time: string): string {
    const [hStr, mStr] = time.split(":");
    let h = Number(hStr);
    const ampm = h >= 12 ? "PM" : "AM";
    h = h % 12 || 12;
    return `${h}:${mStr} ${ampm}`;
  }
  function displayDateValue(v: string): string {
    const { date, time } = parseDateValue(v);
    if (!date) return "—";
    return time ? `${date} · ${format12h(time)}` : date;
  }
  // Inline-input keydown contract:
  //   Enter  → commit + close edit mode, focus drawer (j/k navigates again)
  //   Esc    → bail (no save), close edit mode, focus drawer
  //   Tab    → bail too (don't move-and-advance — the user prefers explicit
  //            j/k navigation between chips)
  //   stopPropagation everywhere so the drawer's own Tab-cycles-tabs and
  //   Enter-enters-edit handlers don't double-fire on the same bubble.
  function handlePageKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      savePageProperty(key, editingValue);
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      editingKey = null;
      requestAnimationFrame(() => rootEl?.focus());
    }
  }
  function handleBlockKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      saveBlockProperty(key, editingBlockValue);
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      editingBlockKey = null;
      requestAnimationFrame(() => rootEl?.focus());
    }
  }

  // Auto-track focusedBlock: when a block is focused, default to the block
  // panel (otherwise the user has to click the "view: block" segment to see
  // their block's status/priority/etc.). When no block is focused, fall
  // back to page. The user can still click the segment to override.
  let lastFocusedId = $state<string | null>(null);
  $effect(() => {
    const id = focusedBlock?.id ?? null;
    if (id !== lastFocusedId) {
      lastFocusedId = id;
      panelContext = id ? "block" : "page";
    }
  });

  // Lightweight count-only fetches for the tab badges. Cheap and reactive.
  const versionsCountQuery = createQuery(() => ({
    queryKey: ["note-versions", noteId, "count"] as const,
    queryFn: () => api.listNoteVersions(noteId, 200),
    enabled: noteId !== "",
  }));
  const versionsCount = $derived(versionsCountQuery.data?.length ?? 0);

  const linkedTasksCountQuery = createQuery(() => ({
    queryKey: ["linked-tasks", noteId, "count"] as const,
    queryFn: () =>
      api.executeQuery(`kind:block tag:Task has-link:${noteId}`, null, null),
    enabled: noteId !== "",
  }));
  const linkedTasksCount = $derived(
    linkedTasksCountQuery.data?.groups?.reduce((acc, g) => acc + g.items.length, 0) ?? 0,
  );

  type TabSpec = { id: BottomTab; label: string; n: number };
  const tabSpecs = $derived<TabSpec[]>([
    { id: "backlinks", label: "Backlinks", n: allBacklinkSources.length },
    { id: "properties", label: "Properties", n: customProperties.length + blockProperties.length },
    { id: "outline", label: "Outline", n: outlineBlocks.length },
    { id: "history", label: "History", n: versionsCount },
    { id: "linkedTasks", label: "Linked tasks", n: linkedTasksCount },
  ]);

  function cycleTab(direction: 1 | -1) {
    const idx = tabSpecs.findIndex((t) => t.id === tab);
    const next = (idx + direction + tabSpecs.length) % tabSpecs.length;
    setBottomTab(tabSpecs[next].id);
  }

  $effect(() => {
    if (focused) {
      if (rootEl && document.activeElement !== rootEl) rootEl.focus();
    } else if (rootEl && document.activeElement === rootEl) {
      rootEl.blur();
    }
  });

  $effect(() => {
    if (selectedNavIndex >= allBacklinkSources.length) {
      selectedNavIndex = Math.max(0, allBacklinkSources.length - 1);
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!focused) return;
    if (e.key === "Tab") {
      // While editing a property inline, Tab is "commit + advance" — let the
      // input's onkeydown handle it (handleBlockKeydown / handlePageKeydown).
      if (editingKey !== null || editingBlockKey !== null) return;
      e.preventDefault();
      cycleTab(e.shiftKey ? -1 : 1);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      // Blur whatever is focused inside the drawer (e.g. a `<select>` the
      // user opened to edit a property). Without this, document focus stays
      // on the drawer element and continues consuming keys via native
      // typeahead even though the active region has flipped back to "focus".
      (document.activeElement as HTMLElement | null)?.blur();
      setActiveRegion("focus");
      return;
    }
    if (tab === "backlinks") {
      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        selectedNavIndex = Math.min(allBacklinkSources.length - 1, selectedNavIndex + 1);
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        selectedNavIndex = Math.max(0, selectedNavIndex - 1);
      } else if (e.key === "Enter" && allBacklinkSources[selectedNavIndex]) {
        e.preventDefault();
        const src = allBacklinkSources[selectedNavIndex];
        goto(`/p/${encodeURIComponent(src.toLowerCase())}`);
        setActiveRegion("focus");
      }
    } else if (tab === "properties") {
      // While editing an inline input, that input owns its keys (handlePage/BlockKeydown).
      if (editingKey !== null || editingBlockKey !== null) return;
      const prop = flatProperties[selectedPropertyIndex];
      const def = prop ? propertyRegistry.get(prop.key.toLowerCase()) : undefined;
      const isSelect = def?.value_type === "select" || def?.value_type === "multi-select";
      const isCheckbox = def?.value_type === "checkbox";

      // ADD-PICKER MODE — choosing which property to add.
      if (addPickerOpen) {
        const props = availableProperties;
        if (e.key === "Escape") { e.preventDefault(); addPickerOpen = false; return; }
        if (e.key === "j" || e.key === "ArrowDown") {
          e.preventDefault();
          addPickerHighlightIdx = Math.min(props.length - 1, addPickerHighlightIdx + 1);
          return;
        }
        if (e.key === "k" || e.key === "ArrowUp") {
          e.preventDefault();
          addPickerHighlightIdx = Math.max(0, addPickerHighlightIdx - 1);
          return;
        }
        if (e.key === "Enter") {
          e.preventDefault();
          const key = props[addPickerHighlightIdx];
          if (key) void addProperty(key);
          return;
        }
        const addChords = derivePropertyChords(props);
        const matched = [...addChords.entries()].find(([, ch]) => ch === e.key)?.[0];
        if (matched) {
          e.preventDefault();
          void addProperty(matched);
          return;
        }
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          e.preventDefault();
        }
        return;
      }

      // PICKER MODE — when a select picker is open, all keys belong to it.
      if (pickerOpen && def && isSelect) {
        const choices = getVisibleChoices(def, panelContext === "block" ? blockHiddenChoices : hiddenChoices);
        if (e.key === "Escape") { e.preventDefault(); pickerOpen = false; return; }
        if (e.key === "j" || e.key === "ArrowDown") {
          e.preventDefault();
          pickerHighlightIdx = Math.min(choices.length - 1, pickerHighlightIdx + 1);
          return;
        }
        if (e.key === "k" || e.key === "ArrowUp") {
          e.preventDefault();
          pickerHighlightIdx = Math.max(0, pickerHighlightIdx - 1);
          return;
        }
        if (e.key === "Enter") {
          e.preventDefault();
          const choice = choices[pickerHighlightIdx];
          if (choice && prop) commitPickerValue(prop.key, choice);
          return;
        }
        // Letter chord → commit that choice directly.
        const valChords = deriveValueChords(choices, def);
        const matched = [...valChords.entries()].find(([, ch]) => ch === e.key)?.[0];
        if (matched && prop) {
          e.preventDefault();
          commitPickerValue(prop.key, matched);
          return;
        }
        // Swallow unmatched letters to avoid leaking to the leader handler.
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          e.preventDefault();
        }
        return;
      }

      // NAV MODE — list of properties.
      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        if (flatProperties.length > 0) {
          selectedPropertyIndex = Math.min(flatProperties.length - 1, selectedPropertyIndex + 1);
        }
        pickerOpen = false;
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        if (flatProperties.length > 0) {
          selectedPropertyIndex = Math.max(0, selectedPropertyIndex - 1);
        }
        pickerOpen = false;
      } else if (e.key === "g") {
        e.preventDefault();
        selectedPropertyIndex = 0;
        pickerOpen = false;
      } else if (e.key === "G") {
        e.preventDefault();
        selectedPropertyIndex = Math.max(0, flatProperties.length - 1);
        pickerOpen = false;
      } else if (e.key === " " || e.key === "Enter") {
        // Open the picker for the current property (or toggle checkbox / inline-edit text).
        e.preventDefault();
        if (prop) openPickerForCurrent();
      } else if (e.key === "l" && isSelect) {
        e.preventDefault();
        cycleSelectValue(1);
      } else if (e.key === "h" && isSelect) {
        e.preventDefault();
        cycleSelectValue(-1);
      } else if (e.key === "x") {
        e.preventDefault();
        clearCurrentProperty();
      } else if (e.key === "N" && availableProperties.length > 0) {
        // Capital N: open the add-property picker. Lowercase letters are
        // reserved for property chord activation, so we use the shifted
        // form for this meta-action.
        e.preventDefault();
        addPickerHighlightIdx = 0;
        addPickerOpen = true;
      } else {
        // Property-chord activation: jump to the named property AND open
        // its picker / edit. Two-keystroke edits with the value chord
        // (e.g. `s` then `D` = status:done).
        const matchedIdx = flatProperties.findIndex((p) => propertyChords.get(p.key) === e.key);
        if (matchedIdx >= 0) {
          e.preventDefault();
          selectedPropertyIndex = matchedIdx;
          openPickerForCurrent();
        }
      }
    }
  }

  function clickOutline(blockId: string) {
    if (!note) return;
    goto(`/p/${encodeURIComponent(note.id)}?block=${encodeURIComponent(blockId)}`);
    setActiveRegion("focus");
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="v9-bottom"
  tabindex="0"
  onfocus={() => setActiveRegion("bottom")}
  onclick={() => setActiveRegion("bottom")}
  onkeydown={handleKeydown}
  style="outline: none;"
>
  <div class="tabs">
    {#each tabSpecs as t}
      <span
        class="tab {t.id === tab ? 'active' : ''}"
        onclick={(e) => { e.stopPropagation(); setBottomTab(t.id); setActiveRegion("bottom"); }}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setBottomTab(t.id); } }}
        role="tab"
        tabindex="-1"
      >
        {t.label} <span class="n">{t.n}</span>
      </span>
    {/each}
  </div>
  <div class="body">
    {#if !noteId}
      <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No note focused</div>
    {:else if tab === "backlinks"}
      {#if allBacklinkSources.length === 0}
        <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No pages link here</div>
      {:else}
        {#each allBacklinkSources as src, bi}
          {@const sel = focused && selectedNavIndex === bi}
          <div
            class="v9-bl-card"
            style="cursor: pointer; {sel ? 'background: var(--v9-bg-3);' : ''}"
            onclick={() => { goto(`/p/${encodeURIComponent(src.toLowerCase())}`); setActiveRegion("focus"); }}
            role="button"
            tabindex="-1"
          >
            <span class="src"><span class="lbl">{src}</span></span>
          </div>
        {/each}
      {/if}
    {:else if tab === "properties"}
      <!-- pg/blk segmented -->
      <div style="display: flex; gap: 6px; margin-bottom: 10px; font-family: var(--v9-mono); font-size: 10.5px;">
        <button
          class="pchip"
          style="cursor: pointer; {panelContext === 'page' ? 'color: var(--v9-amber); border-color: var(--v9-amber);' : ''}"
          onclick={(e) => { e.stopPropagation(); panelContext = 'page'; }}
        >
          <span class="k">view</span><span class="v">page</span>
        </button>
        <button
          class="pchip"
          style="cursor: pointer; {panelContext === 'block' ? 'color: var(--v9-amber); border-color: var(--v9-amber);' : ''} {!focusedBlock ? 'opacity: 0.4; cursor: not-allowed;' : ''}"
          onclick={(e) => { e.stopPropagation(); if (focusedBlock) panelContext = 'block'; }}
          disabled={!focusedBlock}
        >
          <span class="k">view</span><span class="v">block</span>
        </button>
      </div>

      {#if panelContext === "block"}
        {#if focusedBlock}
          {#if blockProperties.length > 0}
            <div class="props-list">
              {#each blockProperties as prop, pi}
                {@const def = propertyRegistry.get(prop.key.toLowerCase())}
                {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, blockHiddenChoices) : []}
                {@const propSelected = focused && tab === "properties" && panelContext === "block" && selectedPropertyIndex === pi}
                {@const propChord = propertyChords.get(prop.key)}
                {@const valChords = visibleChoices.length > 0 ? deriveValueChords(visibleChoices, def) : new Map()}
                <span
                  class="pchip {propSelected ? 'selected' : ''}"
                  data-prop-index={pi}
                  data-prop-context="block"
                >
                  {#if propChord}<kbd class="prop-chord">{propChord}</kbd>{/if}
                  <span class="k">{prop.key}</span>
                  {#if def?.value_type === "checkbox"}
                    <input
                      type="checkbox"
                      checked={prop.value === "true" || prop.value === "yes"}
                      onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLInputElement).checked ? "true" : "false")}
                    />
                  {:else if isSelectType(def)}
                    <button
                      class="value-chip"
                      type="button"
                      onclick={(e) => { e.stopPropagation(); selectedPropertyIndex = pi; if (propSelected && pickerOpen) pickerOpen = false; else openPickerForCurrent(); }}
                    >
                      <span>{prop.value || "—"}</span>
                      <span class="caret">▾</span>
                    </button>
                  {:else if def?.value_type === "date"}
                    <button
                      class="value-chip"
                      type="button"
                      onclick={(e) => {
                        e.stopPropagation();
                        selectedPropertyIndex = pi;
                        const anchor = (e.currentTarget as HTMLElement).closest(".pchip") as HTMLElement | null;
                        openDateEditor(prop.key, anchor);
                      }}
                    >
                      <span>{displayDateValue(prop.value)}</span>
                      <span class="caret">▾</span>
                    </button>
                  {:else if editingBlockKey === prop.key}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      autofocus
                      type={inputTypeFor(def)}
                      bind:value={editingBlockValue}
                      onkeydown={(e) => handleBlockKeydown(e, prop.key)}
                      style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-amber); font-family: var(--v9-mono); font-size: 11px;"
                    />
                  {:else}
                    <span
                      class="v"
                      style="cursor: text;"
                      onclick={(e) => { e.stopPropagation(); editingBlockKey = prop.key; editingBlockValue = prop.value; }}
                    >{prop.value}</span>
                  {/if}
                </span>
                {#if propSelected && pickerOpen && isSelectType(def) && visibleChoices.length > 0}
                  <div class="picker-popover">
                    {#each visibleChoices as choice, ci}
                      {@const ch = valChords.get(choice)}
                      {@const isCurrent = choice === prop.value}
                      {@const isHL = ci === pickerHighlightIdx}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <div
                        class="picker-row {isHL ? 'hl' : ''}"
                        onclick={() => commitPickerValue(prop.key, choice)}
                        onmouseenter={() => (pickerHighlightIdx = ci)}
                      >
                        <kbd class="val-chord">{ch ?? "·"}</kbd>
                        <span class="picker-label">{choice}</span>
                        {#if isCurrent}<span class="picker-check">✓</span>{/if}
                      </div>
                    {/each}
                  </div>
                {/if}
              {/each}
              {#if availableProperties.length > 0}
                <button
                  class="add-prop-row"
                  type="button"
                  onclick={(e) => { e.stopPropagation(); addPickerHighlightIdx = 0; addPickerOpen = !addPickerOpen; }}
                >
                  <kbd class="prop-chord">N</kbd>
                  <span class="add-label">+ Add property</span>
                </button>
                {#if addPickerOpen}
                  {@const addChords = derivePropertyChords(availableProperties)}
                  <div class="picker-popover">
                    {#each availableProperties as candidate, ci}
                      {@const ch = addChords.get(candidate)}
                      {@const isHL = ci === addPickerHighlightIdx}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <div
                        class="picker-row {isHL ? 'hl' : ''}"
                        onclick={() => void addProperty(candidate)}
                        onmouseenter={() => (addPickerHighlightIdx = ci)}
                      >
                        <kbd class="val-chord">{ch ?? "·"}</kbd>
                        <span class="picker-label">{candidate}</span>
                      </div>
                    {/each}
                  </div>
                {/if}
              {/if}
            </div>
          {:else if availableProperties.length > 0}
            <button
              class="add-prop-row"
              type="button"
              onclick={(e) => { e.stopPropagation(); addPickerHighlightIdx = 0; addPickerOpen = !addPickerOpen; }}
            >
              <kbd class="prop-chord">N</kbd>
              <span class="add-label">+ Add property</span>
            </button>
            {#if addPickerOpen}
              {@const addChords = derivePropertyChords(availableProperties)}
              <div class="picker-popover">
                {#each availableProperties as candidate, ci}
                  {@const ch = addChords.get(candidate)}
                  {@const isHL = ci === addPickerHighlightIdx}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="picker-row {isHL ? 'hl' : ''}"
                    onclick={() => void addProperty(candidate)}
                    onmouseenter={() => (addPickerHighlightIdx = ci)}
                  >
                    <kbd class="val-chord">{ch ?? "·"}</kbd>
                    <span class="picker-label">{candidate}</span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else}
            <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No block properties</div>
          {/if}
        {:else}
          <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">Focus a block to see its properties</div>
        {/if}
      {:else}
        {#if note}
          {#if note.metadata.tags.length > 0}
            <div style="display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 8px;">
              {#each note.metadata.tags as tagName}
                <a class="pchip" href="/p/{encodeURIComponent(tagName)}">
                  <span class="k">tag</span><span class="v">{tagName}</span>
                </a>
              {/each}
            </div>
          {/if}
          {#if customProperties.length > 0}
            <div class="props-list">
              {#each customProperties as prop, pi}
                {@const def = propertyRegistry.get(prop.key.toLowerCase())}
                {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, hiddenChoices) : []}
                {@const propSelected = focused && tab === "properties" && panelContext === "page" && selectedPropertyIndex === pi}
                {@const propChord = propertyChords.get(prop.key)}
                {@const valChords = visibleChoices.length > 0 ? deriveValueChords(visibleChoices, def) : new Map()}
                <span
                  class="pchip {propSelected ? 'selected' : ''}"
                  data-prop-index={pi}
                  data-prop-context="page"
                >
                  {#if propChord}<kbd class="prop-chord">{propChord}</kbd>{/if}
                  <span class="k">{prop.key}</span>
                  {#if def?.value_type === "checkbox"}
                    <input
                      type="checkbox"
                      checked={prop.value === "true" || prop.value === "yes"}
                      onchange={(e) => savePageProperty(prop.key, (e.target as HTMLInputElement).checked ? "true" : "false")}
                    />
                  {:else if isSelectType(def)}
                    <button
                      class="value-chip"
                      type="button"
                      onclick={(e) => { e.stopPropagation(); selectedPropertyIndex = pi; if (propSelected && pickerOpen) pickerOpen = false; else openPickerForCurrent(); }}
                    >
                      <span>{prop.value || "—"}</span>
                      <span class="caret">▾</span>
                    </button>
                  {:else if def?.value_type === "date"}
                    <button
                      class="value-chip"
                      type="button"
                      onclick={(e) => {
                        e.stopPropagation();
                        selectedPropertyIndex = pi;
                        const anchor = (e.currentTarget as HTMLElement).closest(".pchip") as HTMLElement | null;
                        openDateEditor(prop.key, anchor);
                      }}
                    >
                      <span>{displayDateValue(prop.value)}</span>
                      <span class="caret">▾</span>
                    </button>
                  {:else if editingKey === prop.key}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      autofocus
                      type={inputTypeFor(def)}
                      bind:value={editingValue}
                      onkeydown={(e) => handlePageKeydown(e, prop.key)}
                      style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-amber); font-family: var(--v9-mono); font-size: 11px;"
                    />
                  {:else}
                    <span
                      class="v"
                      style="cursor: text;"
                      onclick={(e) => { e.stopPropagation(); editingKey = prop.key; editingValue = prop.value; }}
                    >{prop.value}</span>
                  {/if}
                </span>
                {#if propSelected && pickerOpen && isSelectType(def) && visibleChoices.length > 0}
                  <div class="picker-popover">
                    {#each visibleChoices as choice, ci}
                      {@const ch = valChords.get(choice)}
                      {@const isCurrent = choice === prop.value}
                      {@const isHL = ci === pickerHighlightIdx}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <div
                        class="picker-row {isHL ? 'hl' : ''}"
                        onclick={() => commitPickerValue(prop.key, choice)}
                        onmouseenter={() => (pickerHighlightIdx = ci)}
                      >
                        <kbd class="val-chord">{ch ?? "·"}</kbd>
                        <span class="picker-label">{choice}</span>
                        {#if isCurrent}<span class="picker-check">✓</span>{/if}
                      </div>
                    {/each}
                  </div>
                {/if}
              {/each}
              {#if availableProperties.length > 0}
                <button
                  class="add-prop-row"
                  type="button"
                  onclick={(e) => { e.stopPropagation(); addPickerHighlightIdx = 0; addPickerOpen = !addPickerOpen; }}
                >
                  <kbd class="prop-chord">N</kbd>
                  <span class="add-label">+ Add property</span>
                </button>
                {#if addPickerOpen}
                  {@const addChords = derivePropertyChords(availableProperties)}
                  <div class="picker-popover">
                    {#each availableProperties as candidate, ci}
                      {@const ch = addChords.get(candidate)}
                      {@const isHL = ci === addPickerHighlightIdx}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <div
                        class="picker-row {isHL ? 'hl' : ''}"
                        onclick={() => void addProperty(candidate)}
                        onmouseenter={() => (addPickerHighlightIdx = ci)}
                      >
                        <kbd class="val-chord">{ch ?? "·"}</kbd>
                        <span class="picker-label">{candidate}</span>
                      </div>
                    {/each}
                  </div>
                {/if}
              {/if}
            </div>
          {:else if availableProperties.length > 0}
            <button
              class="add-prop-row"
              type="button"
              onclick={(e) => { e.stopPropagation(); addPickerHighlightIdx = 0; addPickerOpen = !addPickerOpen; }}
            >
              <kbd class="prop-chord">N</kbd>
              <span class="add-label">+ Add property</span>
            </button>
            {#if addPickerOpen}
              {@const addChords = derivePropertyChords(availableProperties)}
              <div class="picker-popover">
                {#each availableProperties as candidate, ci}
                  {@const ch = addChords.get(candidate)}
                  {@const isHL = ci === addPickerHighlightIdx}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="picker-row {isHL ? 'hl' : ''}"
                    onclick={() => void addProperty(candidate)}
                    onmouseenter={() => (addPickerHighlightIdx = ci)}
                  >
                    <kbd class="val-chord">{ch ?? "·"}</kbd>
                    <span class="picker-label">{candidate}</span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else if note.metadata.tags.length === 0}
            <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No page properties</div>
          {/if}
        {:else}
          <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">Loading…</div>
        {/if}
      {/if}
    {:else if tab === "outline"}
      {#if outlineBlocks.length === 0}
        <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No outline</div>
      {:else}
        {#each outlineBlocks as b}
          <div
            style="padding-left: {b.indent_level * 14}px; font-size: 12px; color: var(--v9-ink-2); padding-top: 3px; padding-bottom: 3px; cursor: pointer;"
            onclick={() => clickOutline(b.id)}
            role="button"
            tabindex="-1"
          >· {b.text || "(empty)"}</div>
        {/each}
      {/if}
    {:else if tab === "history"}
      <HistoryTab {noteId} />
    {:else if tab === "linkedTasks"}
      <LinkedTasksTab {noteId} />
    {/if}
  </div>
</div>

{#if dateEditPropKey}
  {@const editingProp = flatProperties.find((p) => p.key === dateEditPropKey)}
  {@const parsed = editingProp ? parseDateValue(editingProp.value) : { date: "", time: null }}
  {@const existingRecur = panelContext === "block"
    ? blockProperties.find((p) => p.key.toLowerCase() === "recurring")?.value ?? null
    : null}
  <DatePicker
    initialDate={parsed.date || undefined}
    initialTime={parsed.time}
    initialRecurrence={existingRecur}
    position={dateEditPosition}
    onPick={commitDateEditor}
    onClose={() => (dateEditPropKey = null)}
  />
{/if}

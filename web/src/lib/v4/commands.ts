/**
 * Prism v5 command verbs — the rows that appear in the Station's Palette
 * tab and the `:` ex-mode dispatcher.
 *
 * File still lives under `lib/v4/` for now to avoid mid-cutover import
 * churn; will move to `lib/v5/` in Phase 13.
 */

import { api } from "$lib/api-client";
import { getAppQueryClient } from "$lib/app-query-client.svelte";
import { getFocusedBlock } from "$lib/stores/current-block.svelte";
import { toast } from "$lib/stores/toast.svelte";
import { skipRecurrence } from "$lib/recurrence-actions";
import {
  closeFocusedLeaf,
  closeTab,
  getFocusedBuffer,
  getScratchPruneAfterDays,
  getWorkspace,
  hsplit,
  movePane,
  newTab,
  openPageInFocused,
  vsplit,
} from "$lib/buffer/state.svelte";
import { runScratchPrune } from "$lib/state/scratch-prune";
import {
  asPageId,
  type DerivedBinding,
} from "$lib/buffer/types";
import {
  makeAmbientBuffer,
  makeDerivedBuffer,
  makePageBuffer,
} from "$lib/buffer/tree";
import {
  openSettingsOverlay,
  type SettingsSlug,
} from "$lib/stores/fullscreen-overlay.svelte";

const SETTINGS_PAGES: { slug: SettingsSlug; label: string }[] = [
  { slug: "general", label: "General" },
  { slug: "devices", label: "Devices" },
  { slug: "sync", label: "Sync" },
  { slug: "mosaic", label: "Mosaic" },
  { slug: "data", label: "Data" },
];

const DERIVED_RENDERERS: { name: string; label: string; verb: string; glyph: string }[] = [
  { name: "backlinks-of-page", label: "Backlinks (follow)", verb: "backlinks", glyph: "↩" },
  { name: "outline-of-page", label: "Outline (follow)", verb: "outline", glyph: "⋮" },
  { name: "properties-of-page", label: "Properties (follow)", verb: "properties", glyph: "⚙" },
  { name: "tasks-linked-to-page", label: "Linked tasks (follow)", verb: "tasks", glyph: "☑" },
  { name: "local-graph-of-page", label: "Local graph (follow)", verb: "graph-local", glyph: "✦" },
];

const AMBIENTS: { name: string; label: string; verb: string; glyph: string }[] = [
  { name: "calendar", label: "Calendar", verb: "calendar", glyph: "📅" },
  { name: "today-in-progress", label: "Today in progress", verb: "in-progress", glyph: "⏱" },
  { name: "workspace-dashboard", label: "Workspace dashboard", verb: "dashboard", glyph: "▦" },
  { name: "ai-workspace", label: "AI workspace", verb: "ai", glyph: "✺" },
  { name: "agenda", label: "Agenda", verb: "agenda", glyph: "📋" },
];

export type V4Command = {
  id: string;
  verb?: string;
  label: string;
  glyph: string;
  category: "pane" | "tab" | "tile" | "create" | "navigate" | "derived" | "ambient";
  shortcut?: string;
  keywords: string[];
  argPrompt?: string;
  run: (arg?: string) => void | Promise<void>;
};

async function jumpToDaily() {
  const daily = await api.getDailyNote();
  openPageInFocused(asPageId(daily.id));
}

/** Validate + normalize a user-supplied date arg to YYYY-MM-DD. Accepts
 *  `today`, `yesterday`, `tomorrow`, a relative `-3d` / `+1d`, or an
 *  explicit YYYY-MM-DD. Returns null when unparseable. */
function resolveDateArg(arg: string): string | null {
  const a = arg.trim().toLowerCase();
  const today = new Date();
  today.setHours(12, 0, 0, 0);
  const fmt = (d: Date) => {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  };
  if (a === "today") return fmt(today);
  if (a === "yesterday") {
    const d = new Date(today);
    d.setDate(d.getDate() - 1);
    return fmt(d);
  }
  if (a === "tomorrow") {
    const d = new Date(today);
    d.setDate(d.getDate() + 1);
    return fmt(d);
  }
  const rel = a.match(/^([+-]?\d+)d$/);
  if (rel) {
    const d = new Date(today);
    d.setDate(d.getDate() + Number(rel[1]));
    return fmt(d);
  }
  if (/^\d{4}-\d{2}-\d{2}$/.test(a)) return a;
  return null;
}

async function jumpToDate(arg: string | undefined): Promise<void> {
  const target = arg ? resolveDateArg(arg) : null;
  if (!target) return;
  // getDailyNote(date) auto-creates the file if missing. After it lands
  // we open it as a page-buffer; the daily cascade decides whether to
  // render it as JournalView (anchored to this date) or a single-day
  // outliner based on pane size.
  const note = await api.getDailyNote(target);
  openPageInFocused(asPageId(note.id));
  const qc = getAppQueryClient();
  if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
}

async function createNoteAndJump(title: string) {
  const note = await api.createNote(title, "");
  openPageInFocused(asPageId(note.id));
}

async function createScratchAndJump() {
  // Auto-named by timestamp; `type: scratch` frontmatter so the page-type
  // dispatch can render the chip + so the sidebar/search filters can
  // hide it. Seed one empty bullet so BlockOutliner has a block to mount
  // a BlockEditor against — otherwise the user lands on "↓ to insert"
  // placeholder text with no cm-editor to type into.
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp = `scratch/${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
  const content = `---\ntitle: "${stamp}"\ntype: scratch\ntags: []\n---\n- \n`;
  const note = await api.createNote(stamp, content);
  openPageInFocused(asPageId(note.id));
}

/** Phase 13 + 15 — `:delete-tag` verb. Fetches usage counts, confirms via a
 *  two-step dialog (1: show counts + ask whether to also clean up refs;
 *  2: confirm final delete), then performs cleanup-if-requested followed by
 *  the note delete. */
async function deleteFocusedTag() {
  const buffer = getFocusedBuffer();
  if (!buffer || buffer.kind !== "page" || !buffer.pageId) {
    console.warn("delete-tag: no focused page");
    return;
  }
  const note = await api.getNote(buffer.pageId);
  const isTag = (note.metadata.note_type ?? "").toLowerCase() === "tag";
  if (!isTag) {
    console.warn("delete-tag: focused page is not a tag");
    return;
  }
  const usage = await api.getTagUsage(buffer.pageId);

  // Step 1 — show counts and ask about reference cleanup. `confirm` returns
  // true → also clean refs; false → leave them as broken text. (`prompt`
  // would let us collect text but we just want a yes/no, so the message
  // describes the consequence and OK = clean, Cancel = leave.)
  const summary =
    `Delete tag "${buffer.pageId}"?\n\n` +
    `References (#tag, [[tag]]):  ${usage.references}\n` +
    `Page instances:                ${usage.page_instances}\n` +
    `Block instances:               ${usage.block_instances}\n` +
    `Child tags (will be orphaned): ${usage.children}\n\n` +
    `Press OK to ALSO clean up references in the corpus\n` +
    `(strip #tag tokens, unwrap [[tag]] to plain text,\n` +
    `clear children's parent: frontmatter).\n\n` +
    `Press Cancel to leave references as-is.`;
  const cleanup = window.confirm(summary);

  // Step 2 — final confirmation.
  const finalPrompt = cleanup
    ? `Confirm: delete tag "${buffer.pageId}" AND clean up references.`
    : `Confirm: delete tag "${buffer.pageId}" (references will remain as broken tokens).`;
  if (!window.confirm(finalPrompt)) return;

  if (cleanup) {
    try {
      const result = await api.cleanupTagReferences(buffer.pageId, true);
      console.info(
        `cleanup-tag-references: stripped ${result.refs} ref(s) across ${result.notes} note(s)`,
      );
    } catch (e) {
      console.warn("cleanup-tag-references failed:", e);
      // Continue with the delete anyway — the user already confirmed.
    }
  }

  await fetch(`/api/notes/${encodeURIComponent(buffer.pageId)}`, {
    method: "DELETE",
  });
  const qc = getAppQueryClient();
  if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
}

/** Phase 13 — `:rename-slug` with preview/confirm/commit flow. */
async function renameFocusedTagSlug(toSlug: string) {
  const buffer = getFocusedBuffer();
  if (!buffer || buffer.kind !== "page" || !buffer.pageId) {
    console.warn("rename-slug: no focused page");
    return;
  }
  const note = await api.getNote(buffer.pageId);
  const isTag = (note.metadata.note_type ?? "").toLowerCase() === "tag";
  if (!isTag) {
    console.warn("rename-slug: focused page is not a tag");
    return;
  }
  const newSlug = toSlug.trim().toLowerCase();
  if (!newSlug || newSlug === buffer.pageId) return;

  // Phase 13 — preview rewrite counts, confirm with user, then commit.
  let preview;
  try {
    preview = await api.renameTagSlug(buffer.pageId, newSlug, false);
  } catch (e) {
    console.warn("rename-slug preview failed:", e);
    return;
  }
  const summary =
    `Rename tag "${buffer.pageId}" → "${newSlug}"?\n\n` +
    `Will rewrite ${preview.refs} reference(s) across ${preview.notes} note(s).\n` +
    `Plus move the tag's own file from ${buffer.pageId}.md to ${newSlug}.md.`;
  if (!window.confirm(summary)) return;

  await api.renameTagSlug(buffer.pageId, newSlug, true);
  openPageInFocused(asPageId(newSlug));
  const qc = getAppQueryClient();
  if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
}

/** Phase 14 — convert verbs. Round-trippable: a note → tag → note returns to
 *  the original `type: note` page with content and other frontmatter intact. */
async function convertFocusedTo(newType: "tag" | "note") {
  const buffer = getFocusedBuffer();
  if (!buffer || buffer.kind !== "page" || !buffer.pageId) {
    console.warn(`convert-to-${newType}: no focused page`);
    return;
  }
  const note = await api.getNote(buffer.pageId);
  const current = (note.metadata.note_type ?? "").toLowerCase();
  if (current === newType) return; // idempotent

  // Frontmatter rewrite: replace any existing `type: <value>` line, or
  // insert one right after the opening `---` if absent. Keeps the rest of
  // the frontmatter (parent, extends, tag_properties, etc.) intact so the
  // convert round-trips.
  let next = note.content;
  if (/^type:\s*.+$/m.test(next)) {
    next = next.replace(/^type:\s*.+$/m, `type: ${newType}`);
  } else if (next.startsWith("---\n")) {
    next = next.replace(/^---\n/, `---\ntype: ${newType}\n`);
  } else {
    // No frontmatter at all — synthesize one.
    next = `---\ntype: ${newType}\n---\n${next}`;
  }
  if (next === note.content) return;

  const updated = await api.updateNote(buffer.pageId, next);
  const qc = getAppQueryClient();
  if (qc) {
    qc.setQueryData(["note", buffer.pageId], updated);
    qc.invalidateQueries({ queryKey: ["notes"] });
  }
}

async function promoteFocusedScratch() {
  const buffer = getFocusedBuffer();
  if (!buffer || buffer.kind !== "page" || !buffer.pageId) return;
  const note = await api.getNote(buffer.pageId);
  // Strip `type: scratch` from the YAML frontmatter. Idempotent: a note
  // that doesn't have it is a no-op.
  const next = note.content.replace(/^type:\s*scratch\s*\n/m, "");
  if (next === note.content) return;
  const updated = await api.updateNote(buffer.pageId, next);
  // Refresh the TanStack cache so BufferShell + sidebar surfaces re-read
  // the updated note (chip disappears, scratch leaves the tree's hidden
  // bucket, etc.). Without this, the UI keeps the old note_type === "scratch"
  // value from the previous fetch.
  const qc = getAppQueryClient();
  if (qc) {
    qc.setQueryData(["note", buffer.pageId], updated);
    qc.invalidateQueries({ queryKey: ["notes"] });
  }
}

function followBinding(): DerivedBinding {
  return { mode: "follow" };
}

export function buildV4Commands(): V4Command[] {
  return [
    // ── pane ────────────────────────────────────────────────────────────
    {
      id: "vsplit",
      verb: "vsplit",
      label: "Split pane vertically",
      glyph: "│",
      category: "pane",
      shortcut: "⌘\\",
      keywords: ["split", "vsplit", "vertical", "right", "pane"],
      run: () => vsplit(makePageBuffer(asPageId(""))),
    },
    {
      id: "hsplit",
      verb: "hsplit",
      label: "Split pane horizontally",
      glyph: "─",
      category: "pane",
      shortcut: "⌘-",
      keywords: ["split", "hsplit", "horizontal", "below", "pane"],
      run: () => hsplit(makePageBuffer(asPageId(""))),
    },
    {
      id: "close-pane",
      verb: "quit",
      label: "Close focused pane",
      glyph: "×",
      category: "pane",
      shortcut: "⌘W",
      keywords: ["close", "quit", "kill", "pane"],
      run: () => closeFocusedLeaf(),
    },
    ...(["left", "right", "up", "down"] as const).map((dir) => ({
      id: `move-${dir}`,
      verb: `move-${dir}`,
      label: `Move pane ${dir}`,
      glyph:
        dir === "left" ? "←" : dir === "right" ? "→" : dir === "up" ? "↑" : "↓",
      category: "pane" as const,
      shortcut: `⌘⇧${dir === "left" ? "H" : dir === "right" ? "L" : dir === "up" ? "K" : "J"}`,
      keywords: ["move", "push", "send", dir, "pane"],
      run: () => movePane(dir),
    })),

    // ── tab ─────────────────────────────────────────────────────────────
    {
      id: "tabnew",
      verb: "tabnew",
      label: "New tab",
      glyph: "+",
      category: "tab",
      shortcut: "⌘T",
      keywords: ["tab", "new", "open", "window"],
      run: () => newTab(),
    },
    {
      id: "tab-close",
      verb: "tabclose",
      label: "Close current tab",
      glyph: "×",
      category: "tab",
      shortcut: "⌘⇧W",
      keywords: ["tab", "close", "kill"],
      run: () => closeTab(getWorkspace().activeTabId),
    },

    // ── tile (page-buffer ops) ──────────────────────────────────────────
    {
      id: "jump",
      verb: "jump",
      label: "Jump to page…",
      glyph: "→",
      category: "tile",
      keywords: ["jump", "go", "open", "tile", "note", "page"],
      argPrompt: "note slug or id",
      run: (arg) => {
        if (arg) openPageInFocused(asPageId(arg));
      },
    },

    // ── derived buffers ────────────────────────────────────────────────
    // Open horizontally beneath the focused page at a 70/30 ratio (page
    // up top, derived below). To eventually be configurable via settings.
    ...DERIVED_RENDERERS.map((d) => ({
      id: d.verb,
      verb: d.verb,
      label: `Open ${d.label}`,
      glyph: d.glyph,
      category: "derived" as const,
      keywords: [d.verb, "derived", "follow", d.name],
      run: () => hsplit(makeDerivedBuffer(d.name, followBinding()), 0.7),
    })),

    // ── tag derived buffers ──────────────────────────────────────────
    // Tag-typed derived renderers (`instances-of-tag`, `backlinks-of-tag`).
    // Argument is the tag slug — pinned binding is used since there's no
    // tag-follow source yet (lastFocusedTagPerTab is not tracked).
    {
      id: "instances-of-tag",
      verb: "instances-of-tag",
      label: "Open instances of a tag (pinned)",
      glyph: "▦",
      category: "derived" as const,
      keywords: ["instances", "tag", "members", "uses"],
      argPrompt: "tag slug",
      run: (arg) => {
        if (!arg) return;
        const reference = { kind: "tag" as const, value: arg.toLowerCase() };
        hsplit(
          makeDerivedBuffer("instances-of-tag", { mode: "pinned", reference }),
          0.7,
        );
      },
    },
    {
      id: "backlinks-of-tag",
      verb: "backlinks-of-tag",
      label: "Open backlinks of a tag (pinned)",
      glyph: "↩",
      category: "derived" as const,
      keywords: ["backlinks", "tag", "uses"],
      argPrompt: "tag slug",
      run: (arg) => {
        if (!arg) return;
        const reference = { kind: "tag" as const, value: arg.toLowerCase() };
        hsplit(
          makeDerivedBuffer("backlinks-of-tag", { mode: "pinned", reference }),
          0.7,
        );
      },
    },

    // ── ambient buffers ────────────────────────────────────────────────
    ...AMBIENTS.map((a) => ({
      id: a.verb,
      verb: a.verb,
      label: `Open ${a.label}`,
      glyph: a.glyph,
      category: "ambient" as const,
      keywords: [a.verb, "ambient", a.name],
      run: () => vsplit(makeAmbientBuffer(a.name)),
    })),

    // ── create / navigate ───────────────────────────────────────────────
    {
      id: "daily",
      verb: "daily",
      label: "Today's daily note",
      glyph: "☀",
      category: "navigate",
      keywords: ["daily", "today", "journal"],
      run: () => jumpToDaily(),
    },
    {
      id: "goto",
      verb: "goto",
      label: "Jump to a date (YYYY-MM-DD, today, yesterday, tomorrow, ±Nd)",
      glyph: "→",
      category: "navigate",
      keywords: ["goto", "go", "jump", "date", "day", "daily"],
      argPrompt: "date: YYYY-MM-DD, today, yesterday, tomorrow, ±Nd",
      run: (arg) => {
        if (arg) return jumpToDate(arg);
      },
    },
    {
      id: "scratch",
      verb: "scratch",
      label: "New scratch page",
      glyph: "✎",
      category: "create",
      shortcut: "Space n s",
      keywords: ["scratch", "draft", "throwaway", "new"],
      run: () => createScratchAndJump(),
    },
    {
      id: "promote",
      verb: "promote",
      label: "Promote focused scratch to a regular page",
      glyph: "↑",
      category: "create",
      keywords: ["promote", "keep", "save", "scratch"],
      run: () => promoteFocusedScratch(),
    },
    {
      id: "delete-tag",
      verb: "delete-tag",
      label: "Delete focused tag (with usage confirmation)",
      glyph: "✕",
      category: "create",
      keywords: ["delete", "tag", "remove"],
      run: () => deleteFocusedTag(),
    },
    {
      id: "convert-to-tag",
      verb: "convert-to-tag",
      label: "Convert focused page to a tag page",
      glyph: "↻",
      category: "create",
      keywords: ["convert", "tag", "type"],
      run: () => convertFocusedTo("tag"),
    },
    {
      id: "convert-to-note",
      verb: "convert-to-note",
      label: "Convert focused tag page to a regular note",
      glyph: "↻",
      category: "create",
      keywords: ["convert", "note", "type"],
      run: () => convertFocusedTo("note"),
    },
    {
      id: "rename-slug",
      verb: "rename-slug",
      label: "Rename focused tag's slug",
      glyph: "✎",
      category: "create",
      keywords: ["rename", "slug", "tag", "disambiguate"],
      argPrompt: "new slug (e.g., cardinal-religion)",
      run: async (arg) => {
        if (!arg) {
          console.warn("rename-slug: pass the new slug as an argument");
          return;
        }
        await renameFocusedTagSlug(arg);
      },
    },
    {
      id: "prune-scratches",
      verb: "prune-scratches",
      label: "Prune stale scratch pages now",
      glyph: "🧹",
      category: "create",
      keywords: ["prune", "clean", "sweep", "scratch", "delete"],
      argPrompt: "days threshold (default: workspace setting)",
      run: async (arg) => {
        const fromArg = arg ? Number(arg) : NaN;
        const days = Number.isFinite(fromArg) && fromArg > 0
          ? fromArg
          : getScratchPruneAfterDays();
        const result = await runScratchPrune(days);
        if (!result) {
          console.warn("prune-scratches: no days threshold set");
          return;
        }
        // eslint-disable-next-line no-console
        console.info("scratch prune:", result);
        const qc = getAppQueryClient();
        if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
      },
    },
    ...SETTINGS_PAGES.map(({ slug, label }) => ({
      id: `settings-${slug}`,
      verb: `settings-${slug}`,
      label: `Settings · ${label}`,
      glyph: "⚙",
      category: "navigate" as const,
      keywords: ["settings", "preferences", "config", slug, label.toLowerCase()],
      run: () => openSettingsOverlay(slug),
    })),
    {
      id: "new-note",
      verb: "new",
      label: "New note…",
      glyph: "✎",
      category: "create",
      keywords: ["new", "create", "note", "page"],
      argPrompt: "note title",
      run: (arg) => {
        if (arg) return createNoteAndJump(arg);
      },
    },

    // ── recurrence ─────────────────────────────────────────────────────────
    {
      id: "skip-occurrence",
      verb: "skip",
      label: "Skip to Next Occurrence",
      glyph: "⏭",
      category: "tile",
      keywords: ["skip", "recurrence", "recurring", "next", "occurrence"],
      run: async () => {
        const block = getFocusedBlock();
        if (!block || !block.properties["recurring"]) {
          toast("No recurring task focused", "warn");
          return;
        }
        await skipRecurrence(block.id);
      },
    },
  ];
}

export function matchesV4Command(cmd: V4Command, query: string): boolean {
  if (!query) return true;
  const q = query.toLowerCase();
  if (cmd.label.toLowerCase().includes(q)) return true;
  if (cmd.verb && cmd.verb.toLowerCase().includes(q)) return true;
  return cmd.keywords.some((kw) => kw.includes(q));
}

export function findCommandByVerb(verb: string): V4Command | undefined {
  const v = verb.toLowerCase();
  return buildV4Commands().find((c) => c.verb === v || c.id === v);
}

<script module lang="ts">
  import { EditorView } from "@codemirror/view";
  import { Vim } from "@replit/codemirror-vim";

  // Shared context always pointing to the currently focused block editor.
  // Vim actions are registered ONCE globally (Vim.defineAction is a singleton
  // registry — per-instance calls overwrite each other, causing stale closures
  // when a block unmounts). Actions read from this ctx at call time instead.
  const vimCtx: {
    view: EditorView | null;
    navigate: ((dir: "up" | "down") => void) | null;
    deleteBlock: (() => void) | null;
    yankBlock: (() => void) | null;
    pasteBlock: (() => void) | null;
    newBlockBelow: (() => void) | null;
    newBlockAbove: (() => void) | null;
    indent: ((dir: "indent" | "outdent") => void) | null;
    leader: (() => void) | null;
  } = {
    view: null, navigate: null, deleteBlock: null, yankBlock: null,
    pasteBlock: null, newBlockBelow: null, newBlockAbove: null,
    indent: null, leader: null,
  };

  let _vimActionsRegistered = false;

  function initVimActions() {
    if (_vimActionsRegistered) return;
    _vimActionsRegistered = true;

    Vim.defineAction("moveDownOrNextBlock", () => {
      const v = vimCtx.view;
      if (!v) return;
      const s = v.state;
      const line = s.doc.lineAt(s.selection.main.head);
      if (line.number === s.doc.lines) {
        vimCtx.navigate?.("down");
      } else {
        const next = s.doc.line(line.number + 1);
        v.dispatch({ selection: { anchor: Math.min(next.from + (s.selection.main.head - line.from), next.to) } });
      }
    });
    Vim.mapCommand("j", "action", "moveDownOrNextBlock", {}, { context: "normal" });

    Vim.defineAction("moveUpOrPrevBlock", () => {
      const v = vimCtx.view;
      if (!v) return;
      const s = v.state;
      const line = s.doc.lineAt(s.selection.main.head);
      if (line.number === 1) {
        vimCtx.navigate?.("up");
      } else {
        const prev = s.doc.line(line.number - 1);
        v.dispatch({ selection: { anchor: Math.min(prev.from + (s.selection.main.head - line.from), prev.to) } });
      }
    });
    Vim.mapCommand("k", "action", "moveUpOrPrevBlock", {}, { context: "normal" });

    Vim.defineAction("openLeaderMenu", () => { vimCtx.leader?.(); });
    Vim.mapCommand("<Space>", "action", "openLeaderMenu", {}, { context: "normal" });

    Vim.defineAction("deleteBlock", () => { vimCtx.deleteBlock?.(); });
    Vim.mapCommand("dd", "action", "deleteBlock", {}, { context: "normal" });

    Vim.defineAction("yankBlock", () => { vimCtx.yankBlock?.(); });
    Vim.mapCommand("yy", "action", "yankBlock", {}, { context: "normal" });

    Vim.defineAction("pasteBlock", () => { vimCtx.pasteBlock?.(); });
    Vim.mapCommand("p", "action", "pasteBlock", {}, { context: "normal" });

    Vim.defineAction("newBlockBelow", () => { vimCtx.newBlockBelow?.(); });
    Vim.mapCommand("o", "action", "newBlockBelow", {}, { context: "normal" });

    Vim.defineAction("newBlockAbove", () => { vimCtx.newBlockAbove?.(); });
    Vim.mapCommand("O", "action", "newBlockAbove", {}, { context: "normal" });

    Vim.defineAction("indentBlock", () => { vimCtx.indent?.("indent"); });
    Vim.mapCommand(">>", "action", "indentBlock", {}, { context: "normal" });

    Vim.defineAction("outdentBlock", () => { vimCtx.indent?.("outdent"); });
    Vim.mapCommand("<<", "action", "outdentBlock", {}, { context: "normal" });
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { keymap } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { vim, getCM } from "@replit/codemirror-vim";
  import { teselaDecorations, teselaDecorationTheme } from "$lib/cm-decorations";
  import { setVimMode } from "$lib/stores/pane-state.svelte";
  import SlashMenu, { type SlashCommand } from "./SlashMenu.svelte";
  import AutocompleteMenu, { type AutocompleteItem } from "./AutocompleteMenu.svelte";

  let {
    initialText,
    onblur: onBlur,
    onfocus: onFocus,
    onchange: onChange,
    onnavigate: onNavigate,
    onescape: onEscape,
    onenter: onEnter,
    onindent: onIndent,
    onbackspaceempty: onBackspaceEmpty,
    onbackspacemerge: onBackspaceMerge,
    initialCursorPos,
    startininsert: startInInsert,
    onslashcommand: onSlashCommand,
    onleader: onLeader,
    ondeleteblock: onDeleteBlock,
    onyankblock: onYankBlock,
    onpasteblock: onPasteBlock,
    onnewblockbelow: onNewBlockBelow,
    onnewblockabove: onNewBlockAbove,
    oncyclestatus: onCycleStatus,
    focused,
    noteslist: notesList,
    statusChoices,
  }: {
    initialText: string;
    onblur: () => void;
    onfocus?: () => void;
    onchange: (text: string) => void;
    onnavigate?: (direction: "up" | "down") => void;
    onescape?: () => void;
    onenter?: (textAfterCursor: string) => void;
    onindent?: (direction: "indent" | "outdent") => void;
    onbackspaceempty?: () => void;
    onbackspacemerge?: (text: string) => void;
    initialCursorPos?: number;
    startininsert?: boolean;
    onslashcommand?: (command: string) => void;
    onleader?: () => void;
    ondeleteblock?: () => void;
    onyankblock?: () => void;
    onpasteblock?: () => void;
    onnewblockbelow?: () => void;
    onnewblockabove?: () => void;
    oncyclestatus?: () => void;
    focused?: boolean;
    noteslist?: Array<{ id: string; title: string; tags: string[] }>;
    statusChoices?: string[];
  } = $props();

  let container: HTMLDivElement;
  let view = $state<EditorView | null>(null);

  // Slash menu state
  let showSlashMenu = $state(false);
  let slashFilter = $state("");
  let slashPosition = $state({ x: 0, y: 0 });
  let slashMenuRef = $state<SlashMenu | null>(null);
  let slashStartPos = $state<number>(-1);

  // Autocomplete state (for # tags and [[ wiki-links)
  let showAutocomplete = $state(false);
  let autocompleteFilter = $state("");
  let autocompletePosition = $state({ x: 0, y: 0 });
  let autocompleteRef = $state<AutocompleteMenu | null>(null);
  let autocompleteStartPos = $state<number>(-1);
  let autocompleteType = $state<"tag" | "link">("tag");

  const autocompleteItems: AutocompleteItem[] = $derived(
    (notesList ?? []).map((n) => ({
      id: n.id,
      label: n.title,
      secondary: n.tags.length > 0 ? n.tags[0] : undefined,
    })),
  );

  function applyAutocomplete(item: AutocompleteItem) {
    if (!view || autocompleteStartPos < 0) return;
    const doc = view.state.doc.toString();
    const cursorPos = view.state.selection.main.head;
    const before = doc.slice(0, autocompleteStartPos);
    const after = doc.slice(cursorPos);

    let insert: string;
    if (autocompleteType === "tag") {
      insert = before + "#" + item.label + after;
    } else {
      insert = before + "[[" + item.label + "]]" + after;
    }

    view.dispatch({
      changes: { from: 0, to: doc.length, insert },
      selection: { anchor: insert.length - after.length },
    });
    onChange(insert);
    showAutocomplete = false;
    autocompleteFilter = "";
    autocompleteStartPos = -1;
  }

  function statusIcon(s: string): string {
    if (s === "done") return "✓";
    if (s === "doing") return "◎";
    return "☐";
  }

  function getSlashCommands(): SlashCommand[] {
    const choices = statusChoices ?? ["todo", "doing", "done"];
    const statusItems: SlashCommand[] = choices.map((s) => ({
      id: s,
      label: s.charAt(0).toUpperCase() + s.slice(1).replace(/-/g, " "),
      description: `Set status:: ${s}`,
      icon: statusIcon(s),
      action: () => applySlash(s),
    }));
    return [
      { id: "task", label: "Task", description: "Add #Task tag", icon: "☑", action: () => applySlash("task") },
      ...statusItems,
      { id: "heading", label: "Heading", description: "Convert to heading", icon: "#", action: () => applySlash("heading") },
      { id: "property", label: "Property", description: "Add key:: value", icon: "⊞", action: () => applySlash("property") },
      { id: "link", label: "Link", description: "Insert [[page link]]", icon: "⟦", action: () => applySlash("link") },
      { id: "date", label: "Date", description: "Insert today's date", icon: "📅", action: () => applySlash("date") },
    ];
  }

  function applySlash(command: string) {
    if (!view || slashStartPos < 0) return;
    const doc = view.state.doc.toString();
    const cursorPos = view.state.selection.main.head;
    const before = doc.slice(0, slashStartPos);
    const after = doc.slice(cursorPos);

    let insert = "";
    const allStatuses = statusChoices ?? ["todo", "doing", "done"];
    if (allStatuses.includes(command)) {
      insert = before.trimEnd() + "\nstatus:: " + command + after;
    } else {
      switch (command) {
        case "task":
          insert = before.trimEnd() + (before.length > 0 ? " " : "") + "#Task" + after;
          break;
        case "heading":
          insert = "# " + before.trim() + after;
          break;
        case "property":
          insert = before.trimEnd() + "\nkey:: value" + after;
          break;
        case "link":
          insert = before + "[[]]" + after;
          break;
        case "date": {
          const today = new Date().toISOString().slice(0, 10);
          insert = before + `[[${today}]]` + after;
          break;
        }
        default:
          return;
      }
    }

    view.dispatch({
      changes: { from: 0, to: doc.length, insert },
      selection: { anchor: insert.length - after.length },
    });
    onChange(insert);
    showSlashMenu = false;
    slashFilter = "";
    slashStartPos = -1;
    onSlashCommand?.(command);
  }

  // When parent changes focused prop, programmatically focus/blur CM6
  $effect(() => {
    if (focused && view && !view.hasFocus) {
      view.focus();
    }
  });

  // Keep the global vim context pointing to whichever block is currently focused.
  // This lets the module-level vim actions (registered once) always target the right block.
  $effect(() => {
    if (!focused || !view) return;
    vimCtx.view = view;
    vimCtx.navigate = onNavigate ?? null;
    vimCtx.leader = onLeader ?? null;
    vimCtx.deleteBlock = onDeleteBlock ?? null;
    vimCtx.yankBlock = onYankBlock ?? null;
    vimCtx.pasteBlock = onPasteBlock ?? null;
    vimCtx.newBlockBelow = onNewBlockBelow ?? null;
    vimCtx.newBlockAbove = onNewBlockAbove ?? null;
    vimCtx.indent = onIndent ?? null;
    return () => {
      if (vimCtx.view === view) vimCtx.view = null;
    };
  });

  onMount(() => {
    const theme = EditorView.theme({
      "&": { backgroundColor: "transparent", color: "var(--foreground)", fontSize: "14.5px", fontFamily: "'Source Sans 3', -apple-system, system-ui, sans-serif", lineHeight: "1.7" },
      ".cm-content": { caretColor: "var(--primary)", padding: "0" },
      ".cm-line": { padding: "2px 0" },
      "&.cm-focused .cm-cursor": { borderLeftColor: "var(--primary)", borderLeftWidth: "2px" },
      ".cm-fat-cursor": { background: "color-mix(in srgb, var(--primary) 25%, transparent) !important" },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 15%, transparent)" },
      ".cm-gutters": { display: "none" },
      "&.cm-focused": { outline: "none" },
    });

    const focusBlurHandler = EditorView.domEventHandlers({
      focus: () => { onFocus?.(); return false; },
      blur: () => { if (!showSlashMenu) onBlur(); return false; },
    });

    const inputHandler = EditorView.inputHandler.of((v, from, _to, inserted) => {
      // Slash commands: / at start or after whitespace
      if (inserted === "/") {
        const docBefore = v.state.doc.sliceString(0, from);
        const isAtStart = docBefore.trim() === "";
        const isAfterSpace = docBefore.endsWith(" ") || docBefore.endsWith("\n");
        if (isAtStart || isAfterSpace) {
          setTimeout(() => {
            if (!view) return;
            slashStartPos = from;
            showSlashMenu = true;
            slashFilter = "";
            const coords = view.coordsAtPos(from + 1);
            if (coords) {
              slashPosition = { x: coords.left, y: coords.bottom + 4 };
            } else {
              const rect = container.getBoundingClientRect();
              slashPosition = { x: rect.left, y: rect.bottom + 4 };
            }
          }, 0);
        }
      }

      // Tag autocomplete: # after space or at start
      if (inserted === "#") {
        const docBefore = v.state.doc.sliceString(0, from);
        const isAtStart = docBefore.trim() === "";
        const isAfterSpace = docBefore.endsWith(" ") || docBefore.endsWith("\n");
        if (isAtStart || isAfterSpace) {
          setTimeout(() => {
            if (!view) return;
            autocompleteStartPos = from;
            autocompleteType = "tag";
            showAutocomplete = true;
            autocompleteFilter = "";
            const coords = view.coordsAtPos(from + 1);
            autocompletePosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
          }, 0);
        }
      }

      // Wiki-link autocomplete: [[ (detect second [)
      if (inserted === "[") {
        const docBefore = v.state.doc.sliceString(0, from);
        if (docBefore.endsWith("[")) {
          setTimeout(() => {
            if (!view) return;
            autocompleteStartPos = from - 1; // position of first [
            autocompleteType = "link";
            showAutocomplete = true;
            autocompleteFilter = "";
            const coords = view.coordsAtPos(from + 1);
            autocompletePosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
          }, 0);
        }
      }

      return false;
    });

    const blockKeymap = keymap.of([
      {
        key: "Escape",
        run: (v) => {
          if (showSlashMenu) { showSlashMenu = false; slashFilter = ""; slashStartPos = -1; return true; }
          if (showAutocomplete) { showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1; return true; }
          // Let vim handle Escape when in insert/visual mode so it can transition to normal.
          // Only intercept when already in normal mode (to unfocus the block).
          const cm = getCM(v);
          const vimState = cm?.state?.vim as { insertMode?: boolean; visualMode?: boolean } | undefined;
          if (vimState?.insertMode || vimState?.visualMode) return false;
          onEscape?.();
          return true;
        },
      },
      {
        key: "ArrowUp",
        run: (v) => {
          if (showSlashMenu) return slashMenuRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowUp" })) ?? false;
          if (showAutocomplete) return autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowUp" })) ?? false;
          const line = v.state.doc.lineAt(v.state.selection.main.head);
          if (line.number === 1) { onNavigate?.("up"); return true; }
          return false;
        },
      },
      {
        key: "ArrowDown",
        run: (v) => {
          if (showSlashMenu) return slashMenuRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowDown" })) ?? false;
          if (showAutocomplete) return autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowDown" })) ?? false;
          const line = v.state.doc.lineAt(v.state.selection.main.head);
          if (line.number === v.state.doc.lines) { onNavigate?.("down"); return true; }
          return false;
        },
      },
      {
        key: "Enter",
        run: (v) => {
          if (showSlashMenu) {
            slashMenuRef?.handleKeydown(new KeyboardEvent("keydown", { key: "Enter" }));
            return true;
          }
          if (showAutocomplete) {
            autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "Enter" }));
            return true;
          }
          if (onEnter) {
            const cursor = v.state.selection.main.head;
            const textBefore = v.state.doc.sliceString(0, cursor);
            const textAfter = v.state.doc.sliceString(cursor);
            if (textAfter) onChange(textBefore);
            onEnter(textAfter);
            return true;
          }
          return false;
        },
      },
      { key: "Mod-Enter", run: () => { if (onCycleStatus) { onCycleStatus(); return true; } return false; } },
      { key: "Tab", run: () => { if (onIndent) { onIndent("indent"); return true; } return false; } },
      { key: "Shift-Tab", run: () => { if (onIndent) { onIndent("outdent"); return true; } return false; } },
      {
        key: "Backspace",
        run: (v) => {
          if (v.state.doc.length === 0 && onBackspaceEmpty) { onBackspaceEmpty(); return true; }
          // Merge with previous block when Backspace at cursor pos 0 with content
          const cursor = v.state.selection.main.head;
          if (cursor === 0 && v.state.selection.main.anchor === 0 && v.state.doc.length > 0 && onBackspaceMerge) {
            onBackspaceMerge(v.state.doc.toString());
            return true;
          }
          return false;
        },
      },
    ]);

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        const doc = update.state.doc.toString();
        onChange(doc);
        const cursorPos = update.state.selection.main.head;
        // Update slash filter
        if (showSlashMenu && slashStartPos >= 0) {
          if (cursorPos <= slashStartPos) {
            showSlashMenu = false; slashFilter = ""; slashStartPos = -1;
          } else {
            slashFilter = doc.slice(slashStartPos + 1, cursorPos);
          }
        }
        // Update autocomplete filter
        if (showAutocomplete && autocompleteStartPos >= 0) {
          const offset = autocompleteType === "tag" ? 1 : 2; // skip # or [[
          if (cursorPos <= autocompleteStartPos + offset) {
            showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1;
          } else {
            autocompleteFilter = doc.slice(autocompleteStartPos + offset, cursorPos);
          }
        }
      }
    });

    const clampedCursor = initialCursorPos !== undefined
      ? Math.max(0, Math.min(initialText.length, initialCursorPos))
      : undefined;
    const state = EditorState.create({
      doc: initialText,
      selection: clampedCursor !== undefined ? { anchor: clampedCursor, head: clampedCursor } : undefined,
      extensions: [
        blockKeymap,
        vim(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        history(),
        theme,
        inputHandler,
        updateListener,
        focusBlurHandler,
        teselaDecorations,
        teselaDecorationTheme,
        EditorView.lineWrapping,
      ],
    });

    view = new EditorView({ state, parent: container });

    // If an initial cursor position was requested, focus immediately
    if (clampedCursor !== undefined) {
      view.focus();
    }

    // Register global vim actions once (no-op after first call) and wire
    // mode-change tracking via the CM5-like instance event (not a DOM event).
    let vimModeOff: (() => void) | null = null;
    const cm = getCM(view);
    if (cm) {
      initVimActions();
      const modeListener = (info: { mode: string }) => { setVimMode(info.mode); };
      cm.on("vim-mode-change", modeListener);
      vimModeOff = () => cm.off("vim-mode-change", modeListener);
    }

    // Focus and optionally enter insert mode for newly created blocks
    if (focused) {
      requestAnimationFrame(() => {
        if (!view) return;
        view.focus();
        // Only move cursor to end when no explicit initialCursorPos was given;
        // for split/merge blocks the EditorState already has the right selection.
        if (clampedCursor === undefined) {
          view.dispatch({ selection: { anchor: view.state.doc.length } });
        }
        if (startInInsert) {
          const cm2 = getCM(view);
          if (cm2) Vim.handleKey(cm2, "i", "mapping");
        }
      });
    }

    return () => {
      vimModeOff?.();
      view?.destroy();
      view = null;
    };
  });
</script>

<div class="relative">
  <div bind:this={container} class="text-sm leading-relaxed min-h-[24px]"></div>

  {#if showSlashMenu}
    <SlashMenu
      bind:this={slashMenuRef}
      commands={getSlashCommands()}
      filter={slashFilter}
      position={slashPosition}
      onclose={() => { showSlashMenu = false; slashFilter = ""; slashStartPos = -1; }}
    />
  {/if}

  {#if showAutocomplete}
    <AutocompleteMenu
      bind:this={autocompleteRef}
      items={autocompleteItems}
      filter={autocompleteFilter}
      position={autocompletePosition}
      onselect={(item) => applyAutocomplete(item)}
      onclose={() => { showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1; }}
    />
  {/if}
</div>

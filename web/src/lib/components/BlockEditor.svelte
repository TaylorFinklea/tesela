<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { vim, Vim, getCM } from "@replit/codemirror-vim";
  import { teselaDecorations, teselaDecorationTheme } from "$lib/cm-decorations";
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
    startininsert: startInInsert,
    onslashcommand: onSlashCommand,
    onleader: onLeader,
    ondeleteblock: onDeleteBlock,
    onyankblock: onYankBlock,
    onpasteblock: onPasteBlock,
    onnewblockbelow: onNewBlockBelow,
    onnewblockabove: onNewBlockAbove,
    focused,
    noteslist: notesList,
  }: {
    initialText: string;
    onblur: () => void;
    onfocus?: () => void;
    onchange: (text: string) => void;
    onnavigate?: (direction: "up" | "down") => void;
    onescape?: () => void;
    onenter?: () => void;
    onindent?: (direction: "indent" | "outdent") => void;
    onbackspaceempty?: () => void;
    startininsert?: boolean;
    onslashcommand?: (command: string) => void;
    onleader?: () => void;
    ondeleteblock?: () => void;
    onyankblock?: () => void;
    onpasteblock?: () => void;
    onnewblockbelow?: () => void;
    onnewblockabove?: () => void;
    focused?: boolean;
    noteslist?: Array<{ id: string; title: string; tags: string[] }>;
  } = $props();

  let container: HTMLDivElement;
  let view: EditorView | null = null;

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

  function getSlashCommands(): SlashCommand[] {
    return [
      { id: "task", label: "Task", description: "Add #Task tag", icon: "☑", action: () => applySlash("task") },
      { id: "todo", label: "Todo", description: "Set status:: todo", icon: "☐", action: () => applySlash("todo") },
      { id: "doing", label: "Doing", description: "Set status:: doing", icon: "◎", action: () => applySlash("doing") },
      { id: "done", label: "Done", description: "Set status:: done", icon: "✓", action: () => applySlash("done") },
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
    switch (command) {
      case "task":
        insert = before.trimEnd() + (before.length > 0 ? " " : "") + "#Task" + after;
        break;
      case "todo":
        insert = before.trimEnd() + "\nstatus:: todo" + after;
        break;
      case "doing":
        insert = before.trimEnd() + "\nstatus:: doing" + after;
        break;
      case "done":
        insert = before.trimEnd() + "\nstatus:: done" + after;
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
        run: () => {
          if (showSlashMenu) { showSlashMenu = false; slashFilter = ""; slashStartPos = -1; return true; }
          if (showAutocomplete) { showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1; return true; }
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
        run: () => {
          if (showSlashMenu) {
            slashMenuRef?.handleKeydown(new KeyboardEvent("keydown", { key: "Enter" }));
            return true;
          }
          if (showAutocomplete) {
            autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "Enter" }));
            return true;
          }
          if (onEnter) { onEnter(); return true; }
          return false;
        },
      },
      { key: "Tab", run: () => { if (onIndent) { onIndent("indent"); return true; } return false; } },
      { key: "Shift-Tab", run: () => { if (onIndent) { onIndent("outdent"); return true; } return false; } },
      {
        key: "Backspace",
        run: (v) => {
          if (v.state.doc.length === 0 && onBackspaceEmpty) { onBackspaceEmpty(); return true; }
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

    const state = EditorState.create({
      doc: initialText,
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

    // Register Vim normal-mode commands
    const cm = getCM(view);
    if (cm) {
      // j/k — cross-block navigation at boundaries
      if (onNavigate) {
        Vim.defineAction("moveDownOrNextBlock", () => {
          const v = view;
          if (!v) return;
          const state = v.state;
          const line = state.doc.lineAt(state.selection.main.head);
          if (line.number === state.doc.lines) {
            onNavigate("down");
          } else {
            // Move cursor down one line
            const nextLine = state.doc.line(line.number + 1);
            const col = state.selection.main.head - line.from;
            v.dispatch({ selection: { anchor: Math.min(nextLine.from + col, nextLine.to) } });
          }
        });
        Vim.mapCommand("j", "action", "moveDownOrNextBlock", {}, { context: "normal" });

        Vim.defineAction("moveUpOrPrevBlock", () => {
          const v = view;
          if (!v) return;
          const state = v.state;
          const line = state.doc.lineAt(state.selection.main.head);
          if (line.number === 1) {
            onNavigate("up");
          } else {
            // Move cursor up one line
            const prevLine = state.doc.line(line.number - 1);
            const col = state.selection.main.head - line.from;
            v.dispatch({ selection: { anchor: Math.min(prevLine.from + col, prevLine.to) } });
          }
        });
        Vim.mapCommand("k", "action", "moveUpOrPrevBlock", {}, { context: "normal" });
      }

      // Space → leader menu
      if (onLeader) {
        Vim.defineAction("openLeaderMenu", () => { onLeader?.(); });
        Vim.mapCommand("<Space>", "action", "openLeaderMenu", {}, { context: "normal" });
      }
      // dd → delete block
      if (onDeleteBlock) {
        Vim.defineAction("deleteBlock", () => { onDeleteBlock?.(); });
        Vim.mapCommand("dd", "action", "deleteBlock", {}, { context: "normal" });
      }
      // yy → yank block
      if (onYankBlock) {
        Vim.defineAction("yankBlock", () => { onYankBlock?.(); });
        Vim.mapCommand("yy", "action", "yankBlock", {}, { context: "normal" });
      }
      // p → paste block below
      if (onPasteBlock) {
        Vim.defineAction("pasteBlock", () => { onPasteBlock?.(); });
        Vim.mapCommand("p", "action", "pasteBlock", {}, { context: "normal" });
      }
      // o → new block below, enter insert
      if (onNewBlockBelow) {
        Vim.defineAction("newBlockBelow", () => { onNewBlockBelow?.(); });
        Vim.mapCommand("o", "action", "newBlockBelow", {}, { context: "normal" });
      }
      // O → new block above, enter insert
      if (onNewBlockAbove) {
        Vim.defineAction("newBlockAbove", () => { onNewBlockAbove?.(); });
        Vim.mapCommand("O", "action", "newBlockAbove", {}, { context: "normal" });
      }
      // >> → indent block
      if (onIndent) {
        Vim.defineAction("indentBlock", () => { onIndent?.("indent"); });
        Vim.mapCommand(">>", "action", "indentBlock", {}, { context: "normal" });
        Vim.defineAction("outdentBlock", () => { onIndent?.("outdent"); });
        Vim.mapCommand("<<", "action", "outdentBlock", {}, { context: "normal" });
      }
    }

    // If this block should start focused and in insert mode
    if (focused) {
      requestAnimationFrame(() => {
        if (!view) return;
        view.focus();
        view.dispatch({ selection: { anchor: view.state.doc.length } });
        if (startInInsert) {
          const cm2 = getCM(view);
          if (cm2) Vim.handleKey(cm2, "i", "mapping");
        }
      });
    }

    return () => {
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

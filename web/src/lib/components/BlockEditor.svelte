<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { vim, Vim, getCM } from "@replit/codemirror-vim";
  import SlashMenu, { type SlashCommand } from "./SlashMenu.svelte";

  let {
    initialText,
    onblur: onBlur,
    onchange: onChange,
    onnavigate: onNavigate,
    onescape: onEscape,
    onenter: onEnter,
    onindent: onIndent,
    onbackspaceempty: onBackspaceEmpty,
    startininsert: startInInsert,
    onslashcommand: onSlashCommand,
    onleader: onLeader,
  }: {
    initialText: string;
    onblur: () => void;
    onchange: (text: string) => void;
    onnavigate?: (direction: "up" | "down") => void;
    onescape?: () => void;
    onenter?: () => void;
    onindent?: (direction: "indent" | "outdent") => void;
    onbackspaceempty?: () => void;
    startininsert?: boolean;
    onslashcommand?: (command: string) => void;
    onleader?: () => void;
  } = $props();

  let container: HTMLDivElement;
  let view: EditorView | null = null;

  // Slash menu state
  let showSlashMenu = $state(false);
  let slashFilter = $state("");
  let slashPosition = $state({ x: 0, y: 0 });
  let slashMenuRef = $state<SlashMenu | null>(null);
  let slashStartPos = $state<number>(-1); // cursor position where / was typed

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
    // Remove everything from slash start to cursor (the /command text)
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

  onMount(() => {
    let blurArmed = false;

    const theme = EditorView.theme(
      {
        "&": { backgroundColor: "transparent", color: "var(--foreground)", fontSize: "14px", fontFamily: "inherit" },
        ".cm-content": { caretColor: "var(--foreground)", padding: "0" },
        ".cm-line": { padding: "0" },
        "&.cm-focused .cm-cursor": { borderLeftColor: "var(--foreground)" },
        "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": { backgroundColor: "hsl(0 0% 30%)" },
        ".cm-gutters": { display: "none" },
        "&.cm-focused": { outline: "none" },
      },
      { dark: true },
    );

    const blurHandler = EditorView.domEventHandlers({
      blur: () => { if (blurArmed && !showSlashMenu) onBlur(); return false; },
    });

    // Detect / typed in Insert mode via input handler
    const slashInputHandler = EditorView.inputHandler.of((v, from, _to, inserted) => {
      if (inserted === "/") {
        // Check if this / is at the start of text or after whitespace
        const docBefore = v.state.doc.sliceString(0, from);
        const isAtStart = docBefore.trim() === "";
        const isAfterSpace = docBefore.endsWith(" ") || docBefore.endsWith("\n");
        if (isAtStart || isAfterSpace) {
          // Schedule slash menu to show after the / is inserted
          setTimeout(() => {
            if (!view) return;
            slashStartPos = from;
            showSlashMenu = true;
            slashFilter = "";
            // Position at cursor
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
      return false; // Don't prevent the input
    });

    const blockKeymap = keymap.of([
      {
        key: "Escape",
        run: () => {
          if (showSlashMenu) { showSlashMenu = false; slashFilter = ""; slashStartPos = -1; return true; }
          onEscape?.();
          return true;
        },
      },
      {
        key: "ArrowUp",
        run: (v) => {
          if (showSlashMenu) return slashMenuRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowUp" })) ?? false;
          const line = v.state.doc.lineAt(v.state.selection.main.head);
          if (line.number === 1) { onNavigate?.("up"); return true; }
          return false;
        },
      },
      {
        key: "ArrowDown",
        run: (v) => {
          if (showSlashMenu) return slashMenuRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowDown" })) ?? false;
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
        // Update slash filter if menu is open
        if (showSlashMenu && slashStartPos >= 0) {
          const cursorPos = update.state.selection.main.head;
          if (cursorPos <= slashStartPos) {
            // Cursor moved before slash — close menu
            showSlashMenu = false;
            slashFilter = "";
            slashStartPos = -1;
          } else {
            slashFilter = doc.slice(slashStartPos + 1, cursorPos);
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
        slashInputHandler,
        updateListener,
        blurHandler,
        EditorView.lineWrapping,
      ],
    });

    view = new EditorView({ state, parent: container });

    // Register Vim normal-mode Space → leader menu
    const cm = getCM(view);
    if (cm && onLeader) {
      Vim.defineAction("openLeaderMenu", () => {
        onLeader?.();
      });
      Vim.mapCommand("<Space>", "action", "openLeaderMenu", {}, { context: "normal" });
    }

    requestAnimationFrame(() => {
      if (!view) return;
      view.focus();
      view.dispatch({ selection: { anchor: view.state.doc.length } });
      if (startInInsert) {
        const cm2 = getCM(view);
        if (cm2) Vim.handleKey(cm2, "i", "mapping");
      }
      setTimeout(() => { blurArmed = true; }, 100);
    });

    return () => {
      view?.destroy();
      view = null;
    };
  });
</script>

<div class="relative">
  <div bind:this={container} class="text-sm leading-relaxed min-h-[24px] ring-1 ring-ring/20 rounded-sm px-1 -mx-1"></div>

  {#if showSlashMenu}
    <SlashMenu
      bind:this={slashMenuRef}
      commands={getSlashCommands()}
      filter={slashFilter}
      position={slashPosition}
      onclose={() => { showSlashMenu = false; slashFilter = ""; slashStartPos = -1; }}
    />
  {/if}
</div>

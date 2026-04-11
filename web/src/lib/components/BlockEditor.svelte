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
  } = $props();

  let container: HTMLDivElement;
  let view: EditorView | null = null;

  // Slash menu state
  let showSlashMenu = $state(false);
  let slashFilter = $state("");
  let slashPosition = $state({ x: 0, y: 0 });
  let slashMenuRef = $state<SlashMenu | null>(null);

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
    if (!view) return;
    const doc = view.state.doc.toString();
    // Remove the /command text from the doc
    const slashStart = doc.indexOf("/");
    if (slashStart === -1) return;

    let replacement = "";
    switch (command) {
      case "task":
        replacement = doc.slice(0, slashStart).trimEnd() + (slashStart > 0 ? " " : "") + "#Task";
        break;
      case "todo":
        replacement = doc.slice(0, slashStart).trimEnd() + "\nstatus:: todo";
        break;
      case "doing":
        replacement = doc.slice(0, slashStart).trimEnd() + "\nstatus:: doing";
        break;
      case "done":
        replacement = doc.slice(0, slashStart).trimEnd() + "\nstatus:: done";
        break;
      case "heading":
        replacement = "# " + doc.slice(0, slashStart).trim();
        break;
      case "property":
        replacement = doc.slice(0, slashStart).trimEnd() + "\nkey:: value";
        break;
      case "link":
        replacement = doc.slice(0, slashStart) + "[[]]";
        break;
      case "date": {
        const today = new Date().toISOString().slice(0, 10);
        replacement = doc.slice(0, slashStart) + `[[${today}]]`;
        break;
      }
    }

    view.dispatch({
      changes: { from: 0, to: doc.length, insert: replacement },
      selection: { anchor: replacement.length },
    });
    onChange(replacement);
    showSlashMenu = false;
    slashFilter = "";
    onSlashCommand?.(command);
  }

  function checkForSlash(doc: string) {
    // Check if text starts with / or has / after whitespace
    const match = doc.match(/^\//);
    if (match && view) {
      showSlashMenu = true;
      slashFilter = doc.slice(1); // everything after /
      // Position the menu below the editor
      const rect = container.getBoundingClientRect();
      slashPosition = { x: rect.left, y: rect.bottom + 4 };
    } else {
      showSlashMenu = false;
      slashFilter = "";
    }
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

    const blockKeymap = keymap.of([
      {
        key: "Escape",
        run: () => {
          if (showSlashMenu) { showSlashMenu = false; slashFilter = ""; return true; }
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
        checkForSlash(doc);
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
        updateListener,
        blurHandler,
        EditorView.lineWrapping,
      ],
    });

    view = new EditorView({ state, parent: container });

    requestAnimationFrame(() => {
      if (!view) return;
      view.focus();
      view.dispatch({ selection: { anchor: view.state.doc.length } });
      if (startInInsert) {
        const cm = getCM(view);
        if (cm) Vim.handleKey(cm, "i", "mapping");
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
      onclose={() => { showSlashMenu = false; slashFilter = ""; }}
    />
  {/if}
</div>

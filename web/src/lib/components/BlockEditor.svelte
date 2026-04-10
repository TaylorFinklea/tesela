<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { vim, Vim, getCM } from "@replit/codemirror-vim";

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
  } = $props();

  let container: HTMLDivElement;
  let view: EditorView | null = null;

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
      blur: () => { if (blurArmed) onBlur(); return false; },
    });

    const blockKeymap = keymap.of([
      { key: "Escape", run: () => { onEscape?.(); return true; } },
      {
        key: "ArrowUp",
        run: (v) => {
          const line = v.state.doc.lineAt(v.state.selection.main.head);
          if (line.number === 1) { onNavigate?.("up"); return true; }
          return false;
        },
      },
      {
        key: "ArrowDown",
        run: (v) => {
          const line = v.state.doc.lineAt(v.state.selection.main.head);
          if (line.number === v.state.doc.lines) { onNavigate?.("down"); return true; }
          return false;
        },
      },
      { key: "Enter", run: () => { if (onEnter) { onEnter(); return true; } return false; } },
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
      if (update.docChanged) onChange(update.state.doc.toString());
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
      // Enter insert mode if requested (e.g., after Enter creates a new block)
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

<div bind:this={container} class="text-sm leading-relaxed min-h-[24px] ring-1 ring-ring/20 rounded-sm px-1 -mx-1"></div>

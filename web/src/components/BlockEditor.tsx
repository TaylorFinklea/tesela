import { useRef, useEffect, useCallback } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { vim } from "@replit/codemirror-vim";

/**
 * Inline CM6 editor for a single block's raw text.
 *
 * Supports:
 * - ArrowUp at first line → navigate to previous block
 * - ArrowDown at last line → navigate to next block
 * - Escape → exit editing mode
 * - Enter → create new block (when onEnter provided)
 * - Tab / Shift-Tab → indent/outdent (when provided)
 */
export function BlockEditor({
  initialText,
  onBlur,
  onChange,
  onNavigate,
  onEscape,
  onEnter,
  onIndent,
  onBackspaceEmpty,
}: {
  initialText: string;
  onBlur: () => void;
  onChange: (text: string) => void;
  onNavigate?: (direction: "up" | "down") => void;
  onEscape?: () => void;
  onEnter?: () => void;
  onIndent?: (direction: "indent" | "outdent") => void;
  onBackspaceEmpty?: () => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onBlurRef = useRef(onBlur);
  const onNavigateRef = useRef(onNavigate);
  const onEscapeRef = useRef(onEscape);
  const onEnterRef = useRef(onEnter);
  const onIndentRef = useRef(onIndent);
  const onBackspaceEmptyRef = useRef(onBackspaceEmpty);

  useEffect(() => { onChangeRef.current = onChange; }, [onChange]);
  useEffect(() => { onBlurRef.current = onBlur; }, [onBlur]);
  useEffect(() => { onNavigateRef.current = onNavigate; }, [onNavigate]);
  useEffect(() => { onEscapeRef.current = onEscape; }, [onEscape]);
  useEffect(() => { onEnterRef.current = onEnter; }, [onEnter]);
  useEffect(() => { onIndentRef.current = onIndent; }, [onIndent]);
  useEffect(() => { onBackspaceEmptyRef.current = onBackspaceEmpty; }, [onBackspaceEmpty]);

  const createView = useCallback(() => {
    if (!containerRef.current) return;
    viewRef.current?.destroy();

    const theme = EditorView.theme(
      {
        "&": {
          backgroundColor: "transparent",
          color: "var(--foreground)",
          fontSize: "14px",
          fontFamily: "inherit",
        },
        ".cm-content": {
          caretColor: "var(--foreground)",
          padding: "0",
        },
        ".cm-line": {
          padding: "0",
        },
        "&.cm-focused .cm-cursor": {
          borderLeftColor: "var(--foreground)",
        },
        "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
          backgroundColor: "hsl(0 0% 30%)",
        },
        ".cm-gutters": {
          display: "none",
        },
        "&.cm-focused": {
          outline: "none",
        },
      },
      { dark: true },
    );

    let blurArmed = false;

    const blurHandler = EditorView.domEventHandlers({
      blur: () => {
        if (blurArmed) {
          onBlurRef.current();
        }
        return false;
      },
    });

    // Custom keybindings for block navigation
    const blockKeymap = keymap.of([
      {
        key: "Escape",
        run: () => {
          onEscapeRef.current?.();
          return true;
        },
      },
      {
        key: "ArrowUp",
        run: (view) => {
          // If cursor is on the first line, navigate up
          const line = view.state.doc.lineAt(view.state.selection.main.head);
          if (line.number === 1) {
            onNavigateRef.current?.("up");
            return true;
          }
          return false; // Let CM6 handle normal cursor movement
        },
      },
      {
        key: "ArrowDown",
        run: (view) => {
          // If cursor is on the last line, navigate down
          const line = view.state.doc.lineAt(view.state.selection.main.head);
          if (line.number === view.state.doc.lines) {
            onNavigateRef.current?.("down");
            return true;
          }
          return false;
        },
      },
      {
        key: "Enter",
        run: () => {
          if (onEnterRef.current) {
            onEnterRef.current();
            return true;
          }
          return false; // Allow normal Enter if no handler
        },
      },
      {
        key: "Tab",
        run: () => {
          if (onIndentRef.current) {
            onIndentRef.current("indent");
            return true;
          }
          return false;
        },
      },
      {
        key: "Shift-Tab",
        run: () => {
          if (onIndentRef.current) {
            onIndentRef.current("outdent");
            return true;
          }
          return false;
        },
      },
      {
        key: "Backspace",
        run: (view) => {
          // Only handle if doc is empty and cursor is at position 0
          if (view.state.doc.length === 0 && onBackspaceEmptyRef.current) {
            onBackspaceEmptyRef.current();
            return true;
          }
          return false; // Let CM6 handle normal backspace
        },
      },
    ]);

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChangeRef.current(update.state.doc.toString());
      }
    });

    const state = EditorState.create({
      doc: initialText,
      extensions: [
        blockKeymap, // Must come before vim/defaultKeymap so Escape/Enter/Tab take priority
        vim(),       // Vim mode: hjkl, motions, operators, visual, dot-repeat, /search
        keymap.of([...defaultKeymap, ...historyKeymap]),
        history(),
        theme,
        updateListener,
        blurHandler,
        EditorView.lineWrapping,
      ],
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    requestAnimationFrame(() => {
      view.focus();
      view.dispatch({
        selection: { anchor: view.state.doc.length },
      });
      setTimeout(() => {
        blurArmed = true;
      }, 100);
    });

    viewRef.current = view;
  }, [initialText]);

  useEffect(() => {
    createView();
    return () => {
      viewRef.current?.destroy();
      viewRef.current = null;
    };
  }, [createView]);

  return (
    <div
      ref={containerRef}
      className="text-sm leading-relaxed min-h-[24px] ring-1 ring-ring/20 rounded-sm px-1 -mx-1"
    />
  );
}

import { useRef, useEffect, useCallback } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

/**
 * Inline CM6 editor for a single block's raw text.
 * Mounts when a block is focused, unmounts on blur.
 */
export function BlockEditor({
  initialText,
  onBlur,
  onChange,
}: {
  initialText: string;
  onBlur: () => void;
  onChange: (text: string) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onBlurRef = useRef(onBlur);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    onBlurRef.current = onBlur;
  }, [onBlur]);

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

    // Track whether the editor is "armed" for blur — prevents the mount
    // click from immediately triggering blur before the user sees the editor.
    let blurArmed = false;

    const blurHandler = EditorView.domEventHandlers({
      blur: () => {
        if (blurArmed) {
          onBlurRef.current();
        }
        return false;
      },
    });

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChangeRef.current(update.state.doc.toString());
      }
    });

    const state = EditorState.create({
      doc: initialText,
      extensions: [
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

    // Delay focus to next frame so the mounting click doesn't cause an
    // immediate blur cycle.
    requestAnimationFrame(() => {
      view.focus();
      view.dispatch({
        selection: { anchor: view.state.doc.length },
      });
      // Arm blur handler after focus is stable
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

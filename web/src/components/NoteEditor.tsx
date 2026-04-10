import { useRef, useEffect, useCallback } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";
import { searchKeymap } from "@codemirror/search";

/**
 * A CodeMirror 6 editor for a note's full content (frontmatter + body).
 *
 * MVP approach: single editor instance per note. Decorations and per-block
 * splitting come in later milestones.
 */
export function NoteEditor({
  initialContent,
  onContentChange,
  className,
}: {
  initialContent: string;
  onContentChange?: (content: string) => void;
  className?: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onContentChange);
  useEffect(() => {
    onChangeRef.current = onContentChange;
  }, [onContentChange]);

  const createView = useCallback(() => {
    if (!containerRef.current) return;

    // Clean up previous view
    viewRef.current?.destroy();

    const darkTheme = EditorView.theme(
      {
        "&": {
          backgroundColor: "transparent",
          color: "var(--foreground)",
          fontSize: "14px",
          fontFamily: "var(--font-mono), monospace",
        },
        ".cm-content": {
          caretColor: "var(--foreground)",
          padding: "0",
        },
        ".cm-line": {
          padding: "2px 0",
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
        ".cm-scroller": {
          overflow: "auto",
        },
      },
      { dark: true },
    );

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChangeRef.current?.(update.state.doc.toString());
      }
    });

    const state = EditorState.create({
      doc: initialContent,
      extensions: [
        keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
        history(),
        markdown(),
        syntaxHighlighting(defaultHighlightStyle),
        darkTheme,
        updateListener,
        EditorView.lineWrapping,
      ],
    });

    viewRef.current = new EditorView({
      state,
      parent: containerRef.current,
    });
  }, [initialContent]);

  useEffect(() => {
    createView();
    return () => {
      viewRef.current?.destroy();
      viewRef.current = null;
    };
  }, [createView]);

  return <div ref={containerRef} className={className} />;
}

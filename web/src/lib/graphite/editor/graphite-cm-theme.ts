/**
 * Graphite CodeMirror theme (Part A — Task A1).
 *
 * The reused editor stack (`BlockOutliner` → `BlockEditor` → `cm-decorations`)
 * is NOT modified. Its CodeMirror theme + Tailwind utility classes resolve
 * through semantic CSS variables (`--foreground`, `--primary`, `--background`,
 * `--theme-font-sans`, …). Under `.gr-root` those variables are remapped to
 * Graphite tokens (see `graphite-editor.css`), so the editor inherits the
 * Graphite palette without any change to the editor code.
 *
 * This module exports an OPTIONAL `EditorView.theme` extension for callers
 * that can inject CodeMirror extensions. BlockEditor builds its own theme
 * internally and exposes no `extensions` prop, so it is NOT consumed there —
 * the variable remap in `graphite-editor.css` does the styling. The extension
 * is kept here so a future surface that can inject extensions has a canonical
 * Graphite editor theme to reach for, and so the A1 intent (an editor theme
 * mapped to tokens) is expressed in code, not only CSS.
 */
import { EditorView } from "@codemirror/view";

/** Graphite editor theme, scoped to the tokens supplied under `.gr-root`. */
export const graphiteCmTheme = EditorView.theme({
  "&": {
    backgroundColor: "transparent",
    color: "var(--fg)",
    fontSize: "14.5px",
    fontFamily: "var(--sans)",
    lineHeight: "1.6",
  },
  ".cm-content": {
    caretColor: "var(--coral)",
    padding: "0",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--coral)",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--raised-2)",
  },
  ".cm-line": {
    padding: "2px 0",
  },
});

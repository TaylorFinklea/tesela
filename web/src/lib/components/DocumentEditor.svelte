<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { EditorState } from "@codemirror/state";
  import { keymap } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { vim } from "@replit/codemirror-vim";
  import { teselaDecorations, teselaDecorationTheme } from "$lib/cm-decorations";

  let {
    body,
    frontmatter,
    onContentChange,
  }: {
    body: string;
    frontmatter: string;
    onContentChange?: (fullContent: string) => void;
  } = $props();

  let container: HTMLDivElement;
  let view = $state<EditorView | null>(null);

  onMount(() => {
    const theme = EditorView.theme({
      "&": { backgroundColor: "transparent", color: "var(--foreground)", fontSize: "14.5px", fontFamily: "'Source Sans 3', -apple-system, system-ui, sans-serif", lineHeight: "1.8" },
      ".cm-content": { caretColor: "var(--primary)", padding: "0", maxWidth: "100%" },
      ".cm-line": { padding: "2px 0" },
      ".cm-cursor": { borderLeftColor: "var(--primary)", borderLeftWidth: "2px" },
      ".cm-fat-cursor": { background: "color-mix(in srgb, var(--primary) 25%, transparent) !important" },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 15%, transparent)" },
      ".cm-gutters": { display: "none" },
      "&.cm-focused": { outline: "none" },
    });

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onContentChange?.(`${frontmatter}${update.state.doc.toString()}`);
      }
    });

    const state = EditorState.create({
      doc: body,
      extensions: [
        vim(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        history(),
        theme,
        updateListener,
        teselaDecorations,
        teselaDecorationTheme,
        EditorView.lineWrapping,
      ],
    });

    view = new EditorView({ state, parent: container });
    view.focus();

    return () => {
      view?.destroy();
      view = null;
    };
  });
</script>

<div bind:this={container} class="text-sm leading-relaxed min-h-[200px]"></div>

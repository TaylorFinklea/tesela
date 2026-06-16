import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.query",
  verb: "query",
  label: "Query",
  glyph: "query::",
  category: "editor",
  surface: "editor",
  slashKey: "q",
  keywords: ["query", "filter", "search"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    const head = ed.before.trimEnd() + "\nquery:: type = ";
    const tail = "\nview:: table" + ed.after;
    ed.replaceTrigger(head + tail, tail.length);
    ed.finish("query");
  },
});

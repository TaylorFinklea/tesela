import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.query",
  verb: "query",
  label: "Query",
  glyph: "query::",
  category: "editor",
  surface: "editor",
  slashKey: "q",
  chord: ["i", "q"],
  keywords: ["query", "filter", "search"],
  description:
    "Insert a live query:: block — filters/sorts blocks or pages by JQL. " +
    'Examples: "status = todo AND priority > 2", ' +
    '"type = project AND scheduled IS NULL", ' +
    '"tag = urgent ORDER BY deadline ASC".',
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    const head = ed.before.trimEnd() + "\nquery:: type = ";
    const tail = "\nview:: table" + ed.after;
    ed.replaceTrigger(head + tail, tail.length);
    ed.finish("query");
  },
});

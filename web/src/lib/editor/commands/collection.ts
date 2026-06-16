import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.collection",
  verb: "collection",
  label: "Collection",
  glyph: "collection::",
  category: "editor",
  surface: "editor",
  slashKey: "c",
  chord: ["i", "c"],
  keywords: ["collection", "list", "cards"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.replaceTrigger(ed.before.trimEnd() + "\ncollection:: []\nview:: cards" + ed.after, ed.after.length);
    ed.finish("collection");
  },
});

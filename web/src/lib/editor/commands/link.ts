import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.link",
  verb: "link",
  label: "Link",
  glyph: "[[ ]]",
  category: "editor",
  surface: "editor",
  slashKey: "l",
  chord: ["i", "l"],
  keywords: ["link", "wikilink", "reference"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.replaceTrigger(ed.before + "[[]]" + ed.after, ed.after.length);
    ed.finish("link");
  },
});

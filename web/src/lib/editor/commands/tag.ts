import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.tag",
  verb: "tag",
  label: "Tag picker",
  glyph: "#",
  category: "editor",
  surface: "editor",
  slashKey: "T",
  chord: ["i", "g"],
  keywords: ["tag", "label", "category"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.openTagPicker();
    ed.finish("tag");
  },
});

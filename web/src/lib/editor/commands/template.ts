import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.template",
  verb: "template",
  label: "Template",
  glyph: "📄",
  category: "editor",
  surface: "editor",
  slashKey: "m",
  keywords: ["template", "snippet", "boilerplate"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.openTemplatePicker();
    ed.finish("template");
  },
});

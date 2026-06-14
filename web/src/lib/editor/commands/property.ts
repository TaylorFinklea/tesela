import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.property",
  verb: "property",
  label: "Manual key:: value",
  glyph: "key:: value",
  category: "editor",
  surface: "editor",
  // No slashKey — invoked from the /p submenu leaf, not as a top-level slash verb.
  keywords: ["property", "key", "value", "attribute"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.replaceTrigger(ed.before.trimEnd() + "\nkey:: value" + ed.after, ed.after.length);
    ed.finish("property");
  },
});

import { commandRegistry } from "../../command-registry.svelte.ts";

export function headingText(before: string, after: string): string {
  return "# " + before.trim() + after;
}

commandRegistry.register({
  id: "editor.heading",
  verb: "heading",
  label: "Heading",
  glyph: "#",
  category: "editor",
  surface: "global",
  slashKey: "h",
  chord: ["i", "h"],
  keywords: ["heading", "title", "block"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.replaceTrigger(headingText(ed.before, ed.after), ed.after.length);
    ed.finish("heading");
  },
});

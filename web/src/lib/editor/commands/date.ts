import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.date",
  verb: "date",
  label: "Date",
  glyph: "📅",
  category: "editor",
  surface: "editor",
  slashKey: "d",
  keywords: ["date", "deadline", "scheduled", "calendar"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.openDatePicker();
    ed.finish("date");
  },
});

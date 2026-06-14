import { commandRegistry } from "../../command-registry.svelte.ts";

commandRegistry.register({
  id: "editor.task",
  verb: "task",
  label: "Task",
  glyph: "tags:: Task",
  category: "editor",
  surface: "global",
  slashKey: "t",
  keywords: ["task", "todo", "tag"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.addTag("Task");
    ed.finish("task");
  },
});

import { commandRegistry } from "../../command-registry.svelte.ts";
import { api } from "$lib/api-client";
import { goto } from "$app/navigation";

commandRegistry.register({
  id: "editor.widget",
  verb: "widget",
  label: "New widget",
  glyph: "widget",
  category: "editor",
  surface: "editor",
  slashKey: "w",
  chord: ["n", "w"],
  keywords: ["widget", "query", "saved", "collection"],
  run: (_arg, ctx) => {
    const ed = ctx?.editor;
    if (!ed) return;
    ed.replaceTrigger(ed.before + ed.after, ed.after.length);
    ed.finish("widget");
    setTimeout(async () => {
      const name = window.prompt("New query widget name:");
      if (!name || !name.trim()) return;
      const trimmed = name.trim();
      const content = `---\ntitle: "${trimmed}"\ntype: "Query"\ntags: []\n---\nquery::\nsection:: saved\n`;
      try {
        const created = await api.createNote(trimmed, content);
        goto(`/p/${encodeURIComponent(created.id)}`);
      } catch (e) {
        console.error("Failed to create widget:", e);
      }
    }, 0);
  },
});

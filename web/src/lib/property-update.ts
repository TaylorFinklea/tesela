/**
 * Shared utility for updating block properties in note content.
 * Used by TagTable and KanbanBoard.
 */
import { api } from "$lib/api-client";
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryClient } from "@tanstack/svelte-query";

/**
 * Find a block in note content by matching its first line,
 * then update or insert a property line.
 */
export async function updateBlockProperty(params: {
  block: ParsedBlock;
  propKey: string;
  value: string;
  tagName: string;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, value, tagName, queryClient } = params;
  const key = propKey.toLowerCase();

  const note = await api.getNote(block.note_id);
  const lines = note.content.split("\n");
  let updated = false;

  const blockText = block.raw_text.split("\n")[0] ?? "";
  let inBlock = false;

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();

    if (trimmed.startsWith("- ") && trimmed.slice(2).startsWith(blockText.split("\n")[0])) {
      inBlock = true;
      continue;
    }

    if (inBlock) {
      if (trimmed.startsWith("- ") || (trimmed === "" && i > 0)) {
        const blockIndent = lines[i - 1] ? lines[i - 1].length - lines[i - 1].trimStart().length : 2;
        lines.splice(i, 0, " ".repeat(blockIndent) + `${key}:: ${value}`);
        updated = true;
        break;
      }
      const propMatch = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/);
      if (propMatch && propMatch[1].toLowerCase() === key) {
        const indent = lines[i].length - lines[i].trimStart().length;
        lines[i] = " ".repeat(indent) + `${propMatch[1]}:: ${value}`;
        updated = true;
        break;
      }
    }
  }

  if (!updated && inBlock) {
    lines.push(`  ${key}:: ${value}`);
  }

  await api.updateNote(block.note_id, lines.join("\n"));
  queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
  queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
}

/**
 * Remove a property line from a block (for clearing/unsetting a value).
 */
export async function clearBlockProperty(params: {
  block: ParsedBlock;
  propKey: string;
  tagName: string;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, tagName, queryClient } = params;
  const key = propKey.toLowerCase();

  const note = await api.getNote(block.note_id);
  const lines = note.content.split("\n");

  const blockText = block.raw_text.split("\n")[0] ?? "";
  let inBlock = false;

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();

    if (trimmed.startsWith("- ") && trimmed.slice(2).startsWith(blockText.split("\n")[0])) {
      inBlock = true;
      continue;
    }

    if (inBlock) {
      if (trimmed.startsWith("- ") || (trimmed === "" && i > 0)) {
        break; // property not found in this block
      }
      const propMatch = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/);
      if (propMatch && propMatch[1].toLowerCase() === key) {
        lines.splice(i, 1);
        break;
      }
    }
  }

  await api.updateNote(block.note_id, lines.join("\n"));
  queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
  queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
}

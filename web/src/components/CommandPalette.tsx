import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";

import { api } from "@/lib/api-client";
import type { Note } from "@/lib/types/Note";

/**
 * ⌘K Command Palette — Alfred/Raycast-style universal launcher.
 *
 * Features:
 * - Search all notes by title (fuzzy via cmdk built-in)
 * - Quick actions: New note, Go to daily, etc.
 * - Keyboard-only: arrow keys to navigate, Enter to select, Escape to close
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const router = useRouter();

  // Toggle on ⌘K / Ctrl+K
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  const notesQuery = useQuery<Note[], Error>({
    queryKey: ["notes", { limit: 200 }],
    queryFn: () => api.listNotes({ limit: 200 }),
    enabled: open,
  });

  const navigateTo = (path: string) => {
    setOpen(false);
    router.push(path);
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50">
      <div
        className="absolute inset-0 bg-black/50"
        onClick={() => setOpen(false)}
      />
      <div className="absolute left-1/2 top-[20%] -translate-x-1/2 w-full max-w-lg">
        <Command
          className="rounded-lg border border-border bg-popover text-popover-foreground shadow-2xl"
          label="Command palette"
        >
          <Command.Input
            placeholder="Search notes or type a command…"
            className="w-full border-b border-border bg-transparent px-4 py-3 text-sm outline-none placeholder:text-muted-foreground"
          />
          <Command.List className="max-h-80 overflow-y-auto p-2">
            <Command.Empty className="px-4 py-6 text-center text-sm text-muted-foreground">
              No results.
            </Command.Empty>

            <Command.Group heading="Actions" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground">
              <CommandItem onSelect={() => navigateTo("/")}>
                <span className="text-muted-foreground mr-2">⌂</span>
                Go to notes list
              </CommandItem>
              <CommandItem onSelect={() => navigateTo("/p/daily")}>
                <span className="text-muted-foreground mr-2">☀</span>
                Go to daily note
              </CommandItem>
            </Command.Group>

            {notesQuery.data && notesQuery.data.length > 0 && (
              <Command.Group heading="Notes" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground">
                {notesQuery.data.map((note) => (
                  <CommandItem
                    key={note.id}
                    value={note.title}
                    onSelect={() =>
                      navigateTo(`/p/${encodeURIComponent(note.id)}`)
                    }
                  >
                    <span className="truncate">{note.title}</span>
                    {note.metadata.tags.length > 0 && (
                      <span className="ml-auto text-xs text-muted-foreground shrink-0">
                        {note.metadata.tags.join(", ")}
                      </span>
                    )}
                  </CommandItem>
                ))}
              </Command.Group>
            )}
          </Command.List>
        </Command>
      </div>
    </div>
  );
}

function CommandItem({
  children,
  value,
  onSelect,
}: {
  children: React.ReactNode;
  value?: string;
  onSelect: () => void;
}) {
  return (
    <Command.Item
      value={value}
      onSelect={onSelect}
      className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer aria-selected:bg-accent aria-selected:text-accent-foreground"
    >
      {children}
    </Command.Item>
  );
}

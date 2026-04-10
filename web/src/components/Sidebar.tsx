import { useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useQuery } from "@tanstack/react-query";

import { api } from "@/lib/api-client";
import type { Note } from "@/lib/types/Note";

export function Sidebar({
  collapsed,
  onToggle,
}: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  const pathname = usePathname();
  const [filter, setFilter] = useState("");

  const notesQuery = useQuery<Note[], Error>({
    queryKey: ["notes", { limit: 200 }],
    queryFn: () => api.listNotes({ limit: 200 }),
  });

  const notes = notesQuery.data ?? [];
  const filtered = filter
    ? notes.filter((n) =>
        n.title.toLowerCase().includes(filter.toLowerCase()),
      )
    : notes;

  if (collapsed) {
    return (
      <div className="w-10 border-r border-border flex flex-col items-center py-3">
        <button
          onClick={onToggle}
          className="text-muted-foreground hover:text-foreground text-xs"
          title="Expand sidebar"
        >
          ▶
        </button>
      </div>
    );
  }

  return (
    <div className="w-60 border-r border-border flex flex-col shrink-0">
      <div className="flex items-center justify-between px-3 py-3 border-b border-border">
        <Link href="/" className="text-sm font-medium tracking-tight">
          Tesela
        </Link>
        <button
          onClick={onToggle}
          className="text-muted-foreground hover:text-foreground text-xs"
          title="Collapse sidebar"
        >
          ◀
        </button>
      </div>

      <div className="px-3 py-2">
        <input
          type="text"
          placeholder="Filter pages…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="w-full text-xs bg-accent/50 rounded px-2 py-1.5 text-foreground placeholder:text-muted-foreground outline-none focus:ring-1 focus:ring-ring/30"
        />
      </div>

      <nav className="flex-1 overflow-y-auto px-1.5 pb-2">
        <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider px-1.5 py-1.5">
          Pages
        </div>
        {notesQuery.isLoading && (
          <div className="px-1.5 py-1 text-xs text-muted-foreground">
            Loading…
          </div>
        )}
        {filtered.map((note) => {
          const isActive =
            pathname === `/p/${encodeURIComponent(note.id)}`;
          return (
            <Link
              key={note.id}
              href={`/p/${encodeURIComponent(note.id)}`}
              className={`block rounded px-1.5 py-1 text-xs truncate transition-colors ${
                isActive
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
              }`}
            >
              {note.title}
            </Link>
          );
        })}
        {filtered.length === 0 && !notesQuery.isLoading && (
          <div className="px-1.5 py-1 text-xs text-muted-foreground">
            {filter ? "No matches" : "No notes"}
          </div>
        )}
      </nav>

      <div className="border-t border-border px-3 py-2">
        <div className="text-[10px] text-muted-foreground">
          {notes.length} notes
        </div>
      </div>
    </div>
  );
}

"use client";

import { useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useEffect, useState } from "react";

import { api, ApiError } from "@/lib/api-client";
import { wsClient } from "@/lib/ws-client";
import type { Note } from "@/lib/types/Note";

/**
 * M0 boot screen — proves the whole stack is wired:
 *   1. TanStack Query fetches /notes from tesela-server
 *   2. WebSocket connects and reports live status
 *   3. Dark-first theme from shadcn neutral palette is applied
 *
 * Intentionally minimal. Aesthetic polish comes in later milestones (M6+).
 */
export default function Home() {
  const notesQuery = useQuery<Note[], Error>({
    queryKey: ["notes", { limit: 100 }],
    queryFn: () => api.listNotes({ limit: 100 }),
  });

  const [wsConnected, setWsConnected] = useState(() => wsClient.isConnected);
  useEffect(() => {
    wsClient.setHandlers({
      onConnectionStateChanged: setWsConnected,
      onNoteCreated: () => notesQuery.refetch(),
      onNoteUpdated: () => notesQuery.refetch(),
      onNoteDeleted: () => notesQuery.refetch(),
    });
  }, [notesQuery]);

  return (
    <main className="flex-1 flex flex-col">
      <header className="border-b border-border px-6 py-4 flex items-center justify-between">
        <h1 className="text-sm font-medium tracking-tight">Tesela</h1>
        <StatusPill wsConnected={wsConnected} loading={notesQuery.isLoading} />
      </header>

      <section className="flex-1 overflow-y-auto">
        {notesQuery.isLoading && <LoadingState />}
        {notesQuery.isError && <ErrorState error={notesQuery.error} />}
        {notesQuery.data && notesQuery.data.length === 0 && <EmptyState />}
        {notesQuery.data && notesQuery.data.length > 0 && (
          <NotesList notes={notesQuery.data} />
        )}
      </section>
    </main>
  );
}

function StatusPill({
  wsConnected,
  loading,
}: {
  wsConnected: boolean;
  loading: boolean;
}) {
  const label = loading ? "loading" : wsConnected ? "live" : "offline";
  const dotColor = wsConnected ? "bg-emerald-500" : "bg-muted-foreground";
  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground">
      <span className={`inline-block h-1.5 w-1.5 rounded-full ${dotColor}`} />
      <span>{label}</span>
    </div>
  );
}

function LoadingState() {
  return (
    <div className="px-6 py-8 text-sm text-muted-foreground">Loading…</div>
  );
}

function ErrorState({ error }: { error: Error }) {
  const detail =
    error instanceof ApiError
      ? `${error.status} — ${error.body || "no body"}`
      : error.message;
  return (
    <div className="px-6 py-8 text-sm">
      <div className="text-destructive font-medium">Could not reach tesela-server</div>
      <div className="mt-1 text-muted-foreground">{detail}</div>
      <div className="mt-3 text-xs text-muted-foreground">
        Start it with <code className="font-mono">cargo run -p tesela-server</code> and reload.
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="px-6 py-8 text-sm text-muted-foreground">No notes yet.</div>
  );
}

function NotesList({ notes }: { notes: Note[] }) {
  return (
    <ul className="divide-y divide-border">
      {notes.map((note) => (
        <li key={note.id}>
          <Link
            href={`/p/${encodeURIComponent(note.id)}`}
            className="block px-6 py-3 hover:bg-accent/50"
          >
            <div className="flex items-baseline justify-between gap-4">
              <span className="text-sm font-medium truncate">{note.title}</span>
              <span className="text-xs text-muted-foreground font-mono shrink-0">
                {formatTimestamp(note.modified_at)}
              </span>
            </div>
          </Link>
        </li>
      ))}
    </ul>
  );
}

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

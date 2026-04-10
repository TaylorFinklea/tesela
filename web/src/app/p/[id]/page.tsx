"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useRef } from "react";

import { api, ApiError } from "@/lib/api-client";
import { NoteEditor } from "@/components/NoteEditor";
import type { Note } from "@/lib/types/Note";

export default function NotePage() {
  const { id } = useParams<{ id: string }>();
  const queryClient = useQueryClient();
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const noteQuery = useQuery<Note, Error>({
    queryKey: ["note", id],
    queryFn: () => api.getNote(id),
    enabled: !!id,
  });

  const handleContentChange = useCallback(
    (content: string) => {
      // Debounce saves at 500ms
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        try {
          const updated = await api.updateNote(id, content);
          // Update the cache without refetching
          queryClient.setQueryData(["note", id], updated);
        } catch (e) {
          console.error("Save failed:", e);
        }
      }, 500);
    },
    [id, queryClient],
  );

  if (noteQuery.isLoading) {
    return (
      <div className="flex-1 flex flex-col">
        <NoteHeader title="Loading…" />
        <div className="px-8 py-6 text-sm text-muted-foreground">Loading…</div>
      </div>
    );
  }

  if (noteQuery.isError) {
    const detail =
      noteQuery.error instanceof ApiError
        ? `${noteQuery.error.status} — ${noteQuery.error.body || "unknown"}`
        : noteQuery.error.message;
    return (
      <div className="flex-1 flex flex-col">
        <NoteHeader title="Error" />
        <div className="px-8 py-6 text-sm">
          <div className="text-destructive font-medium">
            Could not load note
          </div>
          <div className="mt-1 text-muted-foreground">{detail}</div>
        </div>
      </div>
    );
  }

  const note = noteQuery.data!;

  return (
    <div className="flex-1 flex flex-col">
      <NoteHeader title={note.title} />
      <div className="flex-1 overflow-y-auto px-8 py-4">
        <NoteEditor
          initialContent={note.content}
          onContentChange={handleContentChange}
          className="min-h-full"
        />
      </div>
    </div>
  );
}

function NoteHeader({ title }: { title: string }) {
  return (
    <header className="border-b border-border px-6 py-4 flex items-center gap-4">
      <Link
        href="/"
        className="text-xs text-muted-foreground hover:text-foreground"
      >
        &larr; Notes
      </Link>
      <h1 className="text-sm font-medium tracking-tight truncate">{title}</h1>
    </header>
  );
}

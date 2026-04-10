"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useMemo, useRef } from "react";

import { api, ApiError } from "@/lib/api-client";
import { BlockOutliner } from "@/components/BlockOutliner";
import type { Note } from "@/lib/types/Note";

/**
 * Splits a note's content into frontmatter + body.
 * Returns { frontmatter: "---\n...\n---\n", body: "..." }
 */
function splitContent(content: string): { frontmatter: string; body: string } {
  if (!content.startsWith("---")) {
    return { frontmatter: "", body: content };
  }
  const endIdx = content.indexOf("---", 3);
  if (endIdx === -1) {
    return { frontmatter: "", body: content };
  }
  const fmEnd = endIdx + 3;
  const afterFm = content.slice(fmEnd);
  // Skip the newline right after ---
  const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
  return {
    frontmatter: content.slice(0, fmEnd) + "\n",
    body: afterFm.slice(bodyStart),
  };
}

export default function NotePage() {
  const { id } = useParams<{ id: string }>();
  const queryClient = useQueryClient();
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const noteQuery = useQuery<Note, Error>({
    queryKey: ["note", id],
    queryFn: () => api.getNote(id),
    enabled: !!id,
  });

  const { frontmatter, body } = useMemo(
    () => splitContent(noteQuery.data?.content ?? ""),
    [noteQuery.data?.content],
  );

  const handleContentChange = useCallback(
    (fullContent: string) => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        try {
          const updated = await api.updateNote(id, fullContent);
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
      <NoteHeader title={note.title} tags={note.metadata.tags} />
      <div className="flex-1 overflow-y-auto px-8 py-4">
        <BlockOutliner
          noteId={note.id}
          body={body}
          frontmatter={frontmatter}
          onContentChange={handleContentChange}
        />
      </div>
    </div>
  );
}

function NoteHeader({ title, tags }: { title: string; tags?: string[] }) {
  return (
    <header className="border-b border-border px-6 py-4 flex items-center gap-4">
      <Link
        href="/"
        className="text-xs text-muted-foreground hover:text-foreground"
      >
        &larr; Notes
      </Link>
      <h1 className="text-sm font-medium tracking-tight truncate">{title}</h1>
      {tags && tags.length > 0 && (
        <div className="flex gap-1">
          {tags.map((tag) => (
            <span
              key={tag}
              className="text-xs px-1.5 py-0.5 rounded bg-accent text-accent-foreground"
            >
              {tag}
            </span>
          ))}
        </div>
      )}
    </header>
  );
}

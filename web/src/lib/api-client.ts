/**
 * Typed fetch wrapper for tesela-server REST API.
 * Types from ts-rs (crates/tesela-core).
 */
import type { Note } from "$lib/types/Note";

const BASE_URL = "http://127.0.0.1:7474";

export class ApiError extends Error {
  constructor(
    public status: number,
    public body: string,
    public url: string,
  ) {
    super(`API ${status} ${url}: ${body}`);
    this.name = "ApiError";
  }
}

async function get<T>(path: string): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, { headers: { Accept: "application/json" } });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as T;
}

async function put<T>(path: string, body: unknown): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, {
    method: "PUT",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as T;
}

export const api = {
  health: () => get<{ status: string }>("/health"),
  listNotes: (params: { tag?: string; limit?: number; offset?: number } = {}) => {
    const q = new URLSearchParams();
    if (params.tag) q.set("tag", params.tag);
    if (params.limit !== undefined) q.set("limit", String(params.limit));
    if (params.offset !== undefined) q.set("offset", String(params.offset));
    const qs = q.toString();
    return get<Note[]>(`/notes${qs ? `?${qs}` : ""}`);
  },
  getNote: (id: string) => get<Note>(`/notes/${encodeURIComponent(id)}`),
  updateNote: (id: string, content: string) =>
    put<Note>(`/notes/${encodeURIComponent(id)}`, { content }),
};

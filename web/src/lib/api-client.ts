/**
 * Typed fetch wrapper for the tesela-server REST API.
 *
 * Types are generated from Rust via ts-rs (see crates/tesela-core/src/*).
 * The server runs on http://127.0.0.1:7474 by default (see crates/tesela-server).
 *
 * Only the endpoints needed by the current milestone are wired here;
 * extend as features require them.
 */

import type { Note } from "@/lib/types/Note";

export interface ApiClientOptions {
  baseUrl?: string;
  fetchImpl?: typeof fetch;
}

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

export class ApiClient {
  readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(opts: ApiClientOptions = {}) {
    this.baseUrl = opts.baseUrl ?? "http://127.0.0.1:7474";
    this.fetchImpl = opts.fetchImpl ?? fetch.bind(globalThis);
  }

  /** Health probe — used at startup to confirm the server is up. */
  async health(): Promise<{ status: string }> {
    return this.get<{ status: string }>("/health");
  }

  /** List notes, optionally filtered by tag. */
  async listNotes(params: ListNotesParams = {}): Promise<Note[]> {
    const query = new URLSearchParams();
    if (params.tag) query.set("tag", params.tag);
    if (params.limit !== undefined) query.set("limit", String(params.limit));
    if (params.offset !== undefined) query.set("offset", String(params.offset));
    const qs = query.toString();
    return this.get<Note[]>(`/notes${qs ? `?${qs}` : ""}`);
  }

  private async get<T>(path: string): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const res = await this.fetchImpl(url, {
      headers: { Accept: "application/json" },
    });
    if (!res.ok) {
      throw new ApiError(res.status, await res.text(), url);
    }
    return (await res.json()) as T;
  }
}

export interface ListNotesParams {
  tag?: string;
  limit?: number;
  offset?: number;
}

/** Default singleton for convenience. Most callers can just use this. */
export const api = new ApiClient();

/**
 * Pure request-body builders for the tesela-server REST API.
 *
 * Extracted from `api-client.ts` so the wire shape is unit-testable in plain
 * `node --test` (api-client itself isn't node-importable: its `ApiError` uses
 * TS parameter properties and it value-imports `$lib/ws-refresh-coordinator`).
 * Keeping the load-bearing body construction here lets the contract live in
 * one auditable, tested place.
 */

/** The JSON body for `PUT /notes/{id}`. `base_content` is OMITTED (not sent as
 *  `undefined`) when no base is supplied so the wire shape matches a legacy
 *  client byte-for-byte (server falls back to its server-file → content diff).
 *  When present, the server diffs `base_content → content` — the author's REAL
 *  changes — so an untouched block is never re-asserted over a concurrent peer
 *  edit. Mirrors `UpdateNoteReq` in `crates/tesela-server/src/routes/notes.rs`
 *  (field name `base_content`, snake_case). */
export function buildUpdateNoteBody(
  content: string,
  baseContent?: string,
): { content: string; base_content?: string } {
  return baseContent !== undefined ? { content, base_content: baseContent } : { content };
}

/**
 * Wire protocol for the live dictation WebSocket
 * (`GET /transcription/stream`, server: routes/transcription.rs).
 *
 * Client → server: binary frames of 16 kHz mono f32-LE PCM, then one
 * `{"type":"stop"}` text frame to finalize (closing without stop
 * cancels). Server → client: the JSON text frames modeled here.
 *
 * Kept alias-free and side-effect-free so `tests/unit` can import it
 * directly under Node's type-stripping (no $lib, no runes).
 */

export type ServerFrame =
  | { type: "ready"; model_id: string; streaming: boolean }
  | { type: "partial"; committed: string; tentative: string; revision: number }
  | { type: "final"; text: string; model_id: string; duration_ms: number }
  | { type: "error"; message: string };

/** The one control frame the client sends. */
export const STOP_FRAME = JSON.stringify({ type: "stop" });

/** Parse a server text frame; null for anything malformed (the caller
 *  treats unknown frames as ignorable, not fatal). */
export function parseServerFrame(raw: string): ServerFrame | null {
  let v: unknown;
  try {
    v = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof v !== "object" || v === null) return null;
  const f = v as Record<string, unknown>;
  switch (f.type) {
    case "ready":
      if (typeof f.model_id === "string" && typeof f.streaming === "boolean") {
        return { type: "ready", model_id: f.model_id, streaming: f.streaming };
      }
      return null;
    case "partial":
      if (
        typeof f.committed === "string" &&
        typeof f.tentative === "string" &&
        typeof f.revision === "number"
      ) {
        return {
          type: "partial",
          committed: f.committed,
          tentative: f.tentative,
          revision: f.revision,
        };
      }
      return null;
    case "final":
      if (typeof f.text === "string" && typeof f.model_id === "string") {
        return {
          type: "final",
          text: f.text,
          model_id: f.model_id,
          duration_ms: typeof f.duration_ms === "number" ? f.duration_ms : 0,
        };
      }
      return null;
    case "error":
      if (typeof f.message === "string") {
        return { type: "error", message: f.message };
      }
      return null;
    default:
      return null;
  }
}

/** mm:ss for the recording timer. */
export function formatElapsed(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

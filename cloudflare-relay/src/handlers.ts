/**
 * Per-endpoint logic, called from the GroupDO's fetch router. MAC
 * verification + replay defence happens HERE (not in the outer
 * Worker) because the DO has the auth_key and the nonce LRU; the
 * outer Worker is just a thin router that forwards to the right DO.
 *
 * Matches the Rust handlers.rs path semantics 1:1 — same response
 * codes, same body shapes, same header names. Conformance test vectors
 * that pass on the Rust side should pass here too.
 */

import type { GroupDO } from "./group-do";
import {
  canonicalRequest,
  constantTimeEq,
  fromB64,
  fromHex,
  hmacSha256,
  isHex,
  sha256Hex,
  toB64,
  toHex,
} from "./crypto";

/** Convert the outer Worker's pre-stripped path back into the canonical
 *  "/groups/{id}/..." form the MAC was signed against. The Worker
 *  stashes the original path in this header so the DO can verify. */
const ORIGINAL_PATH_HEADER = "x-tesela-original-path";
const ORIGINAL_QUERY_HEADER = "x-tesela-original-query";

// ─── /register ────────────────────────────────────────────────────

interface RegisterBody {
  auth_key_b64: string;
  registered_at: number;
  intent_b64: string;
}

export async function handleRegister(self: GroupDO, req: Request): Promise<Response> {
  const body = await req.json<RegisterBody>().catch(() => null);
  if (!body || typeof body.auth_key_b64 !== "string" || typeof body.intent_b64 !== "string") {
    return json({ error: "invalid body" }, 400);
  }
  const auth_key = fromB64(body.auth_key_b64);
  const intent = fromB64(body.intent_b64);
  if (auth_key.length !== 32) return json({ error: "auth_key must be 32 bytes" }, 400);
  if (intent.length !== 32) return json({ error: "intent must be 32 bytes" }, 400);

  const outcome = self.insertOrFetchRegistration(auth_key, body.registered_at, intent);
  if (outcome.outcome === "conflict" && outcome.existing) {
    return json(
      {
        auth_key_b64: toB64(outcome.existing.auth_key),
        registered_at: outcome.existing.registered_at,
        intent_b64: toB64(outcome.existing.intent),
      },
      409,
    );
  }
  return json({ status: "ok" }, 200);
}

export async function handleGetRegistration(self: GroupDO, _req: Request): Promise<Response> {
  const reg = self.getRegistration();
  if (!reg) return json({ error: "not registered" }, 404);
  return json({
    auth_key_b64: toB64(reg.auth_key),
    registered_at: reg.registered_at,
    intent_b64: toB64(reg.intent),
  });
}

// ─── MAC gate (everything below) ──────────────────────────────────

interface MacContext {
  /** The acking/fetching device, IF a well-formed X-Tesela-Device header
   *  was supplied. Optional because the Rust MAC gate does NOT require it
   *  — it's read opportunistically (e.g. for device-seen touch). */
  device_id?: Uint8Array;
  ok: true;
}

async function verifyMac(self: GroupDO, req: Request, bodyBytes: Uint8Array): Promise<MacContext | Response> {
  // Header presence first (mirrors the Rust gate: only MAC + nonce + ts
  // are required; X-Tesela-Device / X-Tesela-Group are NOT — requiring
  // them would 401 a client the Rust relay would accept).
  const ts = req.headers.get("x-tesela-ts");
  const nonce = req.headers.get("x-tesela-nonce");
  const mac = req.headers.get("x-tesela-mac");
  if (!mac) return json({ error: "missing X-Tesela-Mac" }, 401);
  if (!nonce) return json({ error: "missing X-Tesela-Nonce" }, 401);
  if (!ts) return json({ error: "missing X-Tesela-Ts" }, 401);

  // Replay window check. Guard the env parse so an empty/garbage value
  // can't collapse the window to 0s and reject every request.
  const parsedWindow = Number(self.env.TESELA_RELAY_REPLAY_WINDOW_SECS ?? "300");
  const window = Number.isFinite(parsedWindow) && parsedWindow > 0 ? parsedWindow : 300;
  const tsNum = Number(ts);
  const now = Math.floor(Date.now() / 1000);
  if (!Number.isFinite(tsNum) || Math.abs(now - tsNum) > window) {
    return json({ error: "timestamp out of window" }, 400);
  }

  if (!self.recordNonce(nonce)) {
    return json({ error: "nonce replay" }, 400);
  }

  const reg = self.getRegistration();
  if (!reg) return json({ error: "group not registered" }, 401);

  // Reconstruct canonical_request from the outer Worker's header-
  // pinned original path + query. Body hash is "" for empty bodies
  // (matches Rust's body_hash_hex behaviour).
  const originalPath = req.headers.get(ORIGINAL_PATH_HEADER) ?? new URL(req.url).pathname;
  const originalQuery = req.headers.get(ORIGINAL_QUERY_HEADER) ?? "";
  const bodyHash = bodyBytes.length === 0 ? "" : await sha256Hex(bodyBytes);
  const canonical = canonicalRequest(req.method, originalPath, originalQuery, nonce, ts, bodyHash);

  const expected = await hmacSha256(reg.auth_key, canonical);
  const given = fromB64(mac);
  if (!constantTimeEq(expected, given)) {
    return json({ error: "mac mismatch" }, 401);
  }

  // Read the device id opportunistically — only if present + well-formed.
  const deviceHex = req.headers.get("x-tesela-device");
  const device_id = deviceHex && isHex(deviceHex, 16) ? fromHex(deviceHex) : undefined;
  return { device_id, ok: true };
}

// ─── /ops (PUT + GET) ─────────────────────────────────────────────

interface PutOpBody {
  from_device: string;          // 32 hex chars
  payload_b64: string;
}

export async function handlePutOp(self: GroupDO, req: Request): Promise<Response> {
  const max = Number(self.env.TESELA_RELAY_MAX_BODY ?? "1048576");
  const raw = new Uint8Array(await req.arrayBuffer());
  if (raw.length > max) return json({ error: "body too large" }, 413);

  const macCheck = await verifyMac(self, req, raw);
  if (macCheck instanceof Response) return macCheck;

  const body = JSON.parse(new TextDecoder().decode(raw)) as PutOpBody;
  if (typeof body.from_device !== "string" || !isHex(body.from_device, 16)) {
    return json({ error: "from_device must be 16 bytes of hex" }, 400);
  }
  const from_device = fromHex(body.from_device);
  const payload = fromB64(body.payload_b64);

  const ts = Date.now() / 1000;
  const { seq } = self.insertOp(from_device, ts, payload);
  self.touchDevice(from_device, ts);
  return json({ seq, ts });
}

export async function handleGetOps(self: GroupDO, req: Request): Promise<Response> {
  const macCheck = await verifyMac(self, req, new Uint8Array());
  if (macCheck instanceof Response) return macCheck;

  const url = new URL(req.url);
  const since = Number(url.searchParams.get("since") ?? "0");
  const rows = self.listOpsSince(Number.isFinite(since) ? since : 0);
  // Touch the device-seen index (only when a device id was supplied) so
  // consumers count toward known-members — same as the Rust side, which
  // also skips the touch when the header is absent.
  if (macCheck.device_id) self.touchDevice(macCheck.device_id, Date.now() / 1000);

  const records = rows.map((r) => ({
    seq: r.seq,
    from_device: toHex(r.from_device),
    ts: r.ts,
    payload_b64: toB64(r.payload),
  }));
  return json(records);
}

// ─── /ack ─────────────────────────────────────────────────────────

interface AckBody {
  device: string;        // 32 hex chars
  applied_seq: number;
}

export async function handlePostAck(self: GroupDO, req: Request): Promise<Response> {
  const max = Number(self.env.TESELA_RELAY_MAX_BODY ?? "1048576");
  const raw = new Uint8Array(await req.arrayBuffer());
  if (raw.length > max) return json({ error: "body too large" }, 413);

  const macCheck = await verifyMac(self, req, raw);
  if (macCheck instanceof Response) return macCheck;

  const body = JSON.parse(new TextDecoder().decode(raw)) as AckBody;
  if (typeof body.device !== "string" || !isHex(body.device, 16)) {
    return json({ error: "device must be 16 bytes of hex" }, 400);
  }
  const device = fromHex(body.device);
  self.ackOps(device, body.applied_seq);
  self.touchDevice(device, Date.now() / 1000);
  return json({ status: "ok" });
}

// ─── /snapshot (PUT) + /snapshots (GET) — spine Phase 1b ───────────

interface SnapshotEntry {
  stream_id_b64: string;
  payload_b64: string;
}

interface PutSnapshotBody {
  covers_seq: number;
  snapshots: SnapshotEntry[];
}

export async function handlePutSnapshot(self: GroupDO, req: Request): Promise<Response> {
  const max = Number(self.env.TESELA_RELAY_MAX_BODY ?? "1048576");
  const raw = new Uint8Array(await req.arrayBuffer());
  if (raw.length > max) return json({ error: "body too large" }, 413);

  const macCheck = await verifyMac(self, req, raw);
  if (macCheck instanceof Response) return macCheck;

  const body = JSON.parse(new TextDecoder().decode(raw)) as PutSnapshotBody;
  if (!Array.isArray(body.snapshots)) return json({ error: "snapshots must be an array" }, 400);
  // stream_id + payload are OPAQUE to the relay — decode b64 to bytes
  // for storage, never interpret.
  const decoded = body.snapshots.map((s) => ({
    stream_id: fromB64(s.stream_id_b64),
    payload: fromB64(s.payload_b64),
  }));

  const gc = self.depositSnapshotBatch(body.covers_seq, decoded);
  return json({ ok: true, gc });
}

export async function handleGetSnapshots(self: GroupDO, req: Request): Promise<Response> {
  const macCheck = await verifyMac(self, req, new Uint8Array());
  if (macCheck instanceof Response) return macCheck;

  const compaction_seq = self.getCompactionSeq();
  const snapshots = self.listSnapshots().map((s) => ({
    stream_id_b64: toB64(s.stream_id),
    snapshot_seq: s.snapshot_seq,
    payload_b64: toB64(s.payload),
  }));
  return json({ compaction_seq, snapshots });
}

// ─── /admin/registration (DELETE) ─────────────────────────────────

export async function handleAdminDelete(self: GroupDO, req: Request): Promise<Response> {
  // Admin endpoints are DISABLED (404) when no admin token is configured
  // — matches the Rust handler's "admin endpoints disabled" 404.
  const expected = self.env.TESELA_RELAY_ADMIN_TOKEN;
  if (!expected) return new Response("admin endpoints disabled", { status: 404 });
  const auth = req.headers.get("authorization") ?? "";
  const expectedHeader = `Bearer ${expected}`;
  // Constant-time string compare (Web Crypto doesn't have one).
  if (auth.length !== expectedHeader.length) return json({ error: "unauthorized" }, 401);
  let diff = 0;
  for (let i = 0; i < auth.length; i++) diff |= auth.charCodeAt(i) ^ expectedHeader.charCodeAt(i);
  if (diff !== 0) return json({ error: "unauthorized" }, 401);

  // 204 on a real delete, 404 if the group wasn't registered — mirrors
  // the Rust `delete_registration` rows-affected check.
  const existed = self.getRegistration() !== null;
  self.deleteRegistration();
  if (!existed) return new Response("group not registered", { status: 404 });
  return new Response(null, { status: 204 });
}

// ─── helpers ──────────────────────────────────────────────────────

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

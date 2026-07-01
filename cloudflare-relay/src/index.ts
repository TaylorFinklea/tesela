/**
 * Tesela Sync Relay — Cloudflare Worker entry point.
 *
 * Routes:
 *   POST   /groups/:id/register             — onboarding (TOFU)
 *   GET    /groups/:id/registration         — read back to verify intent
 *   PUT    /groups/:id/ops                  — deposit an envelope
 *   GET    /groups/:id/ops?since=N          — drain since seq N
 *   POST   /groups/:id/ack                  — record applied seq
 *   DELETE /admin/groups/:id/register       — hijack recovery (admin token)
 *   GET    /                                — health check
 *
 * Each `:id` is the 32-hex-char group_id. The Worker uses the id as
 * the Durable Object name, so all requests for that group land on the
 * same DO instance + share its SQLite storage + nonce LRU.
 *
 * Wire format is identical to the Rust self-host
 * (crates/tesela-relay) — clients written against either work against
 * both. See .docs/ai/phases/2026-05-24-relay-protocol-design.md.
 */

import { GroupDO, type Env } from "./group-do";
import { DiscoveryIndexDO } from "./discovery-do";
export { GroupDO, DiscoveryIndexDO };

// Constant-time-ish string compare via header-pinning. Outer Worker
// uses these to ferry the original path/query into the DO so MAC
// verification can rebuild the canonical request without us needing
// to thread URL parsing through DO boundaries.
const ORIGINAL_PATH_HEADER = "x-tesela-original-path";
const ORIGINAL_QUERY_HEADER = "x-tesela-original-query";

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);

    if (url.pathname === "/" || url.pathname === "/health") {
      return json({ service: "tesela-relay", status: "ok", impl: "cloudflare-worker" });
    }

    // /groups/:id/<rest>
    const groupsMatch = url.pathname.match(/^\/groups\/([0-9a-f]{32})(\/.*)$/);
    if (groupsMatch) {
      const groupIdHex = groupsMatch[1]!;
      const innerPath = groupsMatch[2]!;
      return await forwardToGroupDO(env, groupIdHex, innerPath, url, req);
    }

    // /admin/groups/:id/register → admin DELETE for hijack recovery.
    const adminMatch = url.pathname.match(/^\/admin\/groups\/([0-9a-f]{32})\/register$/);
    if (adminMatch && req.method === "DELETE") {
      const groupIdHex = adminMatch[1]!;
      return await forwardToGroupDO(env, groupIdHex, "/admin/registration", url, req);
    }

    // GET /discover/:disc — recovery-phrase discovery (ra7 P0.2b). Mirrors
    // the Rust relay's UNAUTHENTICATED discover() handler: a phrase-only
    // device has the GroupKey but not the random group_id, so it can't yet
    // derive an auth_key or hit any MAC-gated endpoint. `disc` is the
    // 32-byte discovery handle, hex-encoded (64 hex chars).
    if (url.pathname.startsWith("/discover/")) {
      const discMatch = url.pathname.match(/^\/discover\/([0-9a-f]{64})$/);
      if (!discMatch) {
        return json({ error: "invalid disc hex" }, 400);
      }
      return await forwardToDiscoveryDO(env, discMatch[1]!);
    }

    return new Response("not found", { status: 404 });
  },
};

async function forwardToGroupDO(
  env: Env,
  groupIdHex: string,
  innerPath: string,
  outerUrl: URL,
  req: Request,
): Promise<Response> {
  const id = env.GROUP_DO.idFromName(groupIdHex);
  const stub = env.GROUP_DO.get(id);

  // The DO sees the trimmed path (e.g. "/ops" instead of
  // "/groups/<id>/ops") so its switch stays readable. We ferry the ORIGINAL
  // path + query along so MAC verification can rebuild the canonical request.
  // The DO doesn't care about hostname; use a fixed origin so the URL is
  // well-formed.
  const inner = `https://do.internal${innerPath}${outerUrl.search}`;

  // WebSocket upgrades MUST forward the ORIGINAL Request: re-issuing a plain
  // Request drops the upgrade state and the 101 + webSocket would never
  // survive. A Request we construct (unlike the inbound one) has mutable
  // headers, so we still pin the original path/query for verifyMac.
  if ((req.headers.get("upgrade") ?? "").toLowerCase() === "websocket") {
    const wsReq = new Request(inner, req);
    wsReq.headers.set(ORIGINAL_PATH_HEADER, outerUrl.pathname);
    wsReq.headers.set(ORIGINAL_QUERY_HEADER, outerUrl.search.replace(/^\?/, ""));
    return await stub.fetch(wsReq);
  }

  const headers = new Headers(req.headers);
  headers.set(ORIGINAL_PATH_HEADER, outerUrl.pathname);
  headers.set(ORIGINAL_QUERY_HEADER, outerUrl.search.replace(/^\?/, ""));
  const innerReq = new Request(inner, {
    method: req.method,
    headers,
    body: ["GET", "HEAD"].includes(req.method) ? undefined : req.body,
  });
  return await stub.fetch(innerReq);
}

async function forwardToDiscoveryDO(env: Env, discHex: string): Promise<Response> {
  const id = env.DISCOVERY_DO.idFromName("global");
  const stub = env.DISCOVERY_DO.get(id);
  return await stub.fetch(`https://do.internal/lookup/${discHex}`);
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

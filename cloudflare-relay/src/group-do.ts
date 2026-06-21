/**
 * GroupDO — Durable Object class holding one group's relay state.
 *
 * Each `group_id` maps to exactly one DO instance via
 * `env.GROUP_DO.idFromName(group_id_hex)`. Cloudflare pins the DO to
 * a single edge location, so we get serialized access without
 * cross-region coordination. Storage is the DO's built-in SQLite
 * (durable, per-DO, written-to-disk-on-commit).
 *
 * Schema mirrors the Rust self-host's store.rs. Note: any `.exec()`
 * calls below are Cloudflare's Durable Object SQL API
 * (`state.storage.sql.exec`), NOT child_process — totally different
 * namespace from Node's `exec`.
 */

import {
  handleRegister,
  handleGetRegistration,
  handlePutOp,
  handleGetOps,
  handlePostAck,
  handleAdminDelete,
  handlePutSnapshot,
  handleGetSnapshots,
  handleRegisterDevice,
} from "./handlers";

export interface Env {
  GROUP_DO: DurableObjectNamespace;
  TESELA_RELAY_ADMIN_TOKEN?: string;
  TESELA_RELAY_MAX_BODY?: string;
  TESELA_RELAY_REPLAY_WINDOW_SECS?: string;
  // APNs config (sync durability P3c) — Worker secrets/vars, set once
  // the .p8 key is provisioned. Absent until then: sendApnsBackgroundPush
  // no-ops (returns false), so the relay runs fine without them. These
  // make Env structurally satisfy `ApnsEnv` from ./apns.
  APNS_KEY_P8?: string;
  APNS_KEY_ID?: string;
  APNS_TEAM_ID?: string;
  APNS_BUNDLE_ID?: string;
  APNS_HOST?: string;
}

/** Per-IP request cap, mirroring the Rust self-host's sliding window
 *  (`state.rs` RATE_LIMIT_MAX / window). Bursts above this in a window
 *  get 429'd. Per-DO + per-IP: each group's DO tracks its own callers. */
const RATE_LIMIT_MAX = 1000;
const RATE_LIMIT_WINDOW_MS = 10_000;

export class GroupDO implements DurableObject {
  /** Nonce → expiration epoch (ms). Anything older gets sweeped on lookup. */
  private nonces = new Map<string, number>();
  /** 5-minute nonce TTL — matches the Rust LRU window. */
  private static readonly NONCE_TTL_MS = 5 * 60 * 1000;

  /** Per-IP request timestamps (ms) for the sliding-window rate limit.
   *  In-memory in the DO — the same caller for a group always lands on
   *  this DO instance, so the window is accurate per (group, IP). */
  private ipHits = new Map<string, number[]>();

  constructor(
    public state: DurableObjectState,
    public env: Env,
  ) {
    this.state.blockConcurrencyWhile(async () => {
      await this.ensureSchema();
    });
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    const path = url.pathname;

    // Per-IP rate limit runs first so even pre-auth scan traffic gets
    // throttled (mirrors the Rust `rate_gate` layered over everything).
    const ip =
      req.headers.get("cf-connecting-ip") ??
      req.headers.get("x-forwarded-for") ??
      "0.0.0.0";
    if (!this.checkIpRate(ip)) {
      return new Response("rate limit exceeded", { status: 429 });
    }

    try {
      switch (`${req.method} ${path}`) {
        case "POST /register":
          return await handleRegister(this, req);
        case "GET /registration":
          return await handleGetRegistration(this, req);
        case "PUT /ops":
          return await handlePutOp(this, req);
        case "GET /ops":
          return await handleGetOps(this, req);
        case "POST /ack":
          return await handlePostAck(this, req);
        case "POST /devices":
          return await handleRegisterDevice(this, req);
        case "PUT /snapshot":
          return await handlePutSnapshot(this, req);
        case "GET /snapshots":
          return await handleGetSnapshots(this, req);
        case "DELETE /admin/registration":
          return await handleAdminDelete(this, req);
        default:
          return new Response("not found", { status: 404 });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      return new Response(`internal: ${msg}`, { status: 500 });
    }
  }

  private async ensureSchema(): Promise<void> {
    this.state.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS registration (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        auth_key BLOB NOT NULL,
        registered_at INTEGER NOT NULL,
        intent BLOB NOT NULL
      );
      CREATE TABLE IF NOT EXISTS ops (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        from_device BLOB NOT NULL,
        ts REAL NOT NULL,
        payload BLOB NOT NULL,
        ack_devices TEXT NOT NULL DEFAULT '[]'
      );
      CREATE TABLE IF NOT EXISTS device_seen (
        device_id BLOB PRIMARY KEY,
        last_seen_ts INTEGER NOT NULL
      );
      CREATE INDEX IF NOT EXISTS ops_seq_idx ON ops(seq);

      -- Latest encrypted snapshot per opaque stream (the client's
      -- per-note key). stream_id + payload are OPAQUE — the relay never
      -- interprets them. A snapshot batch covering relay-seq N is the
      -- compaction gate: once deposited, ops with seq <= N are GC'd
      -- (snapshot-gated compaction, spine Phase 1b).
      CREATE TABLE IF NOT EXISTS snapshots (
        stream_id BLOB PRIMARY KEY,
        snapshot_seq INTEGER NOT NULL,
        payload BLOB NOT NULL,
        created_at INTEGER NOT NULL
      );

      -- Single-row compaction watermark: highest relay-seq a deposited
      -- snapshot batch has covered. Ops at/below it are GC-eligible.
      CREATE TABLE IF NOT EXISTS group_meta (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        compaction_seq INTEGER NOT NULL DEFAULT 0
      );

      -- APNs device-token registry (sync durability P3b). One row per
      -- device id (16 bytes) → its APNs push token (hex). A device token
      -- is a routing identifier, NOT note content — storing it keeps the
      -- relay zero-knowledge. On a PUT /ops the DO pushes a
      -- content-available wake to the group's OTHER tokens (P3c).
      CREATE TABLE IF NOT EXISTS device_tokens (
        device_id BLOB PRIMARY KEY,
        apns_token TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      );
    `);
  }

  getRegistration(): { auth_key: Uint8Array; registered_at: number; intent: Uint8Array } | null {
    // NB: DO SQLite's cursor `.one()` THROWS on zero rows, so an
    // unregistered group must be read with `.toArray()` + a length
    // check — otherwise GET /registration on an absent group would 500
    // instead of 404.
    const rows = this.state.storage.sql
      .exec<{ auth_key: ArrayBuffer; registered_at: number; intent: ArrayBuffer }>(
        "SELECT auth_key, registered_at, intent FROM registration WHERE id = 1",
      )
      .toArray();
    const row = rows[0];
    if (!row) return null;
    return {
      auth_key: new Uint8Array(row.auth_key),
      registered_at: Number(row.registered_at),
      intent: new Uint8Array(row.intent),
    };
  }

  insertOrFetchRegistration(
    auth_key: Uint8Array,
    registered_at: number,
    intent: Uint8Array,
  ): { outcome: "inserted" | "idempotent" | "conflict"; existing?: { auth_key: Uint8Array; registered_at: number; intent: Uint8Array } } {
    const existing = this.getRegistration();
    if (existing) {
      const sameKey = bytesEq(existing.auth_key, auth_key);
      const sameTs = existing.registered_at === registered_at;
      const sameIntent = bytesEq(existing.intent, intent);
      if (sameKey && sameTs && sameIntent) {
        return { outcome: "idempotent", existing };
      }
      return { outcome: "conflict", existing };
    }
    this.state.storage.sql.exec(
      "INSERT INTO registration (id, auth_key, registered_at, intent) VALUES (1, ?, ?, ?)",
      auth_key.buffer,
      registered_at,
      intent.buffer,
    );
    return { outcome: "inserted" };
  }

  deleteRegistration(): void {
    this.state.storage.sql.exec("DELETE FROM registration");
    this.state.storage.sql.exec("DELETE FROM ops");
    this.state.storage.sql.exec("DELETE FROM device_seen");
    this.state.storage.sql.exec("DELETE FROM snapshots");
    this.state.storage.sql.exec("DELETE FROM group_meta");
  }

  insertOp(from_device: Uint8Array, ts: number, payload: Uint8Array): { seq: number; ts: number } {
    this.state.storage.sql.exec(
      "INSERT INTO ops (from_device, ts, payload) VALUES (?, ?, ?)",
      from_device.buffer,
      ts,
      payload.buffer,
    );
    const row = this.state.storage.sql
      .exec<{ seq: number }>("SELECT last_insert_rowid() as seq")
      .one()!;
    return { seq: Number(row.seq), ts };
  }

  listOpsSince(since: number): Array<{ seq: number; from_device: Uint8Array; ts: number; payload: Uint8Array }> {
    const rows = this.state.storage.sql.exec<{
      seq: number;
      from_device: ArrayBuffer;
      ts: number;
      payload: ArrayBuffer;
    }>("SELECT seq, from_device, ts, payload FROM ops WHERE seq > ? ORDER BY seq ASC", since);
    return [...rows].map((r) => ({
      seq: Number(r.seq),
      from_device: new Uint8Array(r.from_device),
      ts: Number(r.ts),
      payload: new Uint8Array(r.payload),
    }));
  }

  ackOps(device: Uint8Array, applied_seq: number): void {
    const deviceHex = bytesToHex(device);
    const rows = this.state.storage.sql.exec<{ seq: number; ack_devices: string }>(
      "SELECT seq, ack_devices FROM ops WHERE seq <= ?",
      applied_seq,
    );
    for (const r of rows) {
      const acked = new Set<string>(JSON.parse(r.ack_devices) as string[]);
      if (acked.has(deviceHex)) continue;
      acked.add(deviceHex);
      this.state.storage.sql.exec(
        "UPDATE ops SET ack_devices = ? WHERE seq = ?",
        JSON.stringify([...acked]),
        r.seq,
      );
    }
    // DURABLE-REPLICA retention (encrypted-replica spine, Phase 1a): the
    // relay KEEPS the full encrypted op log rather than evicting acked
    // ops — it is the off-site backup + bootstrap source, so a wiped
    // device restores the WHOLE mosaic via GET /ops?since=0. Eviction is
    // no longer ack-gated; compaction is snapshot-gated (see
    // depositSnapshotBatch). The ack_devices bookkeeping above is kept
    // for cursor/known-member use. Mirrors the Rust `post_ack` change.
  }

  // ── Snapshots + snapshot-gated compaction (spine Phase 1b) ─────────

  /** Deposit a full snapshot batch covering relay-seq `coversSeq`, then
   *  compact: upsert each per-stream snapshot, advance the watermark
   *  (forward-only), and GC ops with seq <= coversSeq. Returns the
   *  number of ops deleted. The DO serializes access so the three steps
   *  are effectively atomic. Mirrors Rust `store::deposit_snapshot_batch`. */
  depositSnapshotBatch(
    coversSeq: number,
    snapshots: Array<{ stream_id: Uint8Array; payload: Uint8Array }>,
  ): number {
    const now = Math.floor(Date.now() / 1000);
    for (const s of snapshots) {
      this.state.storage.sql.exec(
        "INSERT INTO snapshots (stream_id, snapshot_seq, payload, created_at) VALUES (?, ?, ?, ?) " +
          "ON CONFLICT (stream_id) DO UPDATE SET " +
          "snapshot_seq = excluded.snapshot_seq, payload = excluded.payload, created_at = excluded.created_at",
        s.stream_id.buffer,
        coversSeq,
        s.payload.buffer,
        now,
      );
    }
    // Advance the compaction watermark (never regress).
    this.state.storage.sql.exec(
      "INSERT INTO group_meta (id, compaction_seq) VALUES (1, ?) " +
        "ON CONFLICT (id) DO UPDATE SET compaction_seq = MAX(group_meta.compaction_seq, excluded.compaction_seq)",
      coversSeq,
    );
    // Count then delete the superseded ops (COUNT is robust regardless
    // of the DO cursor's rows-written semantics).
    const cnt = this.state.storage.sql
      .exec<{ n: number }>("SELECT COUNT(*) AS n FROM ops WHERE seq <= ?", coversSeq)
      .one()!;
    this.state.storage.sql.exec("DELETE FROM ops WHERE seq <= ?", coversSeq);
    return Number(cnt.n);
  }

  /** Latest snapshot per opaque stream, ordered by stream_id. */
  listSnapshots(): Array<{ stream_id: Uint8Array; snapshot_seq: number; payload: Uint8Array }> {
    const rows = this.state.storage.sql.exec<{
      stream_id: ArrayBuffer;
      snapshot_seq: number;
      payload: ArrayBuffer;
    }>("SELECT stream_id, snapshot_seq, payload FROM snapshots ORDER BY stream_id ASC");
    return [...rows].map((r) => ({
      stream_id: new Uint8Array(r.stream_id),
      snapshot_seq: Number(r.snapshot_seq),
      payload: new Uint8Array(r.payload),
    }));
  }

  /** Group's compaction watermark (0 if no snapshot deposited). */
  getCompactionSeq(): number {
    const rows = this.state.storage.sql
      .exec<{ compaction_seq: number }>("SELECT compaction_seq FROM group_meta WHERE id = 1")
      .toArray();
    return rows[0] ? Number(rows[0].compaction_seq) : 0;
  }

  touchDevice(device: Uint8Array, ts: number): void {
    this.state.storage.sql.exec(
      "INSERT INTO device_seen (device_id, last_seen_ts) VALUES (?, ?) " +
        "ON CONFLICT (device_id) DO UPDATE SET last_seen_ts = excluded.last_seen_ts",
      device.buffer,
      Math.floor(ts),
    );
  }

  // ── APNs device-token registry (sync durability P3b/P3c) ──────────

  /** Upsert a device's APNs push token (sync durability P3b). */
  upsertDeviceToken(device_id: Uint8Array, apns_token: string, ts: number): void {
    this.state.storage.sql.exec(
      "INSERT INTO device_tokens (device_id, apns_token, updated_at) VALUES (?, ?, ?) " +
        "ON CONFLICT (device_id) DO UPDATE SET apns_token = excluded.apns_token, updated_at = excluded.updated_at",
      device_id.buffer,
      apns_token,
      Math.floor(ts),
    );
  }

  /** APNs tokens of every device in the group EXCEPT `exclude_device`
   *  (the depositor — it already has the op). Filtered in JS by hex to
   *  avoid BLOB-comparison subtleties, mirroring `ackOps`. */
  listOtherApnsTokens(exclude_device: Uint8Array): string[] {
    const excludeHex = bytesToHex(exclude_device);
    const rows = this.state.storage.sql.exec<{ device_id: ArrayBuffer; apns_token: string }>(
      "SELECT device_id, apns_token FROM device_tokens",
    );
    return [...rows]
      .filter((r) => bytesToHex(new Uint8Array(r.device_id)) !== excludeHex)
      .map((r) => r.apns_token);
  }

  /** Prune a permanently-dead APNs token (APNs reported 410 Unregistered
   *  or BadDeviceToken) so a stale token left by a reinstalled device
   *  isn't pushed — and logged as a failure — on every future deposit. */
  deleteDeviceTokenByToken(apns_token: string): void {
    this.state.storage.sql.exec("DELETE FROM device_tokens WHERE apns_token = ?", apns_token);
  }

  /** Hard upper bound on remembered nonces per DO — a backstop so a
   *  sustained burst of distinct nonces can't grow the map without limit
   *  (the rate limiter already bounds the inflow rate). */
  private static readonly NONCE_HARD_CAP = 50_000;

  recordNonce(nonce: string): boolean {
    const now = Date.now();
    const existing = this.nonces.get(nonce);
    if (existing !== undefined && existing > now) return false;

    // Sweep expired entries once the map grows past ~1k, then enforce a
    // hard cap by evicting oldest (Map preserves insertion order) — even
    // if every remembered nonce is still fresh. Mirrors the bounded LRU
    // the Rust relay keeps.
    if (this.nonces.size >= 1000) {
      for (const [k, exp] of this.nonces) {
        if (exp <= now) this.nonces.delete(k);
      }
      while (this.nonces.size >= GroupDO.NONCE_HARD_CAP) {
        const oldest = this.nonces.keys().next().value;
        if (oldest === undefined) break;
        this.nonces.delete(oldest);
      }
    }

    this.nonces.set(nonce, now + GroupDO.NONCE_TTL_MS);
    return true;
  }

  /** Sliding-window per-IP rate limit. Returns false once an IP exceeds
   *  RATE_LIMIT_MAX requests inside RATE_LIMIT_WINDOW_MS. */
  checkIpRate(ip: string): boolean {
    const now = Date.now();
    const cutoff = now - RATE_LIMIT_WINDOW_MS;
    const hits = (this.ipHits.get(ip) ?? []).filter((t) => t > cutoff);
    if (hits.length >= RATE_LIMIT_MAX) {
      this.ipHits.set(ip, hits);
      return false;
    }
    hits.push(now);
    this.ipHits.set(ip, hits);
    return true;
  }
}

function bytesEq(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function bytesToHex(b: Uint8Array): string {
  let out = "";
  for (let i = 0; i < b.length; i++) {
    out += b[i]!.toString(16).padStart(2, "0");
  }
  return out;
}

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

import { handleRegister, handleGetRegistration, handlePutOp, handleGetOps, handlePostAck, handleAdminDelete } from "./handlers";

export interface Env {
  GROUP_DO: DurableObjectNamespace;
  TESELA_RELAY_ADMIN_TOKEN?: string;
  TESELA_RELAY_MAX_BODY?: string;
  TESELA_RELAY_REPLAY_WINDOW_SECS?: string;
}

const KNOWN_MEMBER_TTL_SECS = 30 * 24 * 60 * 60; // 30 days

export class GroupDO implements DurableObject {
  /** Nonce → expiration epoch (ms). Anything older gets sweeped on lookup. */
  private nonces = new Map<string, number>();
  /** 5-minute nonce TTL — matches the Rust LRU window. */
  private static readonly NONCE_TTL_MS = 5 * 60 * 1000;

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
    `);
  }

  getRegistration(): { auth_key: Uint8Array; registered_at: number; intent: Uint8Array } | null {
    const row = this.state.storage.sql
      .exec<{ auth_key: ArrayBuffer; registered_at: number; intent: ArrayBuffer }>(
        "SELECT auth_key, registered_at, intent FROM registration WHERE id = 1",
      )
      .one();
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
    this.gcFullyAckedOps();
  }

  private gcFullyAckedOps(): void {
    const now = Math.floor(Date.now() / 1000);
    const cutoff = now - KNOWN_MEMBER_TTL_SECS;
    const knownRows = this.state.storage.sql.exec<{ device_id: ArrayBuffer }>(
      "SELECT device_id FROM device_seen WHERE last_seen_ts > ?",
      cutoff,
    );
    const known = new Set<string>([...knownRows].map((r) => bytesToHex(new Uint8Array(r.device_id))));
    if (known.size === 0) return;
    const opRows = this.state.storage.sql.exec<{ seq: number; ack_devices: string }>(
      "SELECT seq, ack_devices FROM ops",
    );
    for (const r of opRows) {
      const acked = new Set<string>(JSON.parse(r.ack_devices) as string[]);
      let allAcked = true;
      for (const k of known) {
        if (!acked.has(k)) {
          allAcked = false;
          break;
        }
      }
      if (allAcked) {
        this.state.storage.sql.exec("DELETE FROM ops WHERE seq = ?", r.seq);
      }
    }
  }

  touchDevice(device: Uint8Array, ts: number): void {
    this.state.storage.sql.exec(
      "INSERT INTO device_seen (device_id, last_seen_ts) VALUES (?, ?) " +
        "ON CONFLICT (device_id) DO UPDATE SET last_seen_ts = excluded.last_seen_ts",
      device.buffer,
      Math.floor(ts),
    );
  }

  recordNonce(nonce: string): boolean {
    const now = Date.now();
    if (this.nonces.size > 1000) {
      for (const [k, exp] of this.nonces) {
        if (exp < now) this.nonces.delete(k);
      }
    }
    const existing = this.nonces.get(nonce);
    if (existing !== undefined && existing > now) return false;
    this.nonces.set(nonce, now + GroupDO.NONCE_TTL_MS);
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

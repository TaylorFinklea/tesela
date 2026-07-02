/**
 * DiscoveryIndexDO — a SINGLE global Durable Object holding the
 * disc -> group_id discovery index (recovery-phrase discovery, ra7
 * P0 step 2b, CF parity for the Rust relay's
 * `relay_discovery_index` table added in P0.2a).
 *
 * GroupDO instances are sharded per-group (`idFromName(group_id_hex)`),
 * but this index is a cross-group lookup keyed by `disc` — a value the
 * Worker/GroupDO layer has no group_id-scoped home for. Every caller
 * (the outer Worker's /discover route, and GroupDO's handleRegister)
 * addresses the SAME instance via `env.DISCOVERY_DO.idFromName("global")`,
 * so it is strongly consistent (unlike KV, which is eventually
 * consistent and would make register-then-discover flaky). Storage is
 * the DO's built-in SQLite, mirroring GroupDO's pattern.
 */

export class DiscoveryIndexDO implements DurableObject {
  constructor(public state: DurableObjectState) {
    this.state.blockConcurrencyWhile(async () => {
      await this.ensureSchema();
    });
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    const path = url.pathname;

    try {
      if (req.method === "POST" && path === "/upsert") {
        const body = await req.json<{ disc?: string; group_id?: string }>().catch(() => null);
        if (!body || typeof body.disc !== "string" || typeof body.group_id !== "string") {
          return json({ error: "invalid body" }, 400);
        }
        this.upsert(body.disc, body.group_id);
        return json({ status: "ok" });
      }

      const lookupMatch = path.match(/^\/lookup\/([0-9a-f]+)$/);
      if (req.method === "GET" && lookupMatch) {
        const disc = lookupMatch[1]!;
        const group_id = this.lookup(disc);
        if (group_id === null) return json({ error: "unknown discovery handle" }, 404);
        return json({ group_id });
      }

      // DELETE-by-group (ra7.3): parity with the Rust relay's FK
      // ON DELETE CASCADE — admin hijack-delete on a GroupDO must also
      // scrub this SEPARATE DO's disc->group_id row(s) for that group,
      // or a stale mapping survives the registration wipe.
      const byGroupMatch = path.match(/^\/by-group\/([0-9a-f]{32})$/);
      if (req.method === "DELETE" && byGroupMatch) {
        const group_id = byGroupMatch[1]!;
        this.deleteByGroup(group_id);
        return json({ status: "ok" });
      }

      return new Response("not found", { status: 404 });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      return new Response(`internal: ${msg}`, { status: 500 });
    }
  }

  private async ensureSchema(): Promise<void> {
    this.state.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS discovery_index (
        disc     TEXT PRIMARY KEY,
        group_id TEXT NOT NULL
      );
    `);
  }

  private upsert(disc: string, group_id: string): void {
    this.state.storage.sql.exec(
      "INSERT OR REPLACE INTO discovery_index (disc, group_id) VALUES (?, ?)",
      disc,
      group_id,
    );
  }

  private lookup(disc: string): string | null {
    const rows = this.state.storage.sql
      .exec<{ group_id: string }>("SELECT group_id FROM discovery_index WHERE disc = ?", disc)
      .toArray();
    return rows[0] ? rows[0].group_id : null;
  }

  private deleteByGroup(group_id: string): void {
    this.state.storage.sql.exec("DELETE FROM discovery_index WHERE group_id = ?", group_id);
  }
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

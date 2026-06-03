#!/usr/bin/env node
/**
 * Wire-compatibility proof for collab-editing step C2.1.
 *
 * Proves the JS TLR2 codec (`src/lib/loro/tlr2.ts`) decodes a REAL server
 * binary relay frame byte-for-byte, and that the per-doc update bytes it
 * recovers import cleanly into a JS `loro-crdt` LoroDoc:
 *
 *   1. Open a WebSocket to the server's /ws (binaryType = arraybuffer).
 *   2. Trigger a server edit (POST a block upsert) so the server broadcasts a
 *      TLR2 Loro-delta frame.
 *   3. On the inbound BINARY frame: `decodeTlr2` → assert non-null, ≥1 update,
 *      doc id is 16 bytes. Then `LoroDoc.import(updates[0].updateBytes)` —
 *      assert it does not throw (the update applies).
 *   4. A pure-JS round-trip: `decodeTlr2(encodeTlr2([...]))` deep-equals input.
 *
 * Run (server must be up at 127.0.0.1:7474):
 *   node web/scripts/loro-frame-check.mjs
 *
 * Node ≥22.18 (here: Node 26) strips TS types natively, so we import the codec
 * straight from its `.ts` source — the `import type` and annotations in
 * tlr2.ts are erased, and its `fflate` import resolves against
 * `web/node_modules`. `loro-crdt` resolves to its synchronous nodejs entry and
 * Node ships a global `WebSocket`.
 */
import { encodeTlr2, decodeTlr2 } from "../src/lib/loro/tlr2.ts";
import { LoroDoc } from "loro-crdt";

const BASE = process.env.TESELA_API_TARGET ?? "http://127.0.0.1:7474";
const WS_URL = (process.env.TESELA_WS_TARGET ?? "ws://127.0.0.1:7474") + "/ws";
const SLUG = process.env.TESELA_SLUG ?? "2026-06-03";

let failures = 0;
function assert(cond, msg) {
  if (cond) {
    console.log(`  PASS: ${msg}`);
  } else {
    failures += 1;
    console.error(`  FAIL: ${msg}`);
  }
}

function hex(bytes) {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Pull a trailing-empty (or any) block's bid from the note markdown. */
async function pickBid(slug) {
  const res = await fetch(`${BASE}/notes/${slug}`);
  if (!res.ok) throw new Error(`GET /notes/${slug} -> ${res.status}`);
  const note = await res.json();
  const md = note?.content ?? "";
  const bids = [...md.matchAll(/bid:([0-9a-fA-F-]+)/g)].map((m) => m[1]);
  if (bids.length === 0) throw new Error("no bid found in note markdown");
  // Prefer a trailing-empty block (a line that is just "- " before its marker)
  // so the probe edit doesn't trash real text. Fall back to the last bid.
  const emptyMatch = [
    ...md.matchAll(/^-\s*<!--\s*bid:([0-9a-fA-F-]+)\s*-->/gm),
  ].map((m) => m[1]);
  return { bid: emptyMatch[emptyMatch.length - 1] ?? bids[bids.length - 1], md };
}

/** Fire a block upsert so the server broadcasts a TLR2 delta. */
async function triggerEdit(slug, bid, text) {
  const res = await fetch(`${BASE}/notes/${slug}/blocks`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ops: [{ kind: "upsert", bid, text, indent_level: 0 }],
    }),
  });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`POST blocks -> ${res.status}: ${body.slice(0, 200)}`);
  }
}

function pureRoundTrip() {
  console.log("\n== 2. Pure-JS encode/decode round-trip ==");
  const docId = new Uint8Array(16).fill(0xab);
  const updateBytes = new Uint8Array([1, 2, 3, 4, 5, 250, 0, 99, 128, 255]);
  const input = [{ doc: docId, updateBytes }];
  const frame = encodeTlr2(input);
  assert(
    frame[0] === 0x54 &&
      frame[1] === 0x4c &&
      frame[2] === 0x52 &&
      frame[3] === 0x32,
    `frame starts with TLR2 magic (got ${hex(frame.slice(0, 4))})`,
  );
  const back = decodeTlr2(frame);
  assert(back !== null, "decodeTlr2(encodeTlr2(...)) is non-null");
  assert(back?.length === 1, "round-trip preserves update count (1)");
  if (back && back[0]) {
    assert(
      hex(back[0].doc) === hex(docId),
      "round-trip doc id deep-equals input (16 bytes 0xAB)",
    );
    assert(
      hex(back[0].updateBytes) === hex(updateBytes),
      "round-trip update_bytes deep-equals input",
    );
  }
  // Foreign / short frames decode to null (mirrors Rust Ok(None)).
  assert(decodeTlr2(new Uint8Array([])) === null, "empty frame -> null");
  assert(
    decodeTlr2(new Uint8Array([0x54, 0x4c])) === null,
    "short magic (\"TL\") -> null",
  );
  assert(
    decodeTlr2(new Uint8Array([0x58, 0x58, 0x58, 0x58])) === null,
    'foreign magic ("XXXX") -> null',
  );
}

async function realFrameCheck() {
  console.log("== 1. Decode a REAL server TLR2 frame end-to-end ==");
  const { bid } = await pickBid(SLUG);
  console.log(`  slug=${SLUG}  edit bid=${bid}`);

  const ws = new WebSocket(WS_URL);
  ws.binaryType = "arraybuffer";

  const got = await new Promise((resolve) => {
    let settled = false;
    const finish = (val) => {
      if (settled) return;
      settled = true;
      resolve(val);
    };
    const timeout = setTimeout(() => {
      finish({ kind: "timeout" });
    }, 8000);

    ws.addEventListener("message", (ev) => {
      // Text frames are JSON note-event echoes; we only want the binary delta.
      if (typeof ev.data === "string") return;
      clearTimeout(timeout);
      finish({ kind: "binary", data: ev.data });
    });
    ws.addEventListener("error", (e) => {
      clearTimeout(timeout);
      finish({ kind: "error", error: e?.message ?? String(e) });
    });
    ws.addEventListener("open", async () => {
      try {
        const rnd = Math.random().toString(36).slice(2, 8);
        await triggerEdit(SLUG, bid, `C21 frame check ${rnd}`);
      } catch (err) {
        clearTimeout(timeout);
        finish({ kind: "error", error: err?.message ?? String(err) });
      }
    });
  });

  try {
    ws.close();
  } catch {
    // ignore
  }

  if (got.kind === "timeout") {
    failures += 1;
    console.error(
      "  FAIL: no binary frame arrived within 8s (did the server broadcast a TLR2 delta?)",
    );
    return;
  }
  if (got.kind === "error") {
    failures += 1;
    console.error(`  FAIL: WS/edit error: ${got.error}`);
    return;
  }

  const bytes = new Uint8Array(got.data);
  console.log(
    `  received binary frame: ${bytes.byteLength} bytes, first 16 = ${hex(bytes.slice(0, 16))}`,
  );

  let updates = null;
  let threw = false;
  try {
    updates = decodeTlr2(bytes);
  } catch (err) {
    threw = true;
    console.error(`  decodeTlr2 threw: ${err?.stack ?? err}`);
  }
  assert(!threw, "decodeTlr2(server_frame) did not throw");
  if (updates === null) {
    failures += 1;
    console.error(
      `  FAIL: decodeTlr2 returned null — server frame did NOT match TLR2 format. First 16 bytes: ${hex(bytes.slice(0, 16))}`,
    );
    return;
  }
  assert(updates !== null, "decodeTlr2(server_frame) is non-null (matched TLR2 magic)");
  assert(updates.length >= 1, `server frame carries >= 1 update (${updates.length})`);

  const first = updates[0];
  assert(
    first.doc instanceof Uint8Array && first.doc.length === 16,
    `first update's doc id is 16 bytes (got ${first?.doc?.length})`,
  );
  console.log(`  first doc id (hex): ${hex(first.doc)}`);
  console.log(`  first update_bytes: ${first.updateBytes.length} bytes`);

  // Import the recovered update bytes into a fresh JS LoroDoc.
  const doc = new LoroDoc();
  let importThrew = false;
  try {
    doc.import(first.updateBytes);
  } catch (err) {
    importThrew = true;
    console.error(`  LoroDoc.import threw: ${err?.stack ?? err}`);
  }
  assert(
    !importThrew,
    "LoroDoc.import(updates[0].update_bytes) applied without throwing",
  );
  if (!importThrew) {
    let nodeCount = 0;
    try {
      nodeCount = doc.getTree("blocks").getNodes({ withDeleted: true }).length;
    } catch {
      // A pure delta update may not populate a readable tree on its own; the
      // import-not-throwing assertion above is the real wire-compat proof.
    }
    console.log(
      `  imported OK — "blocks" tree node count after import: ${nodeCount}`,
    );
  }
}

async function main() {
  await realFrameCheck();
  pureRoundTrip();
  console.log(
    `\n== RESULT: ${failures === 0 ? "TLR2 WIRE-COMPATIBLE ✓ (all assertions passed)" : `FAILED — ${failures} assertion(s) failed`} ==`,
  );
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((err) => {
  console.error(`UNCAUGHT: ${err?.stack ?? err}`);
  process.exit(2);
});

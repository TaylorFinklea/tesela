/**
 * C2.2 convergence proof: does the web peer's per-note Loro doc converge with
 * the tesela-server over the LIVE delta channel?
 *
 * Run from `web/`:
 *   node --import ./scripts/ts-resolve-hook.mjs scripts/loro-converge-check.mjs
 * (server must be running at http://127.0.0.1:7474). The `--import` hook lets
 * plain node resolve the real `.ts` app modules this script imports.
 *
 * Steps (each prints PASS/FAIL):
 *   1. noteIdHex("2026-06-03") === the daily's server doc id, confirmed by
 *      decoding a REAL inbound TLR2 frame's `doc` over the WS (the wire path).
 *   2. Bootstrap: fetch the snapshot, import into a fresh LoroDoc, record
 *      block X's text.
 *   3. Live delta: open the WS, POST an upsert changing block X's text, decode
 *      the inbound TLR2 frame, filter to the daily doc, `doc.import` it, and
 *      assert blockTextByBid(X) on the WEB doc === the new server text (also
 *      read back via GET /notes to confirm both sides agree).
 *
 * Uses the SAME modules the app ships: `decodeTlr2`, `noteIdHex`, and the
 * `NoteDoc` read helpers — so this exercises the real code, not a re-impl.
 */
import { decodeTlr2 } from "../src/lib/loro/tlr2.ts";
import { noteIdHex } from "../src/lib/loro/note-id.ts";
import { NoteDoc } from "../src/lib/loro/note-doc.ts";

const BASE = "http://127.0.0.1:7474";
const WS = "ws://127.0.0.1:7474/ws";
const SLUG = "2026-06-03";
// Trailing-empty block in the daily — safe to overwrite without trashing real
// text. (If absent on your server, swap for any existing bid in the daily.)
const BLOCK_X = "51879bac-1d95-461f-a1e4-fa7d85ad47be";

let failures = 0;
function check(label, ok, detail = "") {
  const tag = ok ? "PASS" : "FAIL";
  if (!ok) failures++;
  console.log(`[${tag}] ${label}${detail ? " — " + detail : ""}`);
}

const hexOf = (bytes) =>
  Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");

/** Trigger an edit on block X, return the new text. */
async function upsertBlockX(text) {
  const res = await fetch(`${BASE}/notes/${SLUG}/blocks`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ops: [
        {
          kind: "upsert",
          bid: BLOCK_X,
          text,
          parent_bid: null,
          indent_level: 0,
        },
      ],
    }),
  });
  if (!res.ok) throw new Error(`upsert HTTP ${res.status}`);
  return text;
}

/** GET the daily and pull block X's current text out of the body markdown. */
async function serverTextForX() {
  const res = await fetch(`${BASE}/notes/${SLUG}`);
  if (!res.ok) throw new Error(`GET note HTTP ${res.status}`);
  const note = await res.json();
  const dashless = BLOCK_X.replace(/-/g, "");
  for (const line of (note.body ?? "").split("\n")) {
    const m = line.match(/^- (.*) <!-- bid:([0-9a-fA-F-]+) -->\s*$/);
    if (m && m[2].replace(/-/g, "").toLowerCase() === dashless) {
      return m[1];
    }
  }
  return null;
}

/**
 * Open the WS, run `trigger()` to cause a server edit, and resolve with the
 * first inbound TLR2 frame's decoded updates. Times out after `ms`.
 */
function captureNextDelta(trigger, ms = 8000) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(WS);
    ws.binaryType = "arraybuffer";
    let done = false;
    const finish = (fn, arg) => {
      if (done) return;
      done = true;
      try {
        ws.close();
      } catch {
        /* ignore */
      }
      fn(arg);
    };
    const timer = setTimeout(
      () => finish(reject, new Error("timed out waiting for TLR2 frame")),
      ms,
    );
    ws.addEventListener("open", () => {
      Promise.resolve(trigger()).catch((e) => {
        clearTimeout(timer);
        finish(reject, e);
      });
    });
    ws.addEventListener("message", (ev) => {
      if (!(ev.data instanceof ArrayBuffer)) return;
      let updates;
      try {
        updates = decodeTlr2(new Uint8Array(ev.data));
      } catch (e) {
        clearTimeout(timer);
        finish(reject, e);
        return;
      }
      if (updates === null) return; // non-TLR2 frame; keep waiting
      clearTimeout(timer);
      finish(resolve, updates);
    });
    ws.addEventListener("error", (e) => {
      clearTimeout(timer);
      finish(reject, new Error("WS error: " + (e?.message ?? "unknown")));
    });
  });
}

async function main() {
  console.log(`=== C2.2 Loro convergence check (slug=${SLUG}) ===\n`);
  const wantHex = noteIdHex(SLUG);
  console.log(`noteIdHex("${SLUG}") = ${wantHex}\n`);

  // --- Step 1: confirm the slug→doc-id mapping against the LIVE wire. ---
  // Cause a real edit and read the doc id off the inbound TLR2 frame.
  const rand1 = Math.random().toString(36).slice(2, 8);
  let serverDocHex = null;
  try {
    const updates = await captureNextDelta(() =>
      upsertBlockX(`C22 wire-id ${rand1}`),
    );
    const match = updates.find((u) => hexOf(u.doc) === wantHex);
    serverDocHex = match ? hexOf(match.doc) : hexOf(updates[0]?.doc ?? []);
    check(
      "step 1: noteIdHex(slug) === live server doc id (decoded TLR2 frame)",
      serverDocHex === wantHex,
      `server doc id=${serverDocHex}`,
    );
  } catch (e) {
    check("step 1: capture live TLR2 frame", false, String(e));
  }

  // --- Step 2: bootstrap a fresh web doc from the server snapshot. ---
  const doc = new NoteDoc(BASE);
  await doc.open(SLUG);
  const beforeWeb = doc.blockTextByBid(BLOCK_X);
  const beforeServer = await serverTextForX();
  check(
    "step 2: bootstrap imported snapshot (block X present in web doc)",
    beforeWeb !== null,
    `web[X] before=${JSON.stringify(beforeWeb)}`,
  );
  check(
    "step 2: bootstrapped web doc agrees with server before the edit",
    (beforeWeb ?? "") === (beforeServer ?? ""),
    `server[X] before=${JSON.stringify(beforeServer)}`,
  );

  // --- Step 3: live delta apply → web doc converges with server. ---
  const rand2 = Math.random().toString(36).slice(2, 8);
  const newText = `C22 converge ${rand2}`;
  let applied = 0;
  try {
    const updates = await captureNextDelta(() => upsertBlockX(newText));
    for (const u of updates) {
      if (hexOf(u.doc) !== wantHex) continue;
      doc.doc.import(u.updateBytes);
      applied++;
    }
    check(
      "step 3: inbound TLR2 frame carried an update for the daily doc",
      applied > 0,
      `applied ${applied} update(s)`,
    );
  } catch (e) {
    check("step 3: capture+apply live delta", false, String(e));
  }

  const afterWeb = doc.blockTextByBid(BLOCK_X);
  const afterServer = await serverTextForX();
  console.log("");
  console.log(`block X = ${BLOCK_X}`);
  console.log(`  before:  web=${JSON.stringify(beforeWeb)}  server=${JSON.stringify(beforeServer)}`);
  console.log(`  after:   web=${JSON.stringify(afterWeb)}  server=${JSON.stringify(afterServer)}`);
  console.log("");

  check(
    "step 3: web doc updated to the new text via the live delta",
    afterWeb === newText,
    `web[X] after=${JSON.stringify(afterWeb)} expected=${JSON.stringify(newText)}`,
  );
  check(
    "step 3: web doc CONVERGED with server (web[X] === server[X])",
    (afterWeb ?? "") === (afterServer ?? "") && afterWeb === newText,
  );

  doc.close();

  console.log("");
  if (failures === 0) {
    console.log("=== ALL CHECKS PASSED — web doc converges with server ===");
    process.exit(0);
  } else {
    console.log(`=== ${failures} CHECK(S) FAILED ===`);
    process.exit(1);
  }
}

main().catch((e) => {
  console.error("converge-check crashed:", e);
  process.exit(1);
});

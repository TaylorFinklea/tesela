/**
 * C2.3 binding proof (no browser): does a LOCAL character splice on each of two
 * peers — applied through the SAME splice helper the editor binding uses
 * (`NoteDoc.spliceBlock`) — MERGE rather than clobber when the peers exchange
 * deltas over the TLR2 codec the WS carries?
 *
 * Run from `web/`:
 *   node --import ./scripts/ts-resolve-hook.mjs scripts/loro-editor-binding-check.mjs
 * (no server needed — the shared base is built in-process). The `--import` hook
 * lets plain node resolve the real `.ts` app modules.
 *
 * This is the headless analogue of the two-browser test the caller will drive:
 *   1. Build a shared base doc (one block "A" with text "hello"), snapshot it.
 *   2. Bootstrap two NoteDocs (the real C2.2 class) from that base via the
 *      public `applyInbound` (TLR2-framed, exactly the WS inbound path).
 *   3. Peer 1 splices " world" at the end; peer 2 splices "Hi " at the front —
 *      concurrent edits to the SAME block, each via `spliceBlock` (the editor's
 *      write-path helper).
 *   4. Exchange deltas: export each peer's update since the base, frame with
 *      `encodeTlr2`, decode with `decodeTlr2`, apply with `applyInbound`.
 *   5. Assert BOTH peers converge to the SAME text AND that text contains BOTH
 *      contributions (interleaved) — NOT last-writer-wins.
 *
 * Uses the SAME modules the app ships: `NoteDoc` (+ its new `spliceBlock` /
 * `exportSince` / `currentVersion` / `blockTextByBid`), `encodeTlr2`,
 * `decodeTlr2`, `noteId`. The only test-only piece is constructing the shared
 * base with raw `loro-crdt` (the server normally produces it).
 */
import { LoroDoc, LoroText } from "loro-crdt";
import { NoteDoc } from "../src/lib/loro/note-doc.ts";
import { encodeTlr2, decodeTlr2 } from "../src/lib/loro/tlr2.ts";
import { noteId } from "../src/lib/loro/note-id.ts";

// No server in this headless test — make the NoteDoc bootstrap fetch resolve as
// a 404 immediately (the documented "no server doc yet → empty doc" path), so
// `open()` returns fast and quietly. We seed the shared base ourselves below.
globalThis.fetch = async () =>
  new Response(null, { status: 404, statusText: "Not Found" });

const SLUG = "c23-binding-check";
const BID_DASHED = "019e8d0e-1690-7c3a-9b2e-fa7d85ad47be";
const BID_HEX = BID_DASHED.replace(/-/g, "").toLowerCase();
const NOTE_ID16 = noteId(SLUG);

let failures = 0;
function check(label, ok, detail = "") {
  const tag = ok ? "PASS" : "FAIL";
  if (!ok) failures++;
  console.log(`[${tag}] ${label}${detail ? " — " + detail : ""}`);
}

/** Build a base doc with the server's shape: a `"blocks"` movable-tree whose
 *  one root node carries `block_id` meta + a nested `text_seq` LoroText. Return
 *  its full snapshot bytes. A FIXED peer id makes the base identical for both
 *  peers (shared history), so their later splices are concurrent, not disjoint. */
function buildBaseSnapshot(initialText) {
  const doc = new LoroDoc();
  doc.setPeerId("1000");
  const tree = doc.getTree("blocks");
  const node = tree.createNode();
  node.data.set("block_id", BID_HEX);
  node.data.set("indent_level", 0);
  const text = node.data.getOrCreateContainer("text_seq", new LoroText());
  text.insert(0, initialText);
  doc.commit();
  return doc.export({ mode: "snapshot" });
}

/** Frame an exported update as TLR2 for this note id (the WS outbound path),
 *  then decode it back (the WS inbound path) into LoroDocUpdate[]. */
function roundTripFrame(updateBytes) {
  const frame = encodeTlr2([{ doc: NOTE_ID16, updateBytes }]);
  const decoded = decodeTlr2(frame);
  if (decoded === null) throw new Error("decodeTlr2 returned null (not a TLR2 frame)");
  return decoded;
}

async function makePeer(baseSnapshot) {
  // `open()` fetches a snapshot; the stubbed `fetch` returns 404, so the doc
  // starts empty. We then seed the shared base through the real inbound path.
  const peer = new NoteDoc("");
  await peer.open(SLUG);
  peer.applyInbound([{ doc: NOTE_ID16, updateBytes: baseSnapshot }]);
  return peer;
}

async function main() {
  console.log("=== C2.3 editor-binding merge check (headless) ===\n");

  const base = buildBaseSnapshot("hello");
  const peer1 = await makePeer(base);
  const peer2 = await makePeer(base);

  check(
    "both peers bootstrapped the shared base (block A = 'hello')",
    peer1.blockTextByBid(BID_DASHED) === "hello" &&
      peer2.blockTextByBid(BID_DASHED) === "hello",
    `p1=${JSON.stringify(peer1.blockTextByBid(BID_DASHED))} p2=${JSON.stringify(peer2.blockTextByBid(BID_DASHED))}`,
  );

  // Capture each peer's version AT the shared base, so the export carries ONLY
  // that peer's own splice (the delta the editor's flush would send).
  const vv1Base = peer1.currentVersion();
  const vv2Base = peer2.currentVersion();

  // Concurrent LOCAL splices to the SAME block, via the editor's write helper:
  //   peer 1 appends " world" at utf16 offset 5 (end of "hello")
  //   peer 2 prepends "Hi "   at utf16 offset 0 (front)
  check("peer1 spliceBlock(append) ran", peer1.spliceBlock(BID_DASHED, 5, 0, " world"));
  check("peer2 spliceBlock(prepend) ran", peer2.spliceBlock(BID_DASHED, 0, 0, "Hi "));

  console.log(`  pre-exchange: p1=${JSON.stringify(peer1.blockTextByBid(BID_DASHED))} p2=${JSON.stringify(peer2.blockTextByBid(BID_DASHED))}`);

  // Export each peer's delta-since-base, frame+decode over TLR2, cross-apply.
  const delta1 = peer1.exportSince(vv1Base);
  const delta2 = peer2.exportSince(vv2Base);
  check("peer1 exported a non-empty delta", delta1.length > 0, `${delta1.length} bytes`);
  check("peer2 exported a non-empty delta", delta2.length > 0, `${delta2.length} bytes`);

  peer1.applyInbound(roundTripFrame(delta2)); // peer 2's edit → peer 1
  peer2.applyInbound(roundTripFrame(delta1)); // peer 1's edit → peer 2

  const t1 = peer1.blockTextByBid(BID_DASHED);
  const t2 = peer2.blockTextByBid(BID_DASHED);
  console.log(`  post-exchange: p1=${JSON.stringify(t1)} p2=${JSON.stringify(t2)}\n`);

  check("both peers CONVERGED to the same text", t1 === t2, `p1=${JSON.stringify(t1)} p2=${JSON.stringify(t2)}`);
  check("peer 1's contribution survived (' world')", (t1 ?? "").includes(" world"));
  check("peer 2's contribution survived ('Hi ')", (t1 ?? "").includes("Hi "));
  check("the original text survived ('hello')", (t1 ?? "").includes("hello"));
  check(
    "result is the interleaved MERGE, not last-writer-wins",
    (t1 ?? "").includes("Hi ") && (t1 ?? "").includes("hello") && (t1 ?? "").includes(" world"),
    `merged=${JSON.stringify(t1)}`,
  );

  peer1.close();
  peer2.close();

  console.log("");
  if (failures === 0) {
    console.log("=== ALL CHECKS PASSED — concurrent same-block splices MERGE ===");
    process.exit(0);
  } else {
    console.log(`=== ${failures} CHECK(S) FAILED ===`);
    process.exit(1);
  }
}

main().catch((e) => {
  console.error("binding-check crashed:", e);
  process.exit(1);
});

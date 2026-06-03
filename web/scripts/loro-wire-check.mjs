#!/usr/bin/env node
/**
 * Wire-compatibility proof for the C2 collab-editing milestone.
 *
 * Proves the JS `loro-crdt` peer shares the Loro 1.x wire format with the Rust
 * `loro 1.12.0` server, by:
 *
 *   1. Fetching the Rust server's per-note snapshot bytes
 *      (`GET /loro/notes/{slug}/snapshot` → `export_doc_update(note_id, None)`)
 *      and importing them into a fresh JS `LoroDoc`.
 *   2. Reading the `"blocks"` movable-tree the way the Rust engine writes it:
 *      each live node's `data` meta map holds a `block_id` (hex) and a nested
 *      `text_seq` LoroText container. Asserting at least one block's text is
 *      readable proves the JS peer decoded the Rust snapshot.
 *   3. A JS<->JS concurrent-splice round-trip: two docs splice the SAME text
 *      container concurrently, cross-import, and converge to one interleaved
 *      string with no lost characters.
 *
 * Run (server must be up at 127.0.0.1:7474):
 *   node web/scripts/loro-wire-check.mjs
 *
 * In Node, `loro-crdt` resolves to its `nodejs/` entry (synchronous wasm), so a
 * plain top-level import is fine here — the SSR guard only matters for the
 * SvelteKit browser bundle (see src/lib/loro/loro-client.ts).
 */
import { LoroDoc, LoroText } from "loro-crdt";

const BASE = process.env.TESELA_API_TARGET ?? "http://127.0.0.1:7474";
const PREFERRED_SLUGS = ["2026-06-03", "2026-06-02"];

let failures = 0;
function assert(cond, msg) {
  if (cond) {
    console.log(`  PASS: ${msg}`);
  } else {
    failures += 1;
    console.error(`  FAIL: ${msg}`);
  }
}

async function fetchSnapshot(slug) {
  const url = `${BASE}/loro/notes/${slug}/snapshot`;
  const res = await fetch(url);
  if (!res.ok) return null;
  const buf = await res.arrayBuffer();
  return new Uint8Array(buf);
}

/** Find a slug whose Loro doc exists, preferring today's daily. */
async function resolveSlug() {
  for (const slug of PREFERRED_SLUGS) {
    const bytes = await fetchSnapshot(slug);
    if (bytes && bytes.byteLength > 0) return { slug, bytes };
  }
  // Fall back: ask the server for its current daily slug.
  const res = await fetch(`${BASE}/notes/daily`);
  if (res.ok) {
    const note = await res.json();
    const slug = note?.id ?? note?.title;
    if (slug) {
      const bytes = await fetchSnapshot(slug);
      if (bytes && bytes.byteLength > 0) return { slug, bytes };
    }
  }
  return null;
}

/**
 * Read the `"blocks"` tree mirroring the Rust engine's read-path:
 * node.data.get("block_id") + node.data's nested `text_seq` LoroText.
 */
function readBlocks(doc) {
  const tree = doc.getTree("blocks");
  const nodes = tree.getNodes({ withDeleted: false });
  const blocks = [];
  for (const node of nodes) {
    const meta = node.data;
    const blockId = meta.get("block_id");
    // Mirror `read_block_text`: prefer the nested `text_seq` LoroText.
    let text = "";
    const seq = meta.get("text_seq");
    if (seq instanceof LoroText) {
      text = seq.toString();
    } else if (typeof seq === "string") {
      text = seq;
    } else {
      const legacy = meta.get("text");
      if (typeof legacy === "string") text = legacy;
    }
    blocks.push({ blockId, text });
  }
  return blocks;
}

async function rustSnapshotImport() {
  console.log("== 1. Import Rust server snapshot into JS loro-crdt ==");
  const resolved = await resolveSlug();
  if (!resolved) {
    failures += 1;
    console.error(
      `  FAIL: no Loro snapshot found at ${BASE} (tried ${PREFERRED_SLUGS.join(", ")} + /notes/daily). Is the server up?`,
    );
    return;
  }
  const { slug, bytes } = resolved;
  console.log(`  slug=${slug}  snapshot=${bytes.byteLength} bytes`);

  const doc = new LoroDoc();
  let imported = false;
  try {
    doc.import(bytes);
    imported = true;
  } catch (err) {
    console.error(`  FAIL: doc.import threw: ${err?.stack ?? err}`);
  }
  assert(imported, "doc.import(rust_snapshot) did not throw");
  if (!imported) return;

  const blocks = readBlocks(doc);
  console.log(`  live blocks in "blocks" tree: ${blocks.length}`);
  assert(blocks.length > 0, "blocks tree has at least one live node");

  const withText = blocks.filter((b) => b.text && b.text.length > 0);
  // Print a representative block (first one carrying readable text).
  const sample = withText[0] ?? blocks[0];
  if (sample) {
    const preview =
      sample.text.length > 80 ? sample.text.slice(0, 80) + "…" : sample.text;
    console.log(`  sample block_id: ${sample.blockId}`);
    console.log(`  sample text_seq: ${JSON.stringify(preview)}`);
  }
  assert(
    !!(sample && sample.blockId),
    "a block exposes its block_id meta (hex)",
  );
  assert(
    withText.length > 0,
    "at least one block's text_seq LoroText is readable",
  );
}

function jsRoundTrip() {
  console.log("\n== 2. JS<->JS concurrent-splice convergence ==");
  // Shared base: one doc seeds a block's text_seq, both peers fork from it.
  const base = new LoroDoc();
  const baseTree = base.getTree("blocks");
  const baseNode = baseTree.createNode();
  baseNode.data.set("block_id", "00".repeat(16));
  const baseText = baseNode.data.getOrCreateContainer("text_seq", new LoroText());
  baseText.insert(0, "Hello");
  base.commit();
  const seed = base.export({ mode: "snapshot" });

  // Two peers fork the same base, then splice concurrently into the SAME
  // text_seq container at different offsets.
  const a = new LoroDoc();
  a.setPeerId("1");
  a.import(seed);
  const b = new LoroDoc();
  b.setPeerId("2");
  b.import(seed);

  const aText = a.getTree("blocks").getNodes()[0].data.getOrCreateContainer(
    "text_seq",
    new LoroText(),
  );
  const bText = b.getTree("blocks").getNodes()[0].data.getOrCreateContainer(
    "text_seq",
    new LoroText(),
  );

  // JS LoroText uses unicode-index `insert` (no `insertUtf16`); the strings
  // here are ASCII so unicode == utf16 == utf8 offsets. The underlying
  // sequence-CRDT encoding is identical to the server's `insert_utf16` writes —
  // wire-compat is about the encoding, not the index coordinate system.
  aText.insert(5, " A-side"); // "Hello A-side"
  a.commit();
  bText.insert(0, "B-side "); // "B-side Hello"
  b.commit();

  // Cross-import each peer's update into the other.
  const aUpdate = a.export({ mode: "snapshot" });
  const bUpdate = b.export({ mode: "snapshot" });
  a.import(bUpdate);
  b.import(aUpdate);

  const aFinal = a
    .getTree("blocks")
    .getNodes()[0]
    .data.getOrCreateContainer("text_seq", new LoroText())
    .toString();
  const bFinal = b
    .getTree("blocks")
    .getNodes()[0]
    .data.getOrCreateContainer("text_seq", new LoroText())
    .toString();

  console.log(`  peer A converged: ${JSON.stringify(aFinal)}`);
  console.log(`  peer B converged: ${JSON.stringify(bFinal)}`);
  assert(aFinal === bFinal, "both peers converge to the SAME string");
  assert(
    aFinal.includes("A-side") && aFinal.includes("B-side"),
    "no concurrent splice was lost (both insertions present)",
  );
  assert(
    aFinal.includes("Hello"),
    "the shared base text survived the interleave",
  );
}

async function main() {
  await rustSnapshotImport();
  jsRoundTrip();
  console.log(
    `\n== RESULT: ${failures === 0 ? "WIRE-COMPATIBLE ✓ (all assertions passed)" : `INCOMPATIBLE / FAILED — ${failures} assertion(s) failed`} ==`,
  );
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((err) => {
  console.error(`UNCAUGHT: ${err?.stack ?? err}`);
  process.exit(2);
});

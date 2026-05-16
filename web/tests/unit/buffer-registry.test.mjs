// Prism v5 buffer + registry tests. Node 22+ strips TypeScript types
// natively. Run via `pnpm test:unit` from web/.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  RendererReferenceMismatch,
  pickCascadeMember,
} from "../../src/lib/buffer/protocol.ts";

import * as pageReg from "../../src/lib/renderers/page/index.ts";
import * as derivedReg from "../../src/lib/renderers/derived/index.ts";
import * as ambientReg from "../../src/lib/renderers/ambient/index.ts";

// Stand-in Svelte component — the registry only stores the value.
const Stub = {};

function freshRegs() {
  pageReg._resetForTests();
  derivedReg._resetForTests();
  ambientReg._resetForTests();
}

// ── page registry ──────────────────────────────────────────────────────────

test("page registry: register and lookup by name + by type", () => {
  freshRegs();
  pageReg.register("note", {
    acceptsType: "note",
    cascade: { default: Stub, modes: [] },
  });
  assert.ok(pageReg.get("note"));
  assert.ok(pageReg.getByType("note"));
  assert.equal(pageReg.getByType("missing"), undefined);
});

test("page registry: re-register replaces the prior entry", () => {
  freshRegs();
  const a = { acceptsType: "note", cascade: { default: Stub, modes: [] } };
  const b = { acceptsType: "note", cascade: { default: Stub, modes: [] } };
  pageReg.register("note", a);
  pageReg.register("note", b);
  assert.equal(pageReg.get("note"), b);
});

test("page registry: names() lists registered keys", () => {
  freshRegs();
  pageReg.register("note", {
    acceptsType: "note",
    cascade: { default: Stub, modes: [] },
  });
  pageReg.register("daily", {
    acceptsType: "daily",
    cascade: { default: Stub, modes: [] },
  });
  assert.deepEqual(pageReg.names().sort(), ["daily", "note"]);
});

// ── derived registry ───────────────────────────────────────────────────────

test("derived registry: mount returns renderer when reference matches", () => {
  freshRegs();
  derivedReg.register("backlinks-of-page", {
    accepts: "page",
    cascade: { default: Stub, modes: [] },
  });
  const r = derivedReg.mount("backlinks-of-page", {
    kind: "page",
    path: "x",
  });
  assert.equal(r.accepts, "page");
});

test("derived registry: mount throws RendererReferenceMismatch on bad kind", () => {
  freshRegs();
  derivedReg.register("backlinks-of-page", {
    accepts: "page",
    cascade: { default: Stub, modes: [] },
  });
  assert.throws(
    () => derivedReg.mount("backlinks-of-page", { kind: "tag", value: "x" }),
    (err) =>
      err instanceof RendererReferenceMismatch &&
      err.expected === "page" &&
      err.got === "tag" &&
      err.rendererName === "backlinks-of-page",
  );
});

test("derived registry: mount throws on unknown renderer name", () => {
  freshRegs();
  assert.throws(() =>
    derivedReg.mount("nope", { kind: "page", path: "x" }),
  );
});

test("derived registry: handles all three reference kinds", () => {
  freshRegs();
  derivedReg.register("of-page", {
    accepts: "page",
    cascade: { default: Stub, modes: [] },
  });
  derivedReg.register("of-tag", {
    accepts: "tag",
    cascade: { default: Stub, modes: [] },
  });
  derivedReg.register("of-query", {
    accepts: "query",
    cascade: { default: Stub, modes: [] },
  });
  assert.equal(
    derivedReg.mount("of-page", { kind: "page", path: "x" }).accepts,
    "page",
  );
  assert.equal(
    derivedReg.mount("of-tag", { kind: "tag", value: "x" }).accepts,
    "tag",
  );
  assert.equal(
    derivedReg.mount("of-query", { kind: "query", dsl: "x" }).accepts,
    "query",
  );
});

// ── ambient registry ───────────────────────────────────────────────────────

test("ambient registry: register and lookup", () => {
  freshRegs();
  ambientReg.register("calendar", { cascade: { default: Stub, modes: [] } });
  assert.ok(ambientReg.get("calendar"));
});

test("ambient registry: names() lists registered keys", () => {
  freshRegs();
  ambientReg.register("calendar", { cascade: { default: Stub, modes: [] } });
  ambientReg.register("dashboard", { cascade: { default: Stub, modes: [] } });
  assert.deepEqual(ambientReg.names().sort(), ["calendar", "dashboard"]);
});

// ── cascade picker ─────────────────────────────────────────────────────────

test("pickCascadeMember: picks the most-featured mode that fits", () => {
  const compact = { name: "compact" };
  const wide = { name: "wide" };
  const huge = { name: "huge" };
  const cascade = {
    default: compact,
    modes: [
      { minSize: { cols: 120, rows: 30 }, component: huge },
      { minSize: { cols: 60, rows: 20 }, component: wide },
    ],
  };
  assert.equal(pickCascadeMember(cascade, { cols: 150, rows: 40 }), huge);
  assert.equal(pickCascadeMember(cascade, { cols: 80, rows: 25 }), wide);
  assert.equal(pickCascadeMember(cascade, { cols: 40, rows: 15 }), compact);
});

test("pickCascadeMember: a single dimension shortfall drops to next mode", () => {
  const compact = { name: "compact" };
  const wide = { name: "wide" };
  const cascade = {
    default: compact,
    modes: [{ minSize: { cols: 100, rows: 30 }, component: wide }],
  };
  // cols fits, rows doesn't → fall through to default.
  assert.equal(pickCascadeMember(cascade, { cols: 120, rows: 20 }), compact);
});

test("pickCascadeMember: empty modes always returns default", () => {
  const compact = { name: "compact" };
  const cascade = { default: compact, modes: [] };
  assert.equal(pickCascadeMember(cascade, { cols: 200, rows: 60 }), compact);
});

test("pickCascadeMember: exact threshold match counts as fits", () => {
  const compact = { name: "compact" };
  const wide = { name: "wide" };
  const cascade = {
    default: compact,
    modes: [{ minSize: { cols: 80, rows: 24 }, component: wide }],
  };
  assert.equal(pickCascadeMember(cascade, { cols: 80, rows: 24 }), wide);
});

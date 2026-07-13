import { test } from "node:test";
import assert from "node:assert/strict";
import * as textDelta from "../../src/lib/loro/text-delta.ts";

const { deltaToChanges } = textDelta;

// The coordinate contract under test: CM6 interprets every from/to in a
// multi-change dispatch relative to the ORIGINAL (pre-transaction) document,
// so the mapper must advance its position counter on retain AND delete (both
// consume original-doc length) and NOT on insert (which consumes none). The
// pre-fix BlockEditor mapping inverted insert/delete and misapplied every
// multi-run delta (remote Alt-Enter tag demotes, coalesced same-frame
// splices, offline catch-up merges).

/** Reference applier: CM6 multi-change semantics — all ranges address the
 *  original doc and equal-position inserts retain their declaration order. */
function applyChanges(doc, changes) {
  let cursor = 0;
  let out = "";
  for (const c of changes) {
    out += doc.slice(cursor, c.from) + c.insert;
    cursor = c.to;
  }
  return out + doc.slice(cursor);
}

/** Reference quill-delta applier (sequential, in mutated coordinates). */
function applyDelta(doc, delta) {
  let pos = 0;
  let out = doc;
  for (const op of delta) {
    if (typeof op.retain === "number") pos += op.retain;
    else if (typeof op.insert === "string") {
      out = out.slice(0, pos) + op.insert + out.slice(pos);
      pos += op.insert.length;
    } else if (typeof op.delete === "number") {
      out = out.slice(0, pos) + out.slice(pos + op.delete);
    }
  }
  return out;
}

test("single insert run", () => {
  assert.deepEqual(deltaToChanges([{ retain: 3 }, { insert: "xy" }]), [
    { from: 3, to: 3, insert: "xy" },
  ]);
});

test("single delete run", () => {
  assert.deepEqual(deltaToChanges([{ retain: 2 }, { delete: 4 }]), [
    { from: 2, to: 6, insert: "" },
  ]);
});

test("delete advances the original-doc position (audit worked example)", () => {
  // [retain 2, insert "ab", retain 3, delete 1]: the delete's original-doc
  // span is 5..6 — the pre-fix code emitted 7..8 (advanced on the insert,
  // not on the delete).
  const changes = deltaToChanges([
    { retain: 2 },
    { insert: "ab" },
    { retain: 3 },
    { delete: 1 },
  ]);
  assert.deepEqual(changes, [
    { from: 2, to: 2, insert: "ab" },
    { from: 5, to: 6, insert: "" },
  ]);
});

test("Alt-Enter tag-demote delta (delete then later insert)", () => {
  // "see #foo bar" → "see bar #foo": delete 4..9, insert " #foo" at 12.
  // The pre-fix mapping emitted the insert at 7 — INSIDE the delete range.
  const doc = "see #foo bar";
  const delta = [{ retain: 4 }, { delete: 5 }, { retain: 3 }, { insert: " #foo" }];
  const changes = deltaToChanges(delta);
  assert.deepEqual(changes, [
    { from: 4, to: 9, insert: "" },
    { from: 12, to: 12, insert: " #foo" },
  ]);
  assert.equal(applyChanges(doc, changes), "see bar #foo");
  assert.equal(applyChanges(doc, changes), applyDelta(doc, delta));
});

test("multi-run deltas converge with the sequential quill application", () => {
  const doc = "the quick brown fox jumps";
  const deltas = [
    [{ insert: "A" }, { retain: 3 }, { delete: 1 }, { retain: 5 }, { insert: "zz" }],
    [{ delete: 4 }, { insert: "THE " }, { retain: 6 }, { delete: 6 }],
    [{ retain: 10 }, { insert: "🦊" }, { retain: 4 }, { delete: 3 }, { insert: "x" }],
    [{ delete: 25 }, { insert: "replaced" }],
  ];
  for (const delta of deltas) {
    const changes = deltaToChanges(delta);
    assert.equal(
      applyChanges(doc, changes),
      applyDelta(doc, delta),
      `diverged for ${JSON.stringify(delta)}`,
    );
  }
});

test("retain-only / empty deltas produce no changes", () => {
  assert.deepEqual(deltaToChanges([]), []);
  assert.deepEqual(deltaToChanges([{ retain: 7 }]), []);
});

function planTextReconciliation(currentText, eventDeltas, canonicalText) {
  return textDelta.planTextReconciliation?.(currentText, eventDeltas, canonicalText);
}

function applyReconciliationPlan(currentText, plan) {
  if (plan?.kind === "canonical") return plan.text;
  if (plan?.kind === "unchanged") return currentText;
  if (plan?.kind === "incremental") {
    return plan.events.reduce((doc, changes) => applyChanges(doc, changes), currentText);
  }
  return undefined;
}

test("a lockstep remote event keeps the incremental path and reaches canonical text", () => {
  const plan = planTextReconciliation(
    "abc",
    [[{ retain: 1 }, { insert: "X" }]],
    "aXbc",
  );

  assert.deepEqual(plan, {
    kind: "incremental",
    events: [[{ from: 1, to: 1, insert: "X" }]],
    text: "aXbc",
  });
  assert.equal(applyReconciliationPlan("abc", plan), "aXbc");
});

test("a view one event behind repairs to canonical when the next merge delta is insufficient", () => {
  // The bound LoroText already contained the missing `x` before this event.
  // Applying only the new `Y` event to the stale view is coordinate-valid but
  // projects `abYc`, not the container's canonical `abYxc`.
  const plan = planTextReconciliation(
    "abc",
    [[{ retain: 2 }, { insert: "Y" }]],
    "abYxc",
  );

  assert.deepEqual(plan, { kind: "canonical", text: "abYxc" });
  assert.equal(applyReconciliationPlan("abc", plan), "abYxc");
});

test("equal-length view drift still repairs instead of accepting a valid-looking delta", () => {
  const plan = planTextReconciliation(
    "abX",
    [[{ retain: 3 }, { insert: "!" }]],
    "abc!",
  );

  assert.deepEqual(plan, { kind: "canonical", text: "abc!" });
  assert.equal(applyReconciliationPlan("abX", plan), "abc!");
});

test("an out-of-range delta repairs to canonical instead of clamping coordinates", () => {
  const plan = planTextReconciliation(
    "abc",
    [[{ retain: 4 }, { insert: "!" }]],
    "abc!",
  );

  assert.deepEqual(plan, { kind: "canonical", text: "abc!" });
  assert.equal(applyReconciliationPlan("abc", plan), "abc!");
});

test("subscription attach with no event reconciles a stale view immediately", () => {
  const plan = planTextReconciliation("stale", [], "canonical");

  assert.deepEqual(plan, { kind: "canonical", text: "canonical" });
  assert.equal(applyReconciliationPlan("stale", plan), "canonical");
});

test("multiple event deltas are validated and projected sequentially", () => {
  const plan = planTextReconciliation(
    "abcd",
    [
      [{ retain: 1 }, { delete: 1 }, { insert: "XY" }],
      [{ retain: 3 }, { insert: "!" }],
    ],
    "aXY!cd",
  );

  assert.deepEqual(plan, {
    kind: "incremental",
    events: [
      [
        { from: 1, to: 2, insert: "" },
        { from: 2, to: 2, insert: "XY" },
      ],
      [{ from: 3, to: 3, insert: "!" }],
    ],
    text: "aXY!cd",
  });
  assert.equal(applyReconciliationPlan("abcd", plan), "aXY!cd");
});

test("same-position inserts preserve delta order", () => {
  const plan = planTextReconciliation(
    "ab",
    [[{ retain: 1 }, { insert: "X" }, { insert: "Y" }]],
    "aXYb",
  );

  assert.equal(plan?.kind, "incremental");
  assert.equal(applyReconciliationPlan("ab", plan), "aXYb");
});

test("UTF-16 coordinates can replace a surrogate-pair character", () => {
  const plan = planTextReconciliation(
    "A🦊B",
    [[{ retain: 1 }, { delete: 2 }, { insert: "🐱" }]],
    "A🐱B",
  );

  assert.deepEqual(plan, {
    kind: "incremental",
    events: [[
      { from: 1, to: 3, insert: "" },
      { from: 3, to: 3, insert: "🐱" },
    ]],
    text: "A🐱B",
  });
  assert.equal(applyReconciliationPlan("A🦊B", plan), "A🐱B");
});

test("a malformed op with multiple active shapes takes the canonical path", () => {
  const plan = planTextReconciliation(
    "abc",
    [[{ retain: 1, insert: "X" }]],
    "aXbc",
  );

  assert.deepEqual(plan, { kind: "canonical", text: "aXbc" });
});

function createBindingOwner() {
  return textDelta.createTextBindingGenerationOwner?.();
}

function bindingIdentity(overrides = {}) {
  return {
    view: overrides.view ?? {},
    container: overrides.container ?? {},
    noteSlug: overrides.noteSlug ?? "2026-07-12",
    bid: overrides.bid ?? "11111111-1111-4111-8111-111111111111",
  };
}

test("the current text-binding generation accepts its exact identity", () => {
  const owner = createBindingOwner();
  const identity = bindingIdentity();
  const lease = owner?.claim(identity);

  assert.equal(lease?.owns(identity), true);
});

test("cleanup revokes an old text-binding callback even if unsubscribe later fails", () => {
  const owner = createBindingOwner();
  const identity = bindingIdentity();
  const lease = owner?.claim(identity);

  lease?.revoke();

  assert.equal(lease?.owns(identity), false);
});

test("a newer text-binding claim supersedes an older generation", () => {
  const owner = createBindingOwner();
  const oldIdentity = bindingIdentity();
  const newIdentity = bindingIdentity({ view: {}, container: {}, noteSlug: "2026-07-13" });
  const oldLease = owner?.claim(oldIdentity);
  const newLease = owner?.claim(newIdentity);

  assert.equal(oldLease?.owns(oldIdentity), false);
  assert.equal(newLease?.owns(newIdentity), true);
});

test("a text-binding lease rejects view, container, slug, and bid mismatches", () => {
  const owner = createBindingOwner();
  const identity = bindingIdentity();
  const lease = owner?.claim(identity);

  assert.equal(lease?.owns({ ...identity, view: {} }), false);
  assert.equal(lease?.owns({ ...identity, container: {} }), false);
  assert.equal(lease?.owns({ ...identity, noteSlug: "2026-07-13" }), false);
  assert.equal(lease?.owns({ ...identity, bid: "22222222-2222-4222-8222-222222222222" }), false);
});

function publishCanonicalTextIfChanged(mirrorText, canonicalText, publish) {
  return textDelta.publishCanonicalTextIfChanged?.(mirrorText, canonicalText, publish);
}

test("an equal parent mirror publishes no canonical text", () => {
  const published = [];

  const changed = publishCanonicalTextIfChanged("same", "same", (text) => published.push(text));

  assert.equal(changed, false);
  assert.deepEqual(published, []);
});

test("a stale parent mirror publishes canonical text exactly once", () => {
  const published = [];

  const changed = publishCanonicalTextIfChanged("stale", "canonical", (text) => published.push(text));

  assert.equal(changed, true);
  assert.deepEqual(published, ["canonical"]);
});

test("publishing canonical text reaches a fixed point on the next comparison", () => {
  let mirrorText = "stale";
  let publishCount = 0;
  const publish = (text) => {
    mirrorText = text;
    publishCount += 1;
  };

  assert.equal(publishCanonicalTextIfChanged(mirrorText, "canonical", publish), true);
  assert.equal(publishCanonicalTextIfChanged(mirrorText, "canonical", publish), false);
  assert.equal(publishCount, 1);
});

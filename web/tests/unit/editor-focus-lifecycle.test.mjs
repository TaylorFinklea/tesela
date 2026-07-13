import assert from "node:assert/strict";
import test from "node:test";

async function loadFactory() {
  const mod = await import("../../src/lib/editor/focus-lifecycle.ts").catch(() => ({}));
  assert.equal(
    typeof mod.createDeferredEditorLifecycle,
    "function",
    "expected createDeferredEditorLifecycle to exist",
  );
  return mod.createDeferredEditorLifecycle;
}

async function loadOwnerFactory() {
  const mod = await import("../../src/lib/editor/focus-lifecycle.ts");
  assert.equal(
    typeof mod.createEditorFocusOwnerId,
    "function",
    "expected createEditorFocusOwnerId to exist",
  );
  return mod.createEditorFocusOwnerId;
}

function manualQueue() {
  const tasks = [];
  return {
    queue: (task) => tasks.push(task),
    flush() {
      while (tasks.length > 0) tasks.shift()();
    },
  };
}

function target(name) {
  return { name, connected: true, focused: false };
}

async function harness(initialTarget) {
  const createDeferredEditorLifecycle = await loadFactory();
  const pending = manualQueue();
  const events = [];
  let current = initialTarget;
  const lifecycle = createDeferredEditorLifecycle({
    queue: pending.queue,
    isCurrent: (candidate) => current === candidate && candidate.connected,
    isFocused: (candidate) => candidate.focused,
    clearOwnership: (candidate) => events.push(`clear:${candidate.name}`),
    applyFocus: (candidate) => events.push(`focus:${candidate.name}`),
    applyBlur: (candidate) => events.push(`blur:${candidate.name}`),
  });
  return {
    lifecycle,
    pending,
    events,
    setCurrent: (next) => { current = next; },
  };
}

test("focus and blur callbacks never run in the current stack", async () => {
  const editor = target("a");
  const h = await harness(editor);

  editor.focused = true;
  h.lifecycle.focus(editor);
  assert.deepEqual(h.events, []);
  h.pending.flush();
  assert.deepEqual(h.events, ["focus:a"]);

  editor.focused = false;
  h.lifecycle.blur(editor);
  assert.deepEqual(h.events, ["focus:a"]);
  h.pending.flush();
  assert.deepEqual(h.events, ["focus:a", "clear:a", "blur:a"]);
});

test("a queued blur is superseded by refocus before the microtask runs", async () => {
  const editor = target("a");
  const h = await harness(editor);

  h.lifecycle.blur(editor);
  editor.focused = true;
  h.lifecycle.focus(editor);
  h.pending.flush();

  assert.deepEqual(h.events, ["focus:a"]);
});

test("a queued focus is superseded by blur before the microtask runs", async () => {
  const editor = target("a");
  const h = await harness(editor);

  editor.focused = true;
  h.lifecycle.focus(editor);
  editor.focused = false;
  h.lifecycle.blur(editor);
  h.pending.flush();

  assert.deepEqual(h.events, ["clear:a", "blur:a"]);
});

test("teardown invalidates pending focus and defers id-guarded ownership clearing", async () => {
  const editor = target("a");
  const h = await harness(editor);

  editor.focused = true;
  h.lifecycle.focus(editor);
  h.lifecycle.teardown(editor);
  assert.deepEqual(h.events, []);
  h.pending.flush();

  assert.deepEqual(h.events, ["clear:a"]);
});

test("teardown invalidates pending blur and clears ownership exactly once", async () => {
  const editor = target("a");
  const h = await harness(editor);

  h.lifecycle.blur(editor);
  h.lifecycle.teardown(editor);
  assert.deepEqual(h.events, []);
  h.pending.flush();

  assert.deepEqual(h.events, ["clear:a"]);
});

test("stale queued work cannot act on a rebound or destroyed view", async () => {
  const oldEditor = target("old");
  const newEditor = target("new");
  const h = await harness(oldEditor);

  oldEditor.focused = true;
  h.lifecycle.focus(oldEditor);
  h.setCurrent(newEditor);
  h.pending.flush();
  assert.deepEqual(h.events, []);

  h.setCurrent(oldEditor);
  oldEditor.focused = false;
  h.lifecycle.blur(oldEditor);
  oldEditor.connected = false;
  h.pending.flush();
  assert.deepEqual(h.events, ["clear:old"]);
});

test("same-key remount teardown cannot clear the newer focus owner", async () => {
  const createDeferredEditorLifecycle = await loadFactory();
  const createEditorFocusOwnerId = await loadOwnerFactory();
  const oldQueue = manualQueue();
  const newQueue = manualQueue();
  const stableEditorKey = "bid:same";
  const oldTarget = {
    ownerId: createEditorFocusOwnerId(stableEditorKey),
    connected: true,
    focused: true,
  };
  const newTarget = {
    ownerId: createEditorFocusOwnerId(stableEditorKey),
    connected: true,
    focused: true,
  };
  let focusedOwner = null;

  function lifecycleFor(target, pending) {
    return createDeferredEditorLifecycle({
      queue: pending.queue,
      isCurrent: (candidate) => candidate === target && candidate.connected,
      isFocused: (candidate) => candidate.focused,
      clearOwnership: (candidate) => {
        if (focusedOwner === candidate.ownerId) focusedOwner = null;
      },
      applyFocus: (candidate) => { focusedOwner = candidate.ownerId; },
      applyBlur: () => {},
    });
  }

  const oldLifecycle = lifecycleFor(oldTarget, oldQueue);
  const newLifecycle = lifecycleFor(newTarget, newQueue);
  oldLifecycle.focus(oldTarget);
  oldQueue.flush();
  newLifecycle.focus(newTarget);
  newQueue.flush();
  oldLifecycle.teardown(oldTarget);
  oldQueue.flush();

  assert.notEqual(oldTarget.ownerId, newTarget.ownerId);
  assert.equal(focusedOwner, newTarget.ownerId);
});

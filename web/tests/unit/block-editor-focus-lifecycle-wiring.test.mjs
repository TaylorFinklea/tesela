import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(
  new URL("../../src/lib/components/BlockEditor.svelte", import.meta.url),
  "utf8",
);

function sourceBetween(startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start);
  assert.notEqual(start, -1, `expected ${startMarker}`);
  assert.notEqual(end, -1, `expected ${endMarker}`);
  return source.slice(start, end);
}

test("CodeMirror focus lifecycle captures one mounted view and defers Svelte mutations", () => {
  assert.match(
    source,
    /import \{ createDeferredEditorLifecycle, createEditorFocusOwnerId \} from "\$lib\/editor\/focus-lifecycle";/,
  );

  const lifecycle = sourceBetween(
    "const focusOwnerId = createEditorFocusOwnerId",
    "const focusBlurHandler",
  );
  assert.match(lifecycle, /editorKey,[\s\S]*focusOwnerId,[\s\S]*noteSlug,/);
  assert.match(lifecycle, /vimContext: captureVimContext\(eventView\)/);
  assert.match(lifecycle, /onFocus,[\s\S]*onBlur,[\s\S]*onSetProperty,[\s\S]*detectConfig/);
  assert.match(lifecycle, /slashMenuOpen: showSlashMenu/);
  assert.match(lifecycle, /autocompleteOpen: showAutocomplete/);
  assert.match(lifecycle, /queue: queueMicrotask/);
  assert.match(
    lifecycle,
    /isCurrent: \(target\) => view === target\.view && target\.view\.dom\.isConnected/,
  );
  assert.match(lifecycle, /isFocused: \(target\) => target\.view\.hasFocus/);
  assert.match(lifecycle, /setFocusedEditor\(target\.focusOwnerId\)/);
  assert.match(lifecycle, /setFocusedNoteDoc\(target\.focusOwnerId, target\.noteSlug\)/);
  assert.match(lifecycle, /clearFocusedEditor\(target\.focusOwnerId\)/);
  assert.match(lifecycle, /clearFocusedNoteDoc\(target\.focusOwnerId\)/);
  assert.doesNotMatch(lifecycle, /setFocusedEditor\(target\.editorKey\)/);
  assert.doesNotMatch(lifecycle, /clearFocusedEditor\(target\.editorKey\)/);
});

test("DOM handlers only update vim context synchronously and queue owned work", () => {
  const handlers = sourceBetween("const focusBlurHandler", "paste: (e) =>");

  assert.match(
    handlers,
    /focus: \(_e, eventView\) => \{[\s\S]*const target = captureFocusTarget\(eventView\);[\s\S]*wireVimCtx\(target\.vimContext\);[\s\S]*focusLifecycle\.focus\(target\)/,
  );
  assert.match(
    handlers,
    /blur: \(_e, eventView\) => \{[\s\S]*const target = captureFocusTarget\(eventView\);[\s\S]*clearVimCtxIfMine\(target\.view\);[\s\S]*focusLifecycle\.blur\(target\)/,
  );
  assert.doesNotMatch(handlers, /(?:set|clear)Focused(?:Editor|NoteDoc)\(/);
  assert.doesNotMatch(handlers, /onFocus|onBlur|onSetProperty/);
});

test("delayed blur uses captured props and teardown invalidates before destroy", () => {
  const lifecycle = sourceBetween(
    "const focusOwnerId = createEditorFocusOwnerId",
    "const focusBlurHandler",
  );
  const cleanup = sourceBetween("return () => {\n      if (presenceTimer)", "// Leader → editor bridge");

  assert.match(
    lifecycle,
    /target\.onSetProperty && target\.detectConfig\s*&& !target\.slashMenuOpen && !target\.autocompleteOpen/,
  );
  assert.match(lifecycle, /const doc = target\.view\.state\.doc\.toString\(\)/);
  assert.match(lifecycle, /target\.view\.dispatch\(/);
  assert.match(lifecycle, /target\.onSetProperty\(/);
  assert.match(lifecycle, /if \(!target\.slashMenuOpen\) target\.onBlur\(\)/);
  assert.match(
    cleanup,
    /focusLifecycle\.teardown\(target\);[\s\S]*clearVimCtxIfMine\(mountedView\);[\s\S]*mountedView\.destroy\(\)/,
  );
  assert.doesNotMatch(cleanup, /clearFocusedEditor\(editorKey\)/);
  assert.doesNotMatch(cleanup, /clearFocusedNoteDoc\(editorKey\)/);
  assert.doesNotMatch(cleanup, /view = null/);
});

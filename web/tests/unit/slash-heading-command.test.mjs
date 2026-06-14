import assert from "node:assert/strict";
import test from "node:test";

import { commandRegistry } from "../../src/lib/command-registry.svelte.ts";
import "../../src/lib/editor/commands/heading.ts";

test("editor.heading matches the legacy slash heading transform byte-for-byte", async () => {
  const before = "  Hello world  ";
  const after = "\n  child";
  const legacyInsert = "# " + before.trim() + after;
  const legacyCaret = legacyInsert.length - after.length;

  const cmd = commandRegistry.get("editor.heading");
  assert.ok(cmd, "expected editor.heading to register");

  let replaceCall = null;
  let finished = null;
  await cmd.run(undefined, {
    editor: {
      block: { id: "block-1", bid: null, properties: {} },
      before,
      after,
      propertyDefs: [],
      statusChoices: ["todo", "doing", "done"],
      autoFillNames: () => [],
      replaceTrigger: (insert, caretFromEnd) => { replaceCall = { insert, caretFromEnd }; },
      setProperty: () => {},
      addTag: () => {},
      insertTemplate: () => {},
      openDatePicker: () => {},
      openTagPicker: () => {},
      openTemplatePicker: () => {},
      openPropertyValue: () => {},
      moveCursor: () => {},
      finish: (verb) => { finished = verb; },
    },
  });

  assert.deepEqual(replaceCall, { insert: legacyInsert, caretFromEnd: after.length });
  assert.equal(replaceCall.insert.length - replaceCall.caretFromEnd, legacyCaret);
  assert.equal(finished, "heading");
});

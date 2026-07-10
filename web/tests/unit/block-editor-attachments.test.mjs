import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const editor = readFileSync(new URL("../../src/lib/components/BlockEditor.svelte", import.meta.url), "utf8");
const apiClient = readFileSync(new URL("../../src/lib/api-client.ts", import.meta.url), "utf8");

test("uploadImage posts the image file to attachments and returns the server path", () => {
  assert.match(apiClient, /async function uploadImage\(file: File\)/);
  assert.match(apiClient, /method:\s*["']POST["']/);
  assert.match(apiClient, /\/attachments\?filename=\$\{encodeURIComponent\(file\.name\)\}/);
  assert.match(apiClient, /body:\s*file/);
});

test("BlockEditor uploads pasted and dropped image files at the caret", () => {
  assert.match(editor, /api\.uploadImage\(/);
  assert.match(editor, /clipboardData\?\.files/);
  assert.match(editor, /dataTransfer\?\.files/);
  assert.match(editor, /file\.type\.startsWith\(["']image\//);
  assert.match(editor, /!\[\]\(\$\{uploaded\.path\}\)/);
  assert.match(editor, /preventDefault\(\)/);
});

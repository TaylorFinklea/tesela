import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const devices = readFileSync(
  new URL("../../src/routes/settings/devices/+page.svelte", import.meta.url),
  "utf8",
);
const apiClient = readFileSync(new URL("../../src/lib/api-client.ts", import.meta.url), "utf8");

test("pairing guidance has the existing mosaic issue the code to the joining device", () => {
  assert.match(devices, /On the device that already has the mosaic you want to join/);
  assert.match(devices, /paste it on this joining device/);
  assert.doesNotMatch(devices, /On the device you want to bring in/);
});

test("pairing success tells the joiner to restart when the relay requires it", () => {
  assert.match(apiClient, /restart_required:\s*boolean/);
  assert.match(devices, /r\.restart_required/);
  assert.match(devices, /Quit and reopen Tesela/);
});

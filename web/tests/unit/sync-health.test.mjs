import { test } from "node:test";
import assert from "node:assert/strict";

import {
  blendSyncStatus,
  formatRelaySuccessAge,
} from "../../src/lib/sync-health.ts";

function relay(overrides = {}) {
  return {
    configured: true,
    last_poll_at: 995,
    last_put_at: null,
    last_error: null,
    ...overrides,
  };
}

test("blendSyncStatus returns red whenever the WebSocket is down", () => {
  assert.equal(
    blendSyncStatus(false, relay({ last_error: "relay unavailable" }), 1_000_000),
    "red",
  );
});

test("blendSyncStatus returns green for a configured relay with recent success", () => {
  assert.equal(blendSyncStatus(true, relay(), 1_000_000), "green");
});

test("blendSyncStatus returns amber when relay status reports an error", () => {
  assert.equal(
    blendSyncStatus(true, relay({ last_error: "cursor rejected" }), 1_000_000),
    "amber",
  );
});

test("blendSyncStatus returns amber when relay success is stale or missing", () => {
  assert.equal(
    blendSyncStatus(true, relay({ last_poll_at: 700 }), 1_000_000),
    "amber",
  );
  assert.equal(
    blendSyncStatus(true, relay({ last_poll_at: null, last_put_at: null }), 1_000_000),
    "amber",
  );
  assert.equal(
    blendSyncStatus(true, relay({ configured: false }), 1_000_000),
    "amber",
  );
});

test("blendSyncStatus returns amber when the status request itself failed", () => {
  assert.equal(
    blendSyncStatus(true, relay(), 1_000_000, "server unavailable"),
    "amber",
  );
});

test("formatRelaySuccessAge reports the newest successful relay operation", () => {
  assert.equal(
    formatRelaySuccessAge(relay({ last_poll_at: 980, last_put_at: 995 }), 1_000_000),
    "5s ago",
  );
  assert.equal(
    formatRelaySuccessAge(relay({ last_poll_at: null, last_put_at: null }), 1_000_000),
    "never",
  );
});

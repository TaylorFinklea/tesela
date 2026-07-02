/**
 * Server-sync glue for the keybinding + leader-tree user config
 * (tesela-cmdd.4). Kept OUT of `keybindings.svelte.ts` on purpose — that
 * store is imported directly by `node --test` unit tests, and
 * `api-client.ts` is "not node-importable" (its own comment: `ApiError`
 * uses TS parameter properties + it value-imports `ws-refresh-coordinator`).
 * This module is the only place the two are wired together, called once
 * from `+layout.svelte`'s `onMount` (mirrors `ensureSystemWidgets`).
 */
import { api } from "../api-client.ts";
import * as keybindings from "./keybindings.svelte.ts";

let pushTimer: ReturnType<typeof setTimeout> | undefined;

/** Fire-and-forget PUT of the current whole config, debounced so a burst of
 *  rebinds (or `resetAll`) collapses into one request. */
function schedulePush(): void {
  if (pushTimer) clearTimeout(pushTimer);
  pushTimer = setTimeout(() => {
    void api.putKeymapConfig(keybindings.wholeConfig()).catch((e) => {
      console.warn("keymap-sync: PUT /keymap-config failed", e);
    });
  }, 300);
}

/** Fetch the server's config and hydrate the local store with it (server is
 *  authoritative — this is how a rebind made on device A shows up on device
 *  B's next load). Also registers the push hook so subsequent local edits
 *  sync back. Call once at app bootstrap; tolerates a server that isn't
 *  reachable yet (keeps the localStorage-cached config). */
export async function initKeymapConfig(): Promise<void> {
  keybindings.setSyncHook(schedulePush);
  try {
    const config = await api.getKeymapConfig();
    keybindings.hydrate(config);
  } catch (e) {
    console.warn("keymap-sync: GET /keymap-config failed, using local cache", e);
  }
}

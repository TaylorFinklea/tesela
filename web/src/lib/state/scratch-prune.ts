/**
 * Scratch prune sweep — Phase 12 of Prism v5.
 *
 * Walks every note with `type: scratch`, checks `modified_at`, deletes
 * ones older than the configured threshold. Runs once at app boot when
 * `scratchPruneAfterDays` is set; idempotent + cheap to re-run.
 *
 * Pure logic + an exported function for callers to invoke; tests can
 * supply the threshold + a fake clock without mounting any UI.
 */

import { api } from "$lib/api-client";
import type { Note } from "$lib/types/Note";

export type PruneResult = {
  scanned: number;
  pruned: string[];
};

const LAST_SWEEP_KEY = "tesela:scratch-prune:last-sweep-iso";

export { shouldPrune } from "./scratch-prune-pure.ts";
import { shouldPrune } from "./scratch-prune-pure.ts";

/** Run the prune sweep. No-op when `pruneAfterDays` is undefined or ≤ 0
 *  so callers can pass workspace state directly. */
export async function runScratchPrune(
  pruneAfterDays: number | undefined,
  now: Date = new Date(),
): Promise<PruneResult | null> {
  if (!pruneAfterDays || pruneAfterDays <= 0) return null;
  const cutoff = new Date(now.getTime() - pruneAfterDays * 86_400_000);
  const all = (await api.listNotes({ limit: 1000 })) as Note[];
  const targets = all.filter((n) => shouldPrune(n, cutoff));
  const pruned: string[] = [];
  for (const n of targets) {
    try {
      await api.deleteNote(n.id);
      pruned.push(n.id);
    } catch (e) {
      console.warn("scratch prune: failed to delete", n.id, e);
    }
  }
  return { scanned: all.length, pruned };
}

/** Boot-time hook: runs the sweep at most once per local-day. Persists the
 *  last-sweep date in localStorage so a chatty fresh boot doesn't hit the
 *  API twice. */
export async function maybeRunScratchPruneAtBoot(
  pruneAfterDays: number | undefined,
): Promise<PruneResult | null> {
  if (!pruneAfterDays || pruneAfterDays <= 0) return null;
  if (typeof localStorage === "undefined") return runScratchPrune(pruneAfterDays);
  const today = new Date().toISOString().slice(0, 10); // YYYY-MM-DD local-ish
  const last = localStorage.getItem(LAST_SWEEP_KEY);
  if (last === today) return null;
  const result = await runScratchPrune(pruneAfterDays);
  if (result) localStorage.setItem(LAST_SWEEP_KEY, today);
  return result;
}

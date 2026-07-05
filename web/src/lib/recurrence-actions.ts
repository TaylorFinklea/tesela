/**
 * Shared recurrence action helpers used by both the `skip-occurrence` command
 * (commands/index.ts) and the recurring chip's click menu.
 */

import { api } from "$lib/api-client";
import { getAppQueryClient } from "$lib/app-query-client.svelte";
import { toast } from "$lib/stores/toast.svelte";

/** Skips the given block to its next occurrence.
 *  Invalidates the `["notes"]` query and shows an appropriate toast. */
export async function skipRecurrence(blockId: string): Promise<void> {
  const res = await api.recurBump(blockId, "skip");
  if (res.bumped) {
    const qc = getAppQueryClient();
    if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
    toast("Skipped to next occurrence", "success");
  } else {
    toast("Nothing to skip", "info");
  }
}

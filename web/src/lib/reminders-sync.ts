/**
 * Shared "sync now" handler used by the Cmd+K palette command and the
 * Settings page button. Reports outcome via the toast system so both
 * surfaces give consistent feedback.
 *
 * The button hits `/sync/reminders` (pull-then-push) so external edits
 * in Reminders.app aren't clobbered by an immediate push.
 */

import { api } from "$lib/api-client";
import { toast } from "$lib/stores/toast.svelte";
import type { QueryClient } from "@tanstack/svelte-query";

export async function runRemindersSync(queryClient: QueryClient): Promise<void> {
  toast("Syncing Apple Reminders…", "info", 0);
  try {
    const outcome = await api.remindersSync();
    const pulled = outcome.pull.updated.length;
    const created = outcome.push.created.length;
    const updated = outcome.push.updated.length;
    const errors = outcome.pull.errors.length + outcome.push.errors.length;
    const orphans = outcome.pull.orphans.length;

    if (pulled === 0 && created === 0 && updated === 0) {
      toast(
        errors > 0
          ? `Sync finished with ${errors} error${errors === 1 ? "" : "s"}`
          : "Already up to date",
        errors > 0 ? "warn" : "info",
      );
    } else {
      const parts: string[] = [];
      if (created > 0) parts.push(`${created} new`);
      if (updated > 0) parts.push(`${updated} updated`);
      if (pulled > 0) parts.push(`${pulled} pulled from Reminders`);
      if (orphans > 0) parts.push(`${orphans} orphan${orphans === 1 ? "" : "s"}`);
      const msg = `Sync done: ${parts.join(", ")}`;
      toast(errors > 0 ? `${msg} · ${errors} error${errors === 1 ? "" : "s"}` : msg,
            errors > 0 ? "warn" : "success");
    }

    // Pull may have rewritten note bodies; invalidate everything that
    // could be stale. Cheap on the typical mosaic.
    queryClient.invalidateQueries({ queryKey: ["notes"] });
    queryClient.invalidateQueries({ queryKey: ["typed-blocks"] });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    // Reminders sync is macOS-only; surface that explicitly so the user
    // doesn't think it's a bug when running from a Linux dev box.
    if (msg.includes("only available on macOS")) {
      toast("Apple Reminders sync requires macOS", "warn");
    } else {
      toast(`Sync failed: ${msg}`, "error", 6000);
    }
  }
}

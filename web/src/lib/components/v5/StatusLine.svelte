<script lang="ts">
  /*
   * Prism v5 — status line.
   *
   * Vim/Zellij-shaped: focused buffer kind+name on the left, binding
   * indicator chips in the middle, shortcut hints on the right.
   * The binding indicator is a `$derived` over the active tab's pane
   * tree + focus state. No separate registry — the bindings *are* the
   * pane tree contents.
   */
  import {
    getActiveTab,
    getFocusedBuffer,
    getFocusedLeafId,
    getLastFocusedPageId,
  } from "$lib/buffer/state.svelte";
  import { leaves } from "$lib/buffer/tree";
  import type { Reference } from "$lib/buffer/types";

  type BindingIndicator =
    | { kind: "page-has-followers"; count: number }
    | { kind: "derived-following"; resolvedPagePath: string | null }
    | { kind: "derived-pinned"; reference: Reference }
    | null;

  const tab = $derived(getActiveTab());
  const buffer = $derived(getFocusedBuffer());
  const focusedLeafId = $derived(getFocusedLeafId());
  const lastPageId = $derived(getLastFocusedPageId());

  const indicator: BindingIndicator = $derived.by(() => {
    if (!tab || !buffer) return null;
    if (buffer.kind === "page") {
      // Count derived buffers in this tab that follow (any focus follower).
      let followers = 0;
      for (const l of leaves(tab.layout)) {
        if (l.buffer.kind === "derived" && l.buffer.binding.mode === "follow") {
          followers++;
        }
      }
      return followers > 0
        ? { kind: "page-has-followers", count: followers }
        : null;
    }
    if (buffer.kind === "derived") {
      if (buffer.binding.mode === "follow") {
        return {
          kind: "derived-following",
          resolvedPagePath: lastPageId ?? null,
        };
      }
      return { kind: "derived-pinned", reference: buffer.binding.reference };
    }
    // Ambient buffers don't get a chip — the kind+name display already
    // says everything; an "ambient · calendar" chip would just echo it.
    return null;
  });

  const namePart = $derived.by(() => {
    if (!buffer) return "—";
    if (buffer.kind === "page") return buffer.pageId || "empty";
    if (buffer.kind === "derived") return buffer.rendererName;
    return buffer.ambientName;
  });
</script>

<footer class="v5-statusline">
  <span class="mode">● NORMAL</span>
  <span class="center">
    <span>tab: {tab?.name ?? "—"}</span>
    <span class="sep">·</span>
    <span>{buffer?.kind ?? "—"}</span>
    <span class="sep">·</span>
    <span class="name">{namePart}</span>
    {#if indicator}
      <span class="sep">·</span>
      {#if indicator.kind === "page-has-followers"}
        <span class="chip">↪ {indicator.count} follower{indicator.count === 1 ? "" : "s"}</span>
      {:else if indicator.kind === "derived-following"}
        <span class="chip"
          >following · {indicator.resolvedPagePath ?? "—"}</span
        >
      {:else if indicator.kind === "derived-pinned"}
        <span class="chip"
          >📌 {indicator.reference.kind} ·
          {indicator.reference.kind === "page"
            ? indicator.reference.path
            : indicator.reference.kind === "tag"
              ? indicator.reference.value
              : indicator.reference.dsl}</span
        >
      {/if}
    {/if}
  </span>
  <span class="right">
    <span><b>⌘K</b> station</span>
    <span><b>:</b> ex</span>
    <span><b>⌘I</b> peek</span>
    <span><b>⌘G</b> graph</span>
    <span><b>⌘B</b> sidebar</span>
    <span><b>⌃W hjkl</b> move</span>
  </span>
</footer>

<style>
  .v5-statusline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 0 14px;
    border-top: 1px solid var(--v4-hair);
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink4);
    background: var(--v4-bg);
  }
  .mode {
    color: var(--v4-accent);
    flex-shrink: 0;
  }
  .center {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
  }
  .sep {
    color: var(--v4-ink6);
  }
  .name {
    color: var(--v4-ink2);
  }
  .chip {
    padding: 1px 6px;
    border: 1px solid var(--v4-hair);
    border-radius: 4px;
    color: var(--v4-ink2);
  }
  .right {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }
  .right b {
    color: var(--v4-accent);
    font-weight: 400;
  }
</style>

import type { AmbientRenderer } from "$lib/buffer/protocol";
import Component from "./Inbox.svelte";

/**
 * The v5 Inbox ambient — a triage surface listing every block that
 * doesn't yet carry a `status::`. Drives the GTD-style flow of ripping
 * through untriaged captures with single-key actions.
 *
 * Shares the underlying query (`kind:block -has:status`) with the v4
 * Inbox sidebar widget so the two surfaces stay in sync.
 */
const renderer: AmbientRenderer = {
  cascade: { default: Component, modes: [] },
};

export default renderer;

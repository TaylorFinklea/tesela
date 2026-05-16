import type { AmbientRenderer } from "$lib/buffer/protocol";
import Component from "./WorkspaceDashboard.svelte";

const renderer: AmbientRenderer = {
  cascade: { default: Component, modes: [] },
};

export default renderer;

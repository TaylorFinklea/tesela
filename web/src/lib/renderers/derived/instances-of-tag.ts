import type { DerivedRenderer } from "$lib/buffer/protocol";
import Component from "./instances-of-tag.svelte";

const renderer: DerivedRenderer<"tag"> = {
  accepts: "tag",
  cascade: { default: Component, modes: [] },
};

export default renderer;

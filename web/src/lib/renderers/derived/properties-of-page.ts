import type { DerivedRenderer } from "$lib/buffer/protocol";
import Component from "./properties-of-page.svelte";

const renderer: DerivedRenderer<"page"> = {
  accepts: "page",
  cascade: { default: Component, modes: [] },
};

export default renderer;

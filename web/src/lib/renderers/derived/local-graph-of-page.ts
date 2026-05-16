import type { DerivedRenderer } from "$lib/buffer/protocol";
import Full from "./local-graph-of-page.svelte";
import Compact from "./local-graph-of-page-compact.svelte";

const renderer: DerivedRenderer<"page"> = {
  accepts: "page",
  cascade: {
    // Default = the compact fallback. The full force-directed graph
    // member is selected when there's room (≥ 50 cols × 16 rows).
    default: Compact,
    modes: [
      {
        // Force-directed graph needs real estate — anything narrower
        // than ~80 cols × 22 rows reads as noise. Peek (~50×18) drops
        // to the compact chip; a wide derived-buffer split shows the
        // full graph.
        minSize: { cols: 80, rows: 22 },
        component: Full,
        label: "full graph",
      },
    ],
  },
};

export default renderer;

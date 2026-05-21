<script lang="ts">
  /*
   * Force-directed link graph on a <canvas>. Extracted from the
   * `/graph` route so it can be mounted both there and inside a Prism
   * v4 `graph` pane. Pure renderer: takes `notes` + `edges` as props,
   * owns the canvas / simulation / draw loop / hover + resize, and
   * reports node activation through `onNodePick`. Sizes itself to its
   * parent element via ResizeObserver, so the parent just needs a
   * positioned, sized container.
   */
  import { onMount } from "svelte";
  import type { Note } from "$lib/types/Note";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  let {
    notes,
    edges,
    onNodePick,
  }: {
    notes: Note[];
    edges: GraphEdge[];
    onNodePick?: (noteId: string) => void;
  } = $props();

  let canvas: HTMLCanvasElement;
  let width = $state(800);
  let height = $state(600);

  type GraphNode = {
    id: string;
    title: string;
    x: number;
    y: number;
    vx: number;
    vy: number;
    connections: number;
  };

  let nodes = $state<GraphNode[]>([]);
  let graphEdges = $state<Array<{ source: number; target: number }>>([]);
  let hoveredNode = $state<GraphNode | null>(null);
  let animFrame: number;

  // Rebuild the node/edge arrays whenever the input data changes.
  $effect(() => {
    if (notes.length === 0) {
      nodes = [];
      graphEdges = [];
      return;
    }

    const nodeMap = new Map<string, number>();
    const newNodes: GraphNode[] = [];

    for (const note of notes) {
      const idx = newNodes.length;
      nodeMap.set(note.id.toLowerCase(), idx);
      newNodes.push({
        id: note.id,
        title: note.title,
        x: width / 2 + (Math.random() - 0.5) * width * 0.6,
        y: height / 2 + (Math.random() - 0.5) * height * 0.6,
        vx: 0,
        vy: 0,
        connections: 0,
      });
    }

    const newEdges: Array<{ source: number; target: number }> = [];
    for (const edge of edges) {
      const si = nodeMap.get(edge.source.toLowerCase());
      const ti = nodeMap.get(edge.target.toLowerCase());
      if (si !== undefined && ti !== undefined && si !== ti) {
        newEdges.push({ source: si, target: ti });
        newNodes[si].connections++;
        newNodes[ti].connections++;
      }
    }

    nodes = newNodes;
    graphEdges = newEdges;
  });

  function simulate() {
    if (nodes.length === 0) return;

    // Repulsion
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const dx = nodes[j].x - nodes[i].x;
        const dy = nodes[j].y - nodes[i].y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = 800 / (dist * dist);
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        nodes[i].vx -= fx;
        nodes[i].vy -= fy;
        nodes[j].vx += fx;
        nodes[j].vy += fy;
      }
    }

    // Attraction along edges
    for (const edge of graphEdges) {
      const a = nodes[edge.source];
      const b = nodes[edge.target];
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = (dist - 100) * 0.01;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      a.vx += fx;
      a.vy += fy;
      b.vx -= fx;
      b.vy -= fy;
    }

    // Center gravity
    for (const node of nodes) {
      node.vx += (width / 2 - node.x) * 0.001;
      node.vy += (height / 2 - node.y) * 0.001;
    }

    // Apply velocity with damping
    for (const node of nodes) {
      node.vx *= 0.9;
      node.vy *= 0.9;
      node.x += node.vx;
      node.y += node.vy;
      node.x = Math.max(20, Math.min(width - 20, node.x));
      node.y = Math.max(20, Math.min(height - 20, node.y));
    }
  }

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const style = getComputedStyle(document.documentElement);
    const primary = style.getPropertyValue("--primary").trim() || "#fb5950";
    const fg = style.getPropertyValue("--foreground").trim() || "#f4f1de";
    const muted = style.getPropertyValue("--muted-foreground").trim() || "#928f7e";

    ctx.clearRect(0, 0, width, height);

    // Edges + nodes need to stand out from the dark bg. Previous alphas
    // (20 = 12%, 33 = 20%) were near-invisible. Bumped so the graph
    // actually reads at-a-glance.
    ctx.strokeStyle = muted + "80";
    ctx.lineWidth = 1.2;
    for (const edge of graphEdges) {
      const a = nodes[edge.source];
      const b = nodes[edge.target];
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
    }

    for (const node of nodes) {
      const isHovered = hoveredNode === node;
      const r = Math.max(3, Math.min(8, 2 + node.connections * 1.5));

      ctx.beginPath();
      ctx.arc(node.x, node.y, r, 0, Math.PI * 2);
      ctx.fillStyle = isHovered
        ? primary
        : node.connections > 0
          ? fg + "dd"
          : fg + "88";
      ctx.fill();

      if (isHovered || node.connections >= 2) {
        ctx.font = isHovered
          ? "12px 'Source Sans 3', sans-serif"
          : "10px 'Source Sans 3', sans-serif";
        ctx.fillStyle = isHovered ? fg : muted + "cc";
        ctx.fillText(node.title, node.x + r + 4, node.y + 4);
      }
    }

    simulate();
    animFrame = requestAnimationFrame(draw);
  }

  function handleMouseMove(e: MouseEvent) {
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;

    hoveredNode = null;
    for (const node of nodes) {
      const dx = node.x - mx;
      const dy = node.y - my;
      if (dx * dx + dy * dy < 200) {
        hoveredNode = node;
        break;
      }
    }
    canvas.style.cursor = hoveredNode ? "pointer" : "default";
  }

  function handleClick() {
    if (hoveredNode) onNodePick?.(hoveredNode.id);
  }

  onMount(() => {
    const container = canvas.parentElement;
    if (container) {
      width = container.clientWidth;
      height = container.clientHeight;
    }

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        width = entry.contentRect.width;
        height = entry.contentRect.height;
      }
    });
    if (container) observer.observe(container);

    draw();

    return () => {
      cancelAnimationFrame(animFrame);
      observer.disconnect();
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<canvas
  bind:this={canvas}
  {width}
  {height}
  class="v4-graph-canvas"
  onmousemove={handleMouseMove}
  onclick={handleClick}
></canvas>

<style>
  .v4-graph-canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
</style>

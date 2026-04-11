<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));

  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
  }));

  const notes: Note[] = $derived((notesQuery.data ?? []) as Note[]);
  const edges: GraphEdge[] = $derived((edgesQuery.data ?? []) as GraphEdge[]);

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

  $effect(() => {
    if (notes.length === 0) return;

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
      // Bounds
      node.x = Math.max(20, Math.min(width - 20, node.x));
      node.y = Math.max(20, Math.min(height - 20, node.y));
    }
  }

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.clearRect(0, 0, width, height);

    // Edges
    ctx.strokeStyle = "rgba(255, 255, 255, 0.08)";
    ctx.lineWidth = 1;
    for (const edge of graphEdges) {
      const a = nodes[edge.source];
      const b = nodes[edge.target];
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
    }

    // Nodes
    for (const node of nodes) {
      const isHovered = hoveredNode === node;
      const r = Math.max(3, Math.min(8, 2 + node.connections * 1.5));

      ctx.beginPath();
      ctx.arc(node.x, node.y, r, 0, Math.PI * 2);
      ctx.fillStyle = isHovered
        ? "rgba(255, 255, 255, 0.9)"
        : node.connections > 0
          ? "rgba(255, 255, 255, 0.5)"
          : "rgba(255, 255, 255, 0.2)";
      ctx.fill();

      // Label for hovered or connected nodes
      if (isHovered || node.connections >= 2) {
        ctx.font = isHovered ? "12px sans-serif" : "10px sans-serif";
        ctx.fillStyle = isHovered ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.4)";
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
    if (hoveredNode) {
      goto(`/p/${encodeURIComponent(hoveredNode.id)}`);
    }
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

<div class="flex-1 flex flex-col">
  <header class="border-b border-border px-6 py-3 flex items-center justify-between">
    <span class="text-xs text-muted-foreground">Graph View</span>
    <span class="text-xs text-muted-foreground">{notes.length} notes, {edges.length} links</span>
  </header>

  <div class="flex-1 relative">
    <canvas
      bind:this={canvas}
      {width}
      {height}
      class="absolute inset-0"
      onmousemove={handleMouseMove}
      onclick={handleClick}
    ></canvas>
  </div>
</div>

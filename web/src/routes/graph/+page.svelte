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

  const allNotes: Note[] = $derived((notesQuery.data ?? []) as Note[]);
  const allEdges: GraphEdge[] = $derived((edgesQuery.data ?? []) as GraphEdge[]);

  // Filters
  let filterTag = $state("");
  let maxDepth = $state(0); // 0 = show all

  // Available tags for filter
  const availableTags = $derived.by(() => {
    const tagSet = new Set<string>();
    for (const n of allNotes) {
      for (const t of n.metadata.tags) tagSet.add(t);
    }
    return [...tagSet].sort();
  });

  // Filtered notes/edges based on tag + depth
  const { notes, edges } = $derived.by(() => {
    if (!filterTag && maxDepth === 0) return { notes: allNotes, edges: allEdges };

    // Start with notes matching the tag filter
    let matchingIds: Set<string>;
    if (filterTag) {
      matchingIds = new Set(allNotes.filter((n) => n.metadata.tags.includes(filterTag)).map((n) => n.id.toLowerCase()));
    } else {
      matchingIds = new Set(allNotes.map((n) => n.id.toLowerCase()));
    }

    // Expand by depth (BFS from matching nodes along edges)
    if (maxDepth > 0 && filterTag) {
      let frontier = new Set(matchingIds);
      for (let d = 0; d < maxDepth; d++) {
        const next = new Set<string>();
        for (const edge of allEdges) {
          const sl = edge.source.toLowerCase();
          const tl = edge.target.toLowerCase();
          if (frontier.has(sl) && !matchingIds.has(tl)) next.add(tl);
          if (frontier.has(tl) && !matchingIds.has(sl)) next.add(sl);
        }
        for (const id of next) matchingIds.add(id);
        frontier = next;
        if (next.size === 0) break;
      }
    }

    const filteredNotes = allNotes.filter((n) => matchingIds.has(n.id.toLowerCase()));
    const filteredEdges = allEdges.filter((e) => matchingIds.has(e.source.toLowerCase()) && matchingIds.has(e.target.toLowerCase()));
    return { notes: filteredNotes, edges: filteredEdges };
  });

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

    // Read theme colors
    const style = getComputedStyle(document.documentElement);
    const primary = style.getPropertyValue("--primary").trim() || "#c9a84c";
    const fg = style.getPropertyValue("--foreground").trim() || "#ffffff";
    const muted = style.getPropertyValue("--muted-foreground").trim() || "#888888";

    ctx.clearRect(0, 0, width, height);

    // Edges
    ctx.strokeStyle = muted + "20";
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
        ? primary
        : node.connections > 0
          ? fg + "80"
          : fg + "33";
      ctx.fill();

      // Label for hovered or connected nodes
      if (isHovered || node.connections >= 2) {
        ctx.font = isHovered ? "12px 'Source Sans 3', sans-serif" : "10px 'Source Sans 3', sans-serif";
        ctx.fillStyle = isHovered ? fg : muted + "66";
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
  <header class="border-b border-border px-6 py-2.5 flex items-center gap-4 shrink-0">
    <span class="text-xs text-muted-foreground shrink-0">Graph View</span>

    <!-- Tag filter -->
    <select
      bind:value={filterTag}
      class="text-[11px] bg-muted/30 border border-border/50 rounded-md px-2 py-1 text-foreground/80 outline-none focus:border-primary/40 transition-colors"
    >
      <option value="">All tags</option>
      {#each availableTags as tag}
        <option value={tag}>{tag}</option>
      {/each}
    </select>

    <!-- Depth slider (only when filtering) -->
    {#if filterTag}
      <div class="flex items-center gap-2">
        <span class="text-[10px] text-muted-foreground/60">Depth</span>
        <input
          type="range"
          min="0"
          max="5"
          bind:value={maxDepth}
          class="w-16 accent-primary"
        />
        <span class="text-[10px] text-muted-foreground font-mono w-4">{maxDepth || "∞"}</span>
      </div>
    {/if}

    {#if filterTag}
      <button
        onclick={() => { filterTag = ""; maxDepth = 0; }}
        class="text-[10px] text-muted-foreground/50 hover:text-foreground/70 transition-colors"
      >
        Clear
      </button>
    {/if}

    <span class="flex-1"></span>
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

// === Tesela v4 — The Mosaic ===
// Spatial canvas. Pages are arrangements of tile-cards; indent = containment.
// Keyboard: h/j/k/l = move focus, Enter = drill in, Esc = back out.

const { useState, useEffect, useRef } = React;

// Block tiles for the daily note, manually positioned for the mock.
// Real app would persist x/y/size per block.
const TILES = [
  { id: "intent", x: 60, y: 30, w: 470, h: 100, type: "note", kind: "Heading", large: true, heading: true,
    text: "A quiet Friday for [[search-ranking|shipping search]]." },
  { id: "morning", x: 60, y: 152, w: 360, h: 92, type: "note", kind: "Note",
    text: "Morning intent — finish ranking fix and the weekly review.",
    foot: "08:14" },
  { id: "search-task", x: 440, y: 152, w: 380, h: 200, type: "task", kind: "Task", container: true,
    text: "Fix block-rank weighting in FTS5 query",
    props: [{ k: "doing", c: "s-doing" }, { k: "high", c: "p-high" }, { k: "Apr 26", c: "" }],
    children: [
      { type: "note", text: "Title hits dominate; body matches barely register." },
      { type: "task", text: "Try BM25 with column weights (3.0, 1.0, 2.0)" },
      { type: "note", text: "Bench against [[Claire Rodriguez]]'s 14k corpus." },
    ] },
  { id: "outliner", x: 60, y: 268, w: 360, h: 132, type: "project", kind: "Project",
    text: "Sketch the new outliner block visual — tile vs. rule treatment",
    props: [{ k: "doing", c: "s-doing" }, { k: "May 3", c: "" }] },
  { id: "claire", x: 60, y: 416, w: 360, h: 110, type: "person", kind: "Person",
    text: "1:1 w/ [[Claire Rodriguez]] — bring 3 printed mocks #design",
    props: [{ k: "Apr 26", c: "" }, { k: "medium", c: "" }] },
  { id: "issues", x: 440, y: 372, w: 380, h: 154, type: "issue", kind: "Issue", container: true,
    text: "Inbox triage — 3 #Issue items need a home",
    children: [
      { type: "task", text: "Block render flicker on indent" },
      { type: "task", text: "Lua plugin error swallowed silently" },
    ] },
  { id: "metaphor", x: 840, y: 30, w: 320, h: 120, type: "note", kind: "Note", large: true,
    text: "Walk notes — every block is a tile, the page is the frame.",
    foot: "09:27" },
  { id: "release", x: 840, y: 168, w: 320, h: 110, type: "ritual", kind: "Ritual",
    text: "Weekly review — carry over unfinished, schedule [[release-checklist]].",
    props: [{ k: "Friday", c: "" }] },
  { id: "audit", x: 840, y: 296, w: 320, h: 132, type: "task", kind: "Task",
    text: "Audit FTS5 trigger correctness on rename — feeds [[Tesela 2.0]]",
    props: [{ k: "doing", c: "s-doing" }, { k: "critical", c: "p-critical" }, { k: "Apr 25", c: "" }] },
];

// Connections between tiles (block-level edges)
const EDGES = [
  ["intent", "search-task"],
  ["search-task", "outliner"],
  ["search-task", "claire"],
  ["outliner", "claire"],
  ["search-task", "audit"],
];

function richTextV4(s) {
  if (!s) return null;
  const out = [];
  const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0; let m; let i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) {
      out.push(<span key={`l${i++}`} className="v4link">{m[3] || m[2]}</span>);
    } else {
      out.push(<span key={`t${i++}`} className="v4tag">#{m[5]}</span>);
    }
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

function V4Tile({ t, focused, onFocus }) {
  const cls = [
    "v4-tile",
    t.large ? "large" : "",
    t.heading ? "heading" : "",
    t.container ? "container" : "",
    focused ? "focused" : "",
  ].filter(Boolean).join(" ");
  return (
    <div
      className={cls}
      data-type={t.type}
      style={{ left: t.x, top: t.y, width: t.w, minHeight: t.h }}
      onClick={(e) => { e.stopPropagation(); onFocus(t.id); }}
    >
      <div className="grout"></div>
      {!t.heading && (
        <div className="tile-meta">
          <span className="tname">{t.kind}</span>
        </div>
      )}
      <div className="tile-text">{richTextV4(t.text)}</div>
      {t.children && (
        <div className="child-stack">
          {t.children.map((c, i) => (
            <div key={i} className="child" data-type={c.type}>
              <span className="dot"></span>
              <span className="tx">{richTextV4(c.text)}</span>
            </div>
          ))}
        </div>
      )}
      {(t.props || t.foot) && (
        <div className="tile-foot">
          <div className="props">
            {t.props && t.props.map((p, i) => (
              <span key={i} className={`pchip ${p.c}`}>{p.k}</span>
            ))}
          </div>
          {t.foot && <span className="ts">{t.foot}</span>}
        </div>
      )}
    </div>
  );
}

function V4Connections({ tiles, edges }) {
  // Compute curves between tile centers
  const byId = Object.fromEntries(tiles.map(t => [t.id, t]));
  const path = (a, b) => {
    const ax = a.x + a.w / 2;
    const ay = a.y + a.h / 2;
    const bx = b.x + b.w / 2;
    const by = b.y + b.h / 2;
    const cx = (ax + bx) / 2;
    return `M ${ax} ${ay} Q ${cx} ${ay} ${cx} ${(ay + by) / 2} T ${bx} ${by}`;
  };
  return (
    <svg className="v4-connections" width="100%" height="100%">
      <defs>
        <marker id="v4dot" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="5" markerHeight="5">
          <circle cx="5" cy="5" r="3.5" fill="#b8732a" opacity="0.6"/>
        </marker>
      </defs>
      {edges.map(([aid, bid], i) => {
        const a = byId[aid]; const b = byId[bid];
        if (!a || !b) return null;
        return (
          <path key={i}
            d={path(a, b)}
            fill="none"
            stroke="#b8732a"
            strokeWidth="1.4"
            strokeOpacity="0.30"
            strokeDasharray="3 5"
            markerStart="url(#v4dot)"
            markerEnd="url(#v4dot)"
          />
        );
      })}
    </svg>
  );
}

function V4Quicklook({ tile }) {
  if (!tile) return null;
  return (
    <div className="v4-quicklook">
      <div className="head">{tile.kind}</div>
      <div className="sub">{tile.text.length > 70 ? tile.text.slice(0, 70) + "…" : tile.text}</div>
      <div className="row"><span className="k">created</span><span className="v">2026-04-29 09:14</span></div>
      <div className="row"><span className="k">links in</span><span className="v">3</span></div>
      <div className="row"><span className="k">links out</span><span className="v">2</span></div>
      <div className="row"><span className="k">type</span><span className="v">{tile.kind}</span></div>
      <div className="links">
        <div className="h">links</div>
        <a>→ Tesela 2.0</a>
        <a>→ Claire Rodriguez</a>
        <a>← Search ranking</a>
      </div>
    </div>
  );
}

function V4Minimap({ tiles }) {
  const minX = 0, minY = 0;
  const maxX = Math.max(...tiles.map(t => t.x + t.w)) + 60;
  const maxY = Math.max(...tiles.map(t => t.y + t.h)) + 60;
  const colorFor = (type) => {
    const map = { task: "#b04a3e", project: "#4a5a8a", person: "#7a4a7a", ritual: "#a87d2a", domain: "#6a8454", issue: "#b04a3e", note: "#d4a466" };
    return map[type] || "#b8ad9f";
  };
  const sx = 152 / (maxX - minX);
  const sy = 80 / (maxY - minY);
  return (
    <div className="v4-minimap">
      <div className="head"><span>Page Map</span><span>{tiles.length} tiles</span></div>
      <div className="frame">
        {tiles.map(t => (
          <div key={t.id} className="blip" style={{
            left: t.x * sx, top: t.y * sy,
            width: Math.max(4, t.w * sx), height: Math.max(3, t.h * sy),
            background: colorFor(t.type),
          }}/>
        ))}
        <div className="viewport" style={{ left: 4, top: 4, width: 110, height: 60 }}/>
      </div>
    </div>
  );
}

function V4App() {
  const [focused, setFocused] = useState("search-task");
  const focusedTile = TILES.find(t => t.id === focused);

  useEffect(() => {
    const onKey = (e) => {
      if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA") return;
      if (!["h","j","k","l","ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(e.key)) return;
      e.preventDefault();
      const cur = TILES.find(t => t.id === focused);
      if (!cur) return;
      const cx = cur.x + cur.w/2, cy = cur.y + cur.h/2;
      let dirX = 0, dirY = 0;
      if (e.key === "h" || e.key === "ArrowLeft") dirX = -1;
      if (e.key === "l" || e.key === "ArrowRight") dirX = 1;
      if (e.key === "k" || e.key === "ArrowUp") dirY = -1;
      if (e.key === "j" || e.key === "ArrowDown") dirY = 1;
      let best = null, bestScore = Infinity;
      for (const t of TILES) {
        if (t.id === focused) return;
        const tx = t.x + t.w/2, ty = t.y + t.h/2;
        const dx = tx - cx, dy = ty - cy;
        // Must be in the right direction
        if (dirX && Math.sign(dx) !== dirX) continue;
        if (dirY && Math.sign(dy) !== dirY) continue;
        const along = dirX ? Math.abs(dx) : Math.abs(dy);
        const perp = dirX ? Math.abs(dy) : Math.abs(dx);
        const score = along + perp * 1.4;
        if (score < bestScore) { bestScore = score; best = t; }
      }
      if (best) setFocused(best.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [focused]);

  const railPills = [
    { id: "today", label: "TDY", active: true },
    { id: "task", label: "T", color: "rose", badge: 8 },
    { id: "project", label: "P", color: "indigo" },
    { id: "person", label: "@", color: "plum" },
    { id: "ritual", label: "R", color: "ochre" },
    { id: "issue", label: "!", color: "rose", badge: 3 },
  ];

  return (
    <div className="v4-app" data-screen-label="01 Mosaic">
      <div className="v4-rail">
        <div className="brand"><i></i><i></i><i></i><i></i></div>
        {railPills.map(p => (
          <div key={p.id} className={`pill ${p.active ? "active" : ""}`} title={p.id}>
            {p.label}
            {p.badge && <span className="badge">{p.badge}</span>}
          </div>
        ))}
        <div className="grow"></div>
        <div className="pill" title="Settings" style={{ fontSize: 14 }}>⚙</div>
      </div>
      <div className="v4-stage">
        <div className="v4-topstrip">
          <div className="left">
            <span className="v4-mode">NORMAL</span>
            <div className="crumb-arrows">
              <div className="arrow" title="Yesterday">‹</div>
              <div className="arrow" title="Tomorrow">›</div>
            </div>
            <div className="pageinfo">
              <span className="pagename">A quiet Friday for shipping search.</span>
              <span className="pagedate">Daily · 2026-04-29 · 9 tiles · 3 links</span>
            </div>
          </div>
          <div className="right">
            <span className="key"><kbd>h</kbd><kbd>j</kbd><kbd>k</kbd><kbd>l</kbd> move</span>
            <span className="key"><kbd>↵</kbd> drill in</span>
            <span className="key"><kbd>SPC</kbd> leader</span>
            <span className="key"><kbd>⌘K</kbd> jump</span>
          </div>
        </div>
        <div className="v4-board-wrap" onClick={() => setFocused(null)}>
          <div className="v4-board"></div>
          <V4Connections tiles={TILES} edges={EDGES} />
          {TILES.map(t => (
            <V4Tile key={t.id} t={t} focused={t.id === focused} onFocus={setFocused} />
          ))}
          <V4Quicklook tile={focusedTile} />
          <V4Minimap tiles={TILES} />
          <div className="v4-hint">
            <span className="ico">M</span>
            <div className="body">
              <strong>The Mosaic.</strong> Every block is a tile you can place, pick up,
              and group. Indent becomes containment — drag a tile <em>into</em> another
              to make it a child. Press <kbd>g</kbd><kbd>o</kbd> for an outline view.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

window.V4App = V4App;

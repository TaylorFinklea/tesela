// === Tesela v3 — Editorial. Warm paper, soft light, beautiful by default. ===
// Reuses MockData and Icons from v1 to keep things aligned.

const { useState, useEffect, useRef, useMemo } = React;

// --- Tag → variant map ---
const TAG_KIND = {
  Task: "task", Project: "project", Person: "person", Domain: "domain",
  Ritual: "ritual", Issue: "issue", Daily: "ritual",
};

// Render text with #tags and [[wikilinks]] inline
function richText(s) {
  if (!s) return null;
  const out = [];
  // pattern: [[target|alias]] or [[target]] or #Tag
  const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0; let m; let i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) {
      const label = m[3] || m[2];
      out.push(<span key={`l${i++}`} className="v3link">{label}</span>);
    } else {
      const tag = m[5];
      const kind = TAG_KIND[tag] || "";
      out.push(<span key={`t${i++}`} className={`v3tag ${kind}`}>{tag}</span>);
    }
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

// --- Sidebar ---
function V3Sidebar({ active, onNav }) {
  const { NOTES_LIST, RECENT_IDS } = window.MockData;
  const recents = RECENT_IDS.map(id => NOTES_LIST.find(n => n.id === id)).filter(Boolean);
  const navItems = [
    { id: "today", label: "Today", count: "Fri", icon: "calendar" },
    { id: "tasks", label: "Tasks", count: 38, icon: "check" },
    { id: "graph", label: "Graph", count: null, icon: "graph" },
  ];
  const tagDot = (tags) => {
    if (!tags || !tags.length) return "";
    const t = tags[0].toLowerCase();
    return ["task","project","person","domain","ritual","issue"].includes(t) ? t : "";
  };
  return (
    <aside className="v3-sidebar">
      <div className="v3-brand">
        <div className="mark">T</div>
        <div className="name">Tes<em>e</em>la</div>
      </div>
      <div className="v3-sidebar-search">
        <svg className="ico" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="6"/><path d="m20 20-3.5-3.5"/></svg>
        <input placeholder="Quick search…" />
        <kbd>⌘K</kbd>
      </div>
      <div className="v3-nav">
        {navItems.map(it => (
          <div key={it.id} className={`v3-nav-item ${active === it.id ? "active" : ""}`} onClick={() => onNav(it.id)}>
            <svg className="ico" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
              {it.icon === "calendar" && <><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M8 3v4M16 3v4M3 9h18"/></>}
              {it.icon === "check" && <><path d="m5 12 4 4 10-10"/></>}
              {it.icon === "graph" && <><circle cx="6" cy="6" r="2"/><circle cx="18" cy="7" r="2"/><circle cx="12" cy="17" r="2"/><path d="M8 7l8 .5M7 8l4 7M17 9l-4 6"/></>}
            </svg>
            <span className="label">{it.label}</span>
            {it.count != null && <span className="meta">{it.count}</span>}
          </div>
        ))}
      </div>
      <div className="v3-section-label">Recent <span className="add">+</span></div>
      <div className="v3-pages" style={{ flex: "0 0 auto", marginBottom: 4 }}>
        {recents.map(n => (
          <div key={n.id} className="v3-page-item" onClick={() => onNav("today")}>
            <span className={`dot ${tagDot(n.tags)}`}></span>
            <span className="label">{n.title}</span>
          </div>
        ))}
      </div>
      <div className="v3-section-label">Pages <span className="add">+</span></div>
      <div className="v3-pages">
        {window.MockData.NOTES_LIST.map(n => (
          <div key={n.id} className="v3-page-item">
            <span className={`dot ${tagDot(n.tags)}`}></span>
            <span className="label">{n.title}</span>
          </div>
        ))}
      </div>
      <div className="v3-sidebar-foot">
        <div className="avatar">T</div>
        <div className="name">Taylor</div>
        <span className="gear">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>
        </span>
      </div>
    </aside>
  );
}

// --- Outliner ---
function V3Block({ block, focused, hasChildren, onFocus }) {
  const { props } = block;
  const propEntries = Object.entries(props || {});
  const dataType = block.tag || "";
  const isDone = props?.status === "done";
  return (
    <div
      className={`v3-block ${focused ? "focused" : ""} ${hasChildren ? "has-children" : ""}`}
      data-type={dataType}
      data-done={isDone ? "true" : "false"}
      onClick={onFocus}
    >
      <div className="gutter">
        <span className="grip"><i/><i/><i/><i/><i/><i/></span>
        <span className="bullet"></span>
      </div>
      <div className="body">
        <div className="text">{richText(block.text)}</div>
        {propEntries.length > 0 && (
          <div className="props">
            {propEntries.map(([k, v]) => {
              let kind = `kind-${k}`;
              let mod = "";
              if (k === "status") mod = `s-${v}`;
              if (k === "priority") mod = `p-${v}`;
              return (
                <span key={k} className={`v3-pchip ${kind} ${mod}`}>
                  <span className="k">{k}</span>
                  <span className="v">{v}</span>
                </span>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function V3Outliner({ blocks, focused, setFocused }) {
  // Group children under parents (one level deep grouping for visual purposes)
  const items = [];
  for (let i = 0; i < blocks.length; i++) {
    const b = blocks[i];
    const next = blocks[i + 1];
    const hasChildren = next && next.indent > b.indent;
    items.push({ block: b, hasChildren: !!hasChildren });
  }
  return (
    <div className="v3-outliner">
      {items.map(({ block, hasChildren }, i) => (
        <div key={block.id} style={{ marginLeft: block.indent * 28 }}>
          <V3Block block={block} hasChildren={hasChildren} focused={focused === block.id} onFocus={() => setFocused(block.id)} />
        </div>
      ))}
    </div>
  );
}

// --- Right pane ---
function V3RightPane({ tab, setTab }) {
  const { BACKLINKS, OUTLINE_LINKS, PAGE_PROPERTIES } = window.MockData;
  const tabs = [
    { id: "backlinks", label: "Backlinks", badge: BACKLINKS.length },
    { id: "outline", label: "Outline", badge: OUTLINE_LINKS.length },
    { id: "properties", label: "Properties" },
  ];
  return (
    <aside className="v3-aux">
      <div className="v3-aux-head">
        <div className="v3-aux-tabs">
          {tabs.map(t => (
            <div key={t.id} className={`v3-aux-tab ${tab === t.id ? "active" : ""}`} onClick={() => setTab(t.id)}>
              {t.label}{t.badge != null && <span className="badge">{t.badge}</span>}
            </div>
          ))}
        </div>
      </div>
      <div className="v3-aux-body">
        {tab === "backlinks" && (
          <div>
            <div className="v3-aux-section">
              <div className="v3-aux-h">Linked from {BACKLINKS.length} pages</div>
              {BACKLINKS.map((b, i) => (
                <div key={i} className="v3-bl" data-type={b.icon.toLowerCase()}>
                  <div className="v3-bl-head">
                    <span className="src">{b.source}</span>
                    <span className="badge">{b.icon}</span>
                  </div>
                  <div className="v3-bl-snippet" dangerouslySetInnerHTML={{ __html: b.snippet }} />
                </div>
              ))}
            </div>
          </div>
        )}
        {tab === "outline" && (
          <div className="v3-aux-section">
            <div className="v3-aux-h">On this page</div>
            {OUTLINE_LINKS.map((o, i) => (
              <div key={i} className={`v3-outline-link ${i === 1 ? "active" : ""}`} style={{ paddingLeft: 8 + o.depth * 14 }} data-type={o.label.toLowerCase().includes("block-rank") ? "task" : ""}>
                <span className="marker"></span>
                <span className="text">{o.label}</span>
              </div>
            ))}
          </div>
        )}
        {tab === "properties" && (
          <div className="v3-aux-section">
            <div className="v3-aux-h">Frontmatter</div>
            <div className="v3-fm">
              {PAGE_PROPERTIES.map(p => (
                <div key={p.k} className="v3-fm-row">
                  <div className="k">{p.k}</div>
                  <div className="v">{p.v}</div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}

// --- Command Palette ---
function V3Palette({ onClose, onNav }) {
  const [q, setQ] = useState("");
  const inputRef = useRef(null);
  useEffect(() => { inputRef.current?.focus(); }, []);
  const items = [
    { section: "Jump", label: "Today's daily note", sub: "Apr 24, 2026", icon: "calendar", action: () => onNav("today") },
    { section: "Jump", label: "Tasks — by status", sub: "38 tasks", icon: "check", action: () => onNav("tasks") },
    { section: "Jump", label: "Graph view", icon: "graph" },
    { section: "Create", label: "New note…", shortcut: "n", icon: "plus" },
    { section: "Create", label: "New tag definition", icon: "hash" },
    { section: "Theme", label: "Toggle Day / Evening", icon: "sun" },
    { section: "Theme", label: "Toggle right pane", shortcut: "]", icon: "panel" },
  ].filter(i => !q || i.label.toLowerCase().includes(q.toLowerCase()));
  const sections = [...new Set(items.map(i => i.section))];
  const handleKey = (e) => {
    if (e.key === "Escape") onClose();
    if (e.key === "Enter" && items[0]?.action) { items[0].action(); onClose(); }
  };
  return (
    <div className="v3-palette-overlay" onClick={onClose}>
      <div className="v3-palette" onClick={e => e.stopPropagation()}>
        <div className="v3-palette-search">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#7a6f64" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="6"/><path d="m20 20-3.5-3.5"/></svg>
          <input ref={inputRef} value={q} onChange={e => setQ(e.target.value)} onKeyDown={handleKey} placeholder="Find a page, run a command…" />
          <kbd style={{ fontSize: 10.5, background: "#f7f3ec", border: "1px solid #ece4d4", borderRadius: 4, padding: "1px 5px", color: "#7a6f64" }}>esc</kbd>
        </div>
        <div className="v3-palette-list">
          {sections.map(sec => (
            <div key={sec}>
              <div className="v3-palette-section">{sec}</div>
              {items.filter(i => i.section === sec).map((it, i) => (
                <div key={i} className={`v3-palette-item ${i === 0 && sec === sections[0] ? "selected" : ""}`} onClick={() => { it.action?.(); onClose(); }}>
                  <span className="ico">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                      {it.icon === "calendar" && <><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M8 3v4M16 3v4M3 9h18"/></>}
                      {it.icon === "check" && <path d="m5 12 4 4 10-10"/>}
                      {it.icon === "graph" && <><circle cx="6" cy="6" r="2"/><circle cx="18" cy="7" r="2"/><circle cx="12" cy="17" r="2"/><path d="M8 7l8 .5M7 8l4 7M17 9l-4 6"/></>}
                      {it.icon === "plus" && <path d="M12 5v14M5 12h14"/>}
                      {it.icon === "hash" && <path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18"/>}
                      {it.icon === "sun" && <><circle cx="12" cy="12" r="4"/><path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4"/></>}
                      {it.icon === "panel" && <><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></>}
                    </svg>
                  </span>
                  <span className="label">{it.label}</span>
                  {it.sub && <span className="sub">{it.sub}</span>}
                  {it.shortcut && <kbd>{it.shortcut}</kbd>}
                </div>
              ))}
            </div>
          ))}
        </div>
        <div className="v3-palette-foot">
          <span><kbd>↑↓</kbd>navigate</span>
          <span><kbd>↵</kbd>open</span>
          <span><kbd>esc</kbd>close</span>
        </div>
      </div>
    </div>
  );
}

// --- Tasks View ---
function V3TasksView() {
  const { TASK_ROWS } = window.MockData;
  // Group by status
  const order = ["doing", "todo", "done", "backlog"];
  const labels = { doing: "Doing now", todo: "Up next", done: "Done", backlog: "Backlog" };
  const groups = order.map(s => ({ status: s, label: labels[s], rows: TASK_ROWS.filter(r => r.status === s) }));
  const [activeFilter, setActiveFilter] = useState("status");
  return (
    <div className="v3-canvas-inner" style={{ maxWidth: 880 }}>
      <div className="v3-tasks-head">
        <div>
          <h1>What's <em>doing</em>, what's next.</h1>
          <div className="desc">{TASK_ROWS.length} tasks across {new Set(TASK_ROWS.map(r => r.source)).size} pages — grouped by status.</div>
        </div>
      </div>
      <div className="v3-filter-bar">
        <span className={`chip ${activeFilter === "status" ? "active" : ""}`} onClick={() => setActiveFilter("status")}>
          <span className="k">group:</span><span className="v">status</span>
        </span>
        <span className="chip"><span className="k">tag:</span><span className="v">Task</span></span>
        <span className="chip"><span className="k">deadline:</span><span className="v">≤ this week</span></span>
        <span className="chip"><span className="k">sort:</span><span className="v">priority ↓</span></span>
        <button className="add-filter">+ add filter</button>
      </div>
      {groups.map(g => (
        g.rows.length > 0 && (
          <div key={g.status} className="v3-task-group">
            <div className="v3-task-group-head">
              <span className="label">{g.label}</span>
              <span className="count">{g.rows.length}</span>
            </div>
            {g.rows.map((r, i) => (
              <div key={i} className="v3-task-row" data-status={r.status}>
                <div className="v3-task-check">
                  {r.status === "done" && (
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="m5 12 4 4 10-10"/></svg>
                  )}
                </div>
                <div>
                  <div className="v3-task-text">{r.text}</div>
                  <div className="v3-task-source">in <span className="v3link">{r.source}</span></div>
                </div>
                <div className={`v3-task-meta-priority p-${r.priority}`}>{r.priority !== "—" ? r.priority : ""}</div>
                <div className={`v3-task-meta-deadline ${r.deadline === "Apr 25" || r.deadline === "Apr 26" ? "urgent" : ""}`}>
                  {r.deadline !== "—" ? r.deadline : ""}
                </div>
                <div></div>
              </div>
            ))}
          </div>
        )
      ))}
    </div>
  );
}

// --- App shell ---
function V3App() {
  const { TODAY_STR, DAILY_BLOCKS } = window.MockData;
  const [route, setRoute] = useState("today");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [auxTab, setAuxTab] = useState("backlinks");
  const [focused, setFocused] = useState("b2");
  const [vimMode] = useState("NORMAL");

  useEffect(() => {
    const onKey = (e) => {
      const t = e.target;
      const editing = t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable;
      if (editing) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); setPaletteOpen(true); }
      if (e.key === "/") { e.preventDefault(); setPaletteOpen(true); }
      if (e.key === "g") setRoute("today");
      if (e.key === "t") setRoute("tasks");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="v3-app" data-screen-label={route === "today" ? "01 Daily" : route === "tasks" ? "02 Tasks" : "03 Other"}>
      <div className="v3-body">
        <V3Sidebar active={route} onNav={setRoute} />
        <main className="v3-main">
          <div className="v3-topbar">
            <div className="v3-crumb">
              <span>Daily</span>
              <span className="sep">›</span>
              <span className="now">{route === "today" ? "Apr 24, 2026" : route === "tasks" ? "Tasks" : "Graph"}</span>
            </div>
            <div className="right">
              <div className="iconbtn" title="Search (⌘K)" onClick={() => setPaletteOpen(true)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="6"/><path d="m20 20-3.5-3.5"/></svg>
              </div>
              <div className="iconbtn" title="Star">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M12 3l2.6 5.5 6 .9-4.3 4.2 1 6L12 17l-5.3 2.6 1-6L3.4 9.4l6-.9L12 3z"/></svg>
              </div>
              <div className="iconbtn active" title="Right pane">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></svg>
              </div>
            </div>
          </div>
          <div className="v3-canvas">
            {route === "today" && (
              <div className="v3-canvas-inner">
                <div className="v3-page-eyebrow">
                  <span className="pill">Daily</span>
                  <span>Friday</span>
                  <span className="dot">·</span>
                  <span>Week 17</span>
                  <span className="dot">·</span>
                  <span>4 tasks · 2 doing</span>
                </div>
                <h1 className="v3-page-title">A quiet Friday for <em>shipping search</em>.</h1>
                <div className="v3-page-meta">
                  <div className="item">
                    <span className="ico"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg></span>
                    Started <strong>8:14 am</strong>
                  </div>
                  <div className="item">
                    <span className="ico"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/></svg></span>
                    <strong>{DAILY_BLOCKS.length}</strong> blocks
                  </div>
                  <div className="item">
                    <span className="ico"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9-9-9z"/><circle cx="7.5" cy="7.5" r="1.5"/></svg></span>
                    <strong>Task, Project, Issue</strong>
                  </div>
                </div>
                <V3Outliner blocks={DAILY_BLOCKS} focused={focused} setFocused={setFocused} />
                <div className="v3-add-block">
                  <span style={{ display: "inline-flex" }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14M5 12h14"/></svg>
                  </span>
                  <span>New block — type, or use commands</span>
                  <div className="keys">
                    <kbd>/</kbd><span style={{ fontSize: 10.5 }}>commands</span>
                    <kbd>#</kbd><span style={{ fontSize: 10.5 }}>tag</span>
                    <kbd>[[</kbd><span style={{ fontSize: 10.5 }}>link</span>
                  </div>
                </div>
              </div>
            )}
            {route === "tasks" && <V3TasksView />}
            {route === "graph" && (
              <div className="v3-empty">
                <div>
                  <div className="title">Graph view</div>
                  <div>Force-directed view of pages and their links — coming soon in this mock.</div>
                </div>
              </div>
            )}
          </div>
          <div className="v3-status">
            <span className={`mode ${vimMode.toLowerCase()}`}>{vimMode}</span>
            <span className="doc">{route === "today" ? "Daily — 2026-04-24" : route === "tasks" ? "Tasks" : "Graph"}</span>
            <span className="sep">·</span>
            <span className="saved">saved</span>
            <span className="sep">·</span>
            <span>⌘K palette</span>
          </div>
        </main>
        {route === "today" && <V3RightPane tab={auxTab} setTab={setAuxTab} />}
      </div>
      {paletteOpen && <V3Palette onClose={() => setPaletteOpen(false)} onNav={(r) => setRoute(r)} />}
    </div>
  );
}

window.V3App = V3App;

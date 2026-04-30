// === Tesela v5 — The Workbench ===
// Inverts the Logseq model: the type is the primary unit, not the page.
// Center column = active type pane (Tasks). Left rail = scratchpad daily.
// Right column = structured property inspector + linked context.

const { useState, useEffect } = React;

const TYPES = [
  { id: "daily",   label: "Daily",   glyph: "D", count: 412 },
  { id: "task",    label: "Task",    glyph: "T", count: 38, active: true },
  { id: "project", label: "Project", glyph: "P", count: 7 },
  { id: "person",  label: "Person",  glyph: "@", count: 22 },
  { id: "domain",  label: "Domain",  glyph: "#", count: 14 },
  { id: "ritual",  label: "Ritual",  glyph: "R", count: 6 },
  { id: "issue",   label: "Issue",   glyph: "!", count: 11 },
  { id: "note",    label: "Note",    glyph: "N", count: 156 },
];

const TASKS = [
  // Doing
  { id: "t1", status: "doing", text: "Fix block-rank weighting in FTS5 query — title hits dominate, body matches barely register",
    src: { type: "daily", label: "2026-04-29" }, priority: "high", deadline: "Apr 26", deadlineUrgent: true, effort: "M (3h)" },
  { id: "t2", status: "doing", text: "Audit FTS5 trigger correctness on rename — feeds Tesela 2.0 release",
    src: { type: "project", label: "Tesela 2.0" }, priority: "critical", deadline: "Apr 25", deadlineUrgent: true, effort: "L (1d)" },
  { id: "t3", status: "doing", text: "Sketch new outliner block visual — tile vs. rule treatment, decide on indent depth at 8 vs 12px",
    src: { type: "project", label: "Outliner Refresh" }, priority: "medium", deadline: "May 3", effort: "M (4h)" },

  // Today
  { id: "t4", status: "todo", text: "1:1 with Claire Rodriguez — bring 3 printed mocks + ranking benchmark numbers",
    src: { type: "person", label: "Claire Rodriguez" }, priority: "medium", deadline: "Apr 26", effort: "S (1h)" },
  { id: "t5", status: "todo", text: "Triage 3 #Issue items in inbox — block render flicker on indent + 2 others",
    src: { type: "daily", label: "2026-04-29" }, priority: "high", deadline: "Apr 26", deadlineUrgent: true, effort: "S (45m)" },
  { id: "t6", status: "todo", text: "Try BM25 column weights (3.0, 1.0, 2.0) and bench against Claire's 14k corpus",
    src: { type: "issue", label: "search-ranking" }, priority: "high", deadline: "Apr 26", effort: "M (2h)" },
  { id: "t7", status: "todo", text: "Carry over unfinished from yesterday — schedule release-checklist review",
    src: { type: "ritual", label: "Weekly review" }, priority: "low", deadline: "Apr 26", effort: "S (30m)" },

  // This week
  { id: "t8", status: "todo", text: "Write release notes for 2.0 — focus on type system + Lua plugin API",
    src: { type: "project", label: "Tesela 2.0" }, priority: "medium", deadline: "May 1", effort: "M (3h)" },
  { id: "t9", status: "todo", text: "Lua plugin error handler swallows stack — surface in console panel",
    src: { type: "issue", label: "lua-errors" }, priority: "medium", deadline: "May 2", effort: "S (1h)" },
  { id: "t10", status: "todo", text: "Reach out to early-access cohort for ranking-fix verification",
    src: { type: "person", label: "EA cohort" }, priority: "low", deadline: "May 3", effort: "S (45m)" },

  // Done
  { id: "t11", status: "done", text: "Wire FTS5 column-weight UI behind dev flag",
    src: { type: "project", label: "Tesela 2.0" }, priority: "medium", deadline: "Apr 28", effort: "M" },
  { id: "t12", status: "done", text: "Daily standup notes",
    src: { type: "daily", label: "2026-04-29" }, priority: "low", deadline: "Apr 29", effort: "S" },
];

const SCRATCH = [
  { indent: 0, text: "Morning intent — finish ranking fix and the weekly review.", promoted: false },
  { indent: 0, text: "Fix block-rank weighting in FTS5 query", type: "task", promoted: true },
  { indent: 1, text: "Title hits dominate; body matches barely register.", promoted: false },
  { indent: 1, text: "Try BM25 with column weights (3.0, 1.0, 2.0)", type: "task", promoted: true },
  { indent: 1, text: "Bench against [[Claire Rodriguez]] 's 14k corpus.", promoted: false },
  { indent: 0, text: "Sketch the new outliner block visual", type: "project", promoted: true },
  { indent: 0, text: "1:1 w/ [[Claire Rodriguez]] — bring 3 printed mocks #design", type: "task", promoted: true },
  { indent: 0, text: "Inbox triage — 3 #Issue items need a home", type: "task", promoted: true },
  { indent: 1, text: "Block render flicker on indent", type: "issue", promoted: true },
  { indent: 1, text: "Lua plugin error swallowed silently", type: "issue", promoted: true },
  { indent: 0, text: "Walk notes — every block is a tile, the page is the frame.", promoted: false },
  { indent: 0, text: "Weekly review — carry over unfinished, schedule [[release-checklist]].", type: "ritual", promoted: true },
  { indent: 0, text: "Audit FTS5 trigger correctness on rename — feeds [[Tesela 2.0]]", type: "task", promoted: true },
];

function richV5(s) {
  if (!s) return null;
  const out = [];
  const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0; let m; let i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) {
      out.push(<span key={`l${i++}`} className="v5link">{m[3] || m[2]}</span>);
    } else {
      out.push(<span key={`t${i++}`} className="v5tag">#{m[5]}</span>);
    }
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

function V5TypeRail() {
  return (
    <div className="v5-typerail">
      <div className="brand">T</div>
      {TYPES.map(t => (
        <div key={t.id} className={`typebtn ${t.active ? "active" : ""}`} title={t.label}>
          <span className={`glyph ${t.id}`}>{t.glyph}</span>
          <span className="label">{t.label.slice(0, 4)}</span>
          <span className="count">{t.count}</span>
        </div>
      ))}
      <div className="grow"></div>
      <div className="add" title="New type">+</div>
    </div>
  );
}

function V5Pad() {
  return (
    <div className="v5-pad">
      <div className="v5-pad-head">
        <span className="label">Today's scratchpad</span>
        <span className="title">A quiet Friday</span>
        <span className="date">Friday, 2026-04-29 · 13 blocks</span>
      </div>
      <div className="v5-pad-search">
        <span className="ico">⌕</span>
        <input placeholder="Capture or search…" />
        <kbd>⌘K</kbd>
      </div>
      <div className="v5-pad-blocks">
        {SCRATCH.map((b, i) => (
          <div key={i}
               className={`v5-pad-block indent-${b.indent}`}
               data-promoted={b.promoted ? "true" : "false"}>
            <span className="bullet"><i></i></span>
            <span>
              {b.type && <span className={`ctype ${b.type}`}>{b.type}</span>}
              {b.type && " "}
              {richV5(b.text)}
            </span>
          </div>
        ))}
      </div>
      <div className="v5-pad-foot">
        <span>13 blocks · 6 promoted</span>
        <span className="key"><kbd>⌘↵</kbd> promote</span>
      </div>
    </div>
  );
}

function V5Row({ t, selected, onSelect }) {
  return (
    <div className={`v5-row ${selected ? "selected" : ""}`} data-status={t.status} onClick={() => onSelect(t.id)}>
      <span className="check"></span>
      <div className="text-cell">
        <div className="text">{t.text}</div>
        <div className="source">
          <span className={`ctype ${t.src.type}`}>{t.src.type}</span>
          <span>↳ {t.src.label}</span>
        </div>
      </div>
      <span className={`priority p-${t.priority}`}>{t.priority}</span>
      <span className={`deadline ${t.deadlineUrgent ? "urgent" : ""}`}>{t.deadline}</span>
      <span className="effort">{t.effort}</span>
    </div>
  );
}

function V5Workbench({ selectedId, setSelectedId }) {
  const groups = [
    { id: "doing", label: "Doing", filter: t => t.status === "doing" },
    { id: "today", label: "Today", filter: t => t.status === "todo" && (t.deadline === "Apr 26") },
    { id: "week",  label: "This week", filter: t => t.status === "todo" && t.deadline !== "Apr 26" },
    { id: "done",  label: "Done", filter: t => t.status === "done" },
  ];
  return (
    <div className="v5-workbench">
      <div className="v5-wb-head">
        <div className="v5-wb-title-row">
          <div className="v5-wb-titlegroup">
            <h1 className="v5-wb-title">Task</h1>
            <span className="v5-wb-typetag"><span className="swatch"></span>type pane · 38 instances</span>
          </div>
          <div className="v5-wb-actions">
            <button className="ab"><span>Sort: priority</span></button>
            <button className="ab"><span>Group: status</span></button>
            <button className="ab primary"><span>+ New task</span><kbd>n</kbd></button>
          </div>
        </div>
        <div className="v5-wb-meta">
          <span><strong>3</strong> doing</span>
          <span className="dot">·</span>
          <span><strong>4</strong> due today</span>
          <span className="dot">·</span>
          <span><strong>2</strong> overdue</span>
          <span className="dot">·</span>
          <span>From <strong>Daily 2026-04-29</strong> + 4 other pages</span>
        </div>
        <div className="v5-querybar">
          <span className="v5-qchip"><span className="k">type</span><span className="o">==</span><span className="v">Task</span></span>
          <span className="v5-qchip"><span className="k">status</span><span className="o">!=</span><span className="v">archived</span></span>
          <span className="v5-qchip"><span className="k">deadline</span><span className="o">≤</span><span className="v">+7d</span></span>
          <span className="v5-qchip add">+ filter</span>
        </div>
        <div className="v5-views-tabs">
          <span className="vtab active"><span className="ico">≡</span>Table</span>
          <span className="vtab"><span className="ico">▥</span>Board</span>
          <span className="vtab"><span className="ico">⌘</span>Calendar</span>
          <span className="vtab"><span className="ico">⊞</span>Gallery</span>
          <span className="vtab"><span className="ico">⌥</span>Graph</span>
        </div>
      </div>
      <div className="v5-wb-body">
        {groups.map(g => {
          const items = TASKS.filter(g.filter);
          if (items.length === 0) return null;
          return (
            <div key={g.id} className="v5-group">
              <div className="v5-group-head">
                <span className="chev">▾</span>
                <span className="label">{g.label}</span>
                <span className="count">{items.length}</span>
                <span className="summary">
                  {g.id === "doing" && "in flight · est. 2d"}
                  {g.id === "today" && "due Apr 26 · est. 4h"}
                  {g.id === "week" && "Apr 27 – May 3"}
                </span>
              </div>
              {items.map(t => (
                <V5Row key={t.id} t={t} selected={t.id === selectedId} onSelect={setSelectedId}/>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function V5Inspector({ task }) {
  if (!task) return null;
  return (
    <div className="v5-inspector">
      <div className="v5-ins-head">
        <span className="v5-ins-tag"><span className="swatch"></span>Task · doing</span>
        <span className="v5-ins-title">{task.text}</span>
      </div>
      <div className="v5-ins-tabs">
        <span className="t active">Properties</span>
        <span className="t">Backlinks 5</span>
        <span className="t">Schema</span>
      </div>
      <div className="v5-ins-body">
        <div className="v5-ins-section">
          <div className="h">Core properties</div>
          <div className="v5-prop">
            <span className="k">status</span>
            <span className="v"><span className="swatch"></span>doing</span>
          </div>
          <div className="v5-prop">
            <span className="k">priority</span>
            <span className="v"><span className="swatch" style={{background:"#b04a3e"}}></span>high</span>
          </div>
          <div className="v5-prop">
            <span className="k">deadline</span>
            <span className="v editable">2026-04-26 (Sun)</span>
          </div>
          <div className="v5-prop">
            <span className="k">effort</span>
            <span className="v editable">M · 3h</span>
          </div>
          <div className="v5-prop">
            <span className="k">assignee</span>
            <span className="v"><span className="swatch" style={{background:"#7a4a7a"}}></span>me</span>
          </div>
          <div className="v5-prop">
            <span className="k">domain</span>
            <span className="v"><span className="swatch" style={{background:"#5a7a48"}}></span>search</span>
          </div>
        </div>
        <div className="v5-ins-section">
          <div className="h">Custom · from #task schema</div>
          <div className="v5-prop">
            <span className="k">blocks</span>
            <span className="v editable">[[Tesela 2.0]]</span>
          </div>
          <div className="v5-prop">
            <span className="k">repro</span>
            <span className="v editable">14k corpus</span>
          </div>
          <div className="v5-prop">
            <span className="k">scope</span>
            <span className="v editable">FTS5 only</span>
          </div>
        </div>
        <div className="v5-ins-section">
          <div className="h">Backlinks · 5</div>
        </div>
        <div className="v5-link-card">
          <span className="src"><span className="ctype daily">daily</span>2026-04-29</span>
          <span className="snippet">…Morning intent — finish <mark>ranking fix</mark> and the weekly review…</span>
        </div>
        <div className="v5-link-card">
          <span className="src"><span className="ctype project">project</span>Tesela 2.0</span>
          <span className="snippet">…<mark>FTS5 weighting</mark> is the last blocker before we cut the RC build…</span>
        </div>
        <div className="v5-link-card">
          <span className="src"><span className="ctype person">person</span>Claire Rodriguez</span>
          <span className="snippet">…can run the <mark>14k corpus benchmark</mark> over the weekend if mock numbers land Friday…</span>
        </div>
        <div className="v5-link-card">
          <span className="src"><span className="ctype issue">issue</span>search-ranking</span>
          <span className="snippet">…title hits <mark>dominate</mark>; body matches <mark>barely register</mark>…</span>
        </div>
        <div className="v5-link-card">
          <span className="src"><span className="ctype ritual">ritual</span>Weekly review</span>
          <span className="snippet">…<mark>carry over</mark> unfinished items into next week…</span>
        </div>
      </div>
    </div>
  );
}

function V5App() {
  const [selectedId, setSelectedId] = useState("t1");
  const selected = TASKS.find(t => t.id === selectedId);
  return (
    <div className="v5-app" data-screen-label="01 Workbench">
      <V5TypeRail />
      <V5Pad />
      <V5Workbench selectedId={selectedId} setSelectedId={setSelectedId} />
      <V5Inspector task={selected} />
      <div className="v5-cmdbar">
        <span className="mode">NORMAL</span>
        <span className="doc">type:Task</span>
        <span className="sep">·</span>
        <span className="key"><kbd>j</kbd><kbd>k</kbd> row</span>
        <span className="key"><kbd>g</kbd><kbd>t</kbd> change type</span>
        <span className="key"><kbd>p</kbd> set property</span>
        <span className="key"><kbd>:</kbd> command</span>
        <span className="key"><kbd>⌘K</kbd> jump</span>
      </div>
    </div>
  );
}

window.V5App = V5App;

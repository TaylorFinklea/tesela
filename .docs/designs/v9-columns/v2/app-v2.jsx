// v2 main app — mosaic outliner with terminal-mono chrome
const { useState: useStateApp, useEffect: useEffectApp } = React;

const TWEAK_DEFAULTS_V2 = /*EDITMODE-BEGIN*/{
  "view": "daily",
  "auxTab": "linked",
  "stripe": true
}/*EDITMODE-END*/;

function RailGlyph() {
  return (
    <div className="glyph" title="Tesela">
      <i></i><i></i><i></i><i></i>
    </div>
  );
}

function V2Rail({ view, onView }) {
  const items = [
    { id: "daily", label: "TDY", title: "Today" },
    { id: "tasks", label: "TSK", title: "Tasks" },
    { id: "graph", label: "GPH", title: "Graph" },
    { id: "inbox", label: "INB", title: "Inbox" },
  ];
  return (
    <div className="v2-rail">
      <RailGlyph />
      {items.map((it) => (
        <div
          key={it.id}
          className={`ricon ${view === it.id ? "active" : ""}`}
          title={it.title}
          onClick={() => onView(it.id)}
        >{it.label}</div>
      ))}
      <div className="spacer" />
      <div className="ricon" title="Settings">⚙</div>
    </div>
  );
}

function V2Index({ view, onView }) {
  const D = window.V2Data;
  const [filter, setFilter] = useStateApp("");
  const filtered = D.NOTES_LIST.filter((n) => !filter || n.title.toLowerCase().includes(filter.toLowerCase()));
  return (
    <aside className="v2-index">
      <div className="v2-index-head">
        <div className="crumb">MOSAIC // ~/notes</div>
        <div className="name">tesela</div>
      </div>
      <div className="v2-index-search">
        <span className="ico">⌕</span>
        <input
          placeholder="filter pages…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <kbd>/</kbd>
      </div>

      <div className="v2-index-section"><span>TYPES</span><span>{D.TYPE_NAV.length}</span></div>
      <div>
        {D.TYPE_NAV.map((t) => (
          <div
            key={t.name}
            className={`v2-type-row ${view === "tasks" && t.name === "Task" ? "active" : ""}`}
            onClick={() => t.name === "Task" && onView("tasks")}
          >
            <span className="swatch" style={{ background: t.color }} />
            <span className="label">{t.name}</span>
            <span className="count">{t.count}</span>
          </div>
        ))}
      </div>

      <div className="v2-index-section"><span>PAGES</span><span>{filtered.length}</span></div>
      <div className="v2-index-list">
        {filtered.map((n) => (
          <div
            key={n.id}
            className={`v2-page-row ${n.id === "tesela-2.0" && view !== "daily" && view !== "tasks" ? "active" : ""}`}
          >
            <span className="glyph-letter">{n.letter}</span>
            <span className="label">{n.title}</span>
          </div>
        ))}
      </div>

      <div className="v2-index-foot">
        <span>12 pages</span>
        <span>v0.7.2</span>
      </div>
    </aside>
  );
}

function V2Aux({ tab, onTab, stripe }) {
  const D = window.V2Data;
  return (
    <aside className="v2-aux">
      <div className="v2-aux-tabs">
        <div className={`tab ${tab === "linked" ? "active" : ""}`} onClick={() => onTab("linked")}>
          Linked <span className="badge">{D.BACKLINKS.length}</span>
        </div>
        <div className={`tab ${tab === "outline" ? "active" : ""}`} onClick={() => onTab("outline")}>
          Outline
        </div>
        <div className={`tab ${tab === "props" ? "active" : ""}`} onClick={() => onTab("props")}>
          Meta
        </div>
      </div>
      <div className="v2-aux-body">
        {tab === "linked" && (
          <>
            <div className="v2-aux-section">
              <div className="v2-aux-label">
                <span>Backlinks</span>
                <span className="num">{D.BACKLINKS.length} ↩</span>
              </div>
              {D.BACKLINKS.map((b, i) => (
                <div key={i} className="v2-bl-card" data-type={b.type}>
                  <div className="src">
                    {b.source}
                    <span className="type-badge">{b.type}</span>
                  </div>
                  <div className="snippet" dangerouslySetInnerHTML={{ __html: b.snippet }} />
                </div>
              ))}
            </div>

            <div className="v2-aux-section">
              <div className="v2-aux-label">
                <span>Mentions</span>
                <span className="num">2 ↪</span>
              </div>
              <div className="v2-bl-card" data-type="task">
                <div className="src">today.md<span className="type-badge">task</span></div>
                <div className="snippet">→ <mark>Tesela 2.0</mark> · <mark>Claire Rodriguez</mark></div>
              </div>
            </div>
          </>
        )}

        {tab === "outline" && (
          <div className="v2-aux-section">
            <div className="v2-aux-label">
              <span>Page outline</span>
              <span className="num">7 blocks</span>
            </div>
            {D.OUTLINE.map((o, i) => (
              <div key={i} className="v2-outline-link" style={{ paddingLeft: 6 + o.d * 14 }}>
                <span className="num">{o.num}</span>
                <span>{o.label}</span>
              </div>
            ))}
          </div>
        )}

        {tab === "props" && (
          <div className="v2-aux-section">
            <div className="v2-aux-label">
              <span>Frontmatter</span>
              <span className="num">{D.PAGE_PROPS.length}</span>
            </div>
            {D.PAGE_PROPS.map((p) => (
              <div key={p.k} className="v2-prop-row">
                <span className="k">{p.k}</span>
                <span className="v editable">{p.v}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </aside>
  );
}

function V2TagTable() {
  const D = window.V2Data;
  return (
    <div className="v2-canvas">
      <div className="v2-canvas-inner" style={{ maxWidth: 1100 }}>
        <div className="v2-tag-head">
          <span className="marker" />
          <h1>All <em>#Task</em> blocks</h1>
          <span className="meta">{D.TASK_ROWS.length} results · across 5 pages</span>
        </div>
        <div className="v2-querybar">
          <span><span className="tok-key">type:</span> <span className="tok-tag">Task</span></span>
          <span><span className="tok-key">where</span> <span className="tok-prop">status</span> ≠ <span className="tok-val">done</span></span>
          <span><span className="tok-key">order</span> <span className="tok-prop">priority</span> desc, <span className="tok-prop">deadline</span> asc</span>
        </div>
        <table className="v2-table">
          <thead>
            <tr>
              <th style={{ width: "44%" }}>Block</th>
              <th>Status</th>
              <th>Priority</th>
              <th>Deadline</th>
            </tr>
          </thead>
          <tbody>
            {D.TASK_ROWS.map((r, i) => (
              <tr key={i}>
                <td>
                  <div className="tt-text">{r.text}</div>
                  <div className="tt-source">↳ {r.source}</div>
                </td>
                <td>
                  <span className={`pchip s-${r.status}`} style={{ display: "inline-flex" }}>
                    <span className="k">·</span>
                    <span className="v">{r.status}</span>
                  </span>
                </td>
                <td>
                  <span className={`pchip p-${r.priority}`} style={{ display: "inline-flex" }}>
                    <span className="k">·</span>
                    <span className="v">{r.priority}</span>
                  </span>
                </td>
                <td style={{ fontFamily: "var(--v2-mono)", fontSize: 11.5 }}>{r.deadline}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function V2App() {
  const D = window.V2Data;
  const [t, setTweak] = window.useTweaks(TWEAK_DEFAULTS_V2);
  const [focusedBlock, setFocusedBlock] = useStateApp("b2b");
  const [vimMode, setVimMode] = useStateApp("NORMAL");

  const view = t.view || "daily";

  return (
    <div className="v2-app">
      <div className="v2-body">
        <V2Rail view={view} onView={(v) => setTweak("view", v)} />
        <V2Index view={view} onView={(v) => setTweak("view", v)} />

        <main className="v2-main">
          {view === "daily" && (
            <>
              <div className="v2-main-head">
                <div className="breadcrumb">
                  <span>daily</span>
                  <span className="sep">/</span>
                  <span>2026</span>
                  <span className="sep">/</span>
                  <span>04</span>
                  <span className="sep">/</span>
                  <span className="now">24 — friday</span>
                </div>
                <div className="actions">
                  <div className="iconbtn" title="Yesterday">‹</div>
                  <div className="iconbtn" title="Today">●</div>
                  <div className="iconbtn" title="Tomorrow">›</div>
                </div>
              </div>
              <div className="v2-canvas">
                <div className="v2-canvas-inner">
                  <div className="v2-page-head">
                    <div>
                      <div className="v2-page-eyebrow">
                        <span className="stamp">DAILY</span>
                        <span className="marker" />
                        <span>{D.TODAY_STR}</span>
                      </div>
                      <h1 className="v2-page-title">A quiet Friday for <em>shipping search.</em></h1>
                    </div>
                    <div className="v2-page-stats">
                      <div className="row"><span className="num">12</span><span>blocks</span></div>
                      <div className="row"><span className="num">04</span><span>tasks</span></div>
                      <div className="row"><span className="num">04</span><span>links in</span></div>
                    </div>
                  </div>

                  <window.V2Outliner blocks={D.DAILY_BLOCKS} focusedId={focusedBlock} onFocus={setFocusedBlock} />

                  <div className="v2-add-block">
                    <span>+ add block at indent 0</span>
                    <div className="keys">
                      <kbd>o</kbd><span>below</span>
                      <kbd>O</kbd><span>above</span>
                      <kbd>tab</kbd><span>indent</span>
                    </div>
                  </div>
                </div>
              </div>
            </>
          )}

          {view === "tasks" && (
            <>
              <div className="v2-main-head">
                <div className="breadcrumb">
                  <span>types</span>
                  <span className="sep">/</span>
                  <span className="now">Task</span>
                  <span className="sep">/</span>
                  <span>query</span>
                </div>
                <div className="actions">
                  <div className="iconbtn" title="Save query">★</div>
                  <div className="iconbtn" title="Settings">⚙</div>
                </div>
              </div>
              <V2TagTable />
            </>
          )}

          {view !== "daily" && view !== "tasks" && (
            <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--v2-fg-dim)" }}>
              <div style={{ textAlign: "center" }}>
                <div style={{ fontFamily: "var(--v2-display)", fontSize: 32, color: "var(--v2-fg)" }}>
                  {view === "graph" ? "graph" : "inbox"}
                </div>
                <div style={{ fontFamily: "var(--v2-mono)", fontSize: 11, marginTop: 6 }}>
                  out of scope for this redesign — try TDY or TSK
                </div>
              </div>
            </div>
          )}
        </main>

        <V2Aux tab={t.auxTab} onTab={(v) => setTweak("auxTab", v)} stripe={t.stripe} />
      </div>

      <div className="v2-cmdrail">
        <span className={`vim ${vimMode.toLowerCase()}`}>{vimMode}</span>
        <span className="doc">~/notes/dailies/2026-04-24.md</span>
        <span className="pos">blk 4/12 · ln 18 · col 32</span>
        <span className="saved">● saved 12s ago</span>
        <span className="spacer" />
        <span className="key"><kbd>SPC</kbd> leader</span>
        <span className="key"><kbd>⌘K</kbd> palette</span>
        <span className="key"><kbd>g</kbd><kbd>d</kbd> daily</span>
        <span className="key"><kbd>g</kbd><kbd>t</kbd> tasks</span>
      </div>

      <window.TweaksPanel title="Tweaks">
        <window.TweakSection label="View" />
        <window.TweakRadio
          label="Screen"
          value={t.view}
          onChange={(v) => setTweak("view", v)}
          options={[
            { value: "daily", label: "Daily" },
            { value: "tasks", label: "Tasks" },
          ]}
        />
        <window.TweakSection label="Right panel" />
        <window.TweakRadio
          label="Tab"
          value={t.auxTab}
          onChange={(v) => setTweak("auxTab", v)}
          options={[
            { value: "linked", label: "Linked" },
            { value: "outline", label: "Outline" },
            { value: "props", label: "Meta" },
          ]}
        />
        <window.TweakSection label="Notes" />
        <div style={{ fontSize: 11, color: "var(--v2-fg-muted)", lineHeight: 1.5, fontFamily: "var(--v2-mono)" }}>
          v2 commits to a bold direction:
          terminal-mono chrome, type-driven color stripes,
          mosaic tiles with grout. Compare with v1 in the design canvas.
        </div>
      </window.TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("v2-root")).render(<V2App />);

// Sidebar — left navigation
const { useState } = React;

function Sidebar({ collapsed, onToggle, currentView, onNavigate, onOpenPalette }) {
  const I = window.Icons;
  const [filter, setFilter] = useState("");
  const filtered = window.MockData.NOTES_LIST.filter((n) =>
    n.title.toLowerCase().includes(filter.toLowerCase())
  );
  const recent = window.MockData.RECENT_IDS
    .map((id) => window.MockData.NOTES_LIST.find((n) => n.id === id))
    .filter(Boolean);

  if (collapsed) {
    return (
      <div className="sidebar-collapsed-rail">
        <button className="icon-btn" onClick={onToggle} title="Expand sidebar (1)">
          <I.ChevRight />
        </button>
        <div style={{ width: 16, height: 1, background: "var(--border)" }} />
        <button className="icon-btn" onClick={onOpenPalette} title="Search (⌘K)">
          <I.Search />
        </button>
        <button className="icon-btn" onClick={() => onNavigate("daily")} title="Today">
          <I.Sun />
        </button>
        <button className="icon-btn" onClick={() => onNavigate("tasks")} title="Tasks">
          <I.Tag />
        </button>
        <button className="icon-btn" title="Graph">
          <I.Graph />
        </button>
      </div>
    );
  }

  const navItems = [
    { id: "daily", label: "Today", icon: <I.Sun />, badge: "Apr 24" },
    { id: "timeline", label: "Timeline", icon: <I.Clock /> },
    { id: "graph", label: "Graph", icon: <I.Graph /> },
    { id: "inbox", label: "Inbox", icon: <I.Inbox />, badge: "3" },
  ];

  return (
    <div className="sidebar">
      <div className="brand">
        <div className="brand-mark">
          <span className="glyph" />
          <span>Tesela</span>
        </div>
        <button className="icon-btn" onClick={onToggle} title="Collapse (1)">
          <I.ChevLeft />
        </button>
      </div>

      <div className="sidebar-search" onClick={onOpenPalette}>
        <I.Search size={13} className="search-icon" />
        <input
          placeholder="Search notes…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onFocus={(e) => e.target.blur() || onOpenPalette()}
          readOnly
        />
        <kbd>⌘K</kbd>
      </div>

      {/* Quick nav */}
      <div style={{ padding: "0 8px 4px" }}>
        {navItems.map((it) => (
          <div
            key={it.id}
            className={`nav-item ${currentView === it.id ? "active" : ""}`}
            onClick={() => onNavigate(it.id)}
          >
            <span className="ico">{it.icon}</span>
            <span className="label">{it.label}</span>
            {it.badge && <span className="badge">{it.badge}</span>}
          </div>
        ))}
      </div>

      {/* Tag nav (typed-block queries) */}
      <div className="sidebar-section-label">
        <span className="dot" /> Types
      </div>
      <div style={{ padding: "0 8px" }}>
        {window.MockData.TAG_NAV.map((t) => (
          <div
            key={t.name}
            className={`nav-item ${currentView === "tasks" && t.name === "Task" ? "active" : ""}`}
            onClick={() => t.name === "Task" && onNavigate("tasks")}
          >
            <span className={`tag-dot ${t.kind}`} />
            <span className="label">{t.name}</span>
            <span className="badge">{t.count}</span>
          </div>
        ))}
      </div>

      {/* Recents */}
      <div className="sidebar-section-label">
        <span className="dot" /> Recent
      </div>
      <div className="sidebar-nav" style={{ flex: "0 0 auto", maxHeight: 160 }}>
        {recent.map((n) => (
          <div key={n.id} className="nav-item">
            <span className="ico" style={{ opacity: 0.6 }}>
              <I.File />
            </span>
            <span className="label">{n.title}</span>
          </div>
        ))}
      </div>

      {/* Pages */}
      <div className="sidebar-section-label">
        <span className="dot" /> Pages
      </div>
      <div className="sidebar-nav">
        {filtered.map((n) => (
          <div key={n.id} className="nav-item">
            <span className="ico" style={{ opacity: 0.5 }}>
              <I.File />
            </span>
            <span className="label">{n.title}</span>
          </div>
        ))}
      </div>

      <div className="sidebar-footer">
        <div className="nav-item" style={{ padding: "4px 8px", margin: 0, fontSize: 11.5 }}>
          <span className="ico"><I.Settings /></span>
          <span className="label">Settings</span>
        </div>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: 10 }}>
          {window.MockData.NOTES_LIST.length} notes
        </span>
      </div>
    </div>
  );
}

window.Sidebar = Sidebar;

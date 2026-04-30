// Leader menu — appears after pressing Space in normal mode
function LeaderMenu({ onClose }) {
  const I = window.Icons;
  const groups = [
    {
      label: "n — note",
      items: [
        { k: "n", d: "New note" },
        { k: "d", d: "Open daily note" },
        { k: "r", d: "Rename current" },
        { k: "x", d: "Delete current" },
      ],
    },
    {
      label: "f — find",
      items: [
        { k: "f", d: "Find note" },
        { k: "/", d: "Full-text search" },
        { k: "t", d: "Filter by tag" },
        { k: "b", d: "Backlinks panel" },
      ],
    },
    {
      label: "b — block",
      items: [
        { k: "i", d: "Indent block" },
        { k: "o", d: "Outdent block" },
        { k: "z", d: "Zoom into block" },
        { k: "y", d: "Yank block" },
      ],
    },
    {
      label: "v — view",
      items: [
        { k: "1", d: "Toggle sidebar" },
        { k: "2", d: "Toggle context rail" },
        { k: "g", d: "Open graph" },
        { k: "T", d: "Toggle theme" },
      ],
    },
  ];
  return (
    <div className="leader-overlay" onClick={onClose}>
      <div className="leader" onClick={(e) => e.stopPropagation()}>
        <div className="leader-header">
          <span className="key">Space</span>
          <span>Leader menu — pick a category</span>
          <span style={{ marginLeft: "auto" }}>
            <kbd className="kbd">esc</kbd> close
          </span>
        </div>
        <div className="leader-grid">
          {groups.map((g) => (
            <div key={g.label}>
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 10.5,
                  textTransform: "uppercase",
                  letterSpacing: "0.12em",
                  color: "var(--muted-foreground)",
                  margin: "10px 8px 4px",
                }}
              >
                {g.label}
              </div>
              {g.items.map((it) => (
                <div key={it.k} className="leader-row">
                  <span className="lkey">{it.k}</span>
                  <span className="ldesc">{it.d}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

window.LeaderMenu = LeaderMenu;

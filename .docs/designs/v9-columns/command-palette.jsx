// Command palette
const { useState: useCpState, useEffect: useCpEffect } = React;

function CommandPalette({ open, onClose, onNavigate }) {
  const I = window.Icons;
  const [q, setQ] = useCpState("");
  const [sel, setSel] = useCpState(0);

  useCpEffect(() => {
    setQ("");
    setSel(0);
  }, [open]);

  if (!open) return null;

  const cmds = [
    { id: "today", label: "Go to today", icon: <I.Sun />, kbd: "g d", action: () => onNavigate("daily") },
    { id: "tasks", label: "All tasks", icon: <I.Hash />, kbd: "g t", action: () => onNavigate("tasks") },
    { id: "graph", label: "Graph", icon: <I.Graph />, kbd: "g g" },
    { id: "newnote", label: "New note", icon: <I.Plus />, kbd: "⌘N" },
    { id: "newtype", label: "New type page", icon: <I.Tag /> },
    { id: "theme", label: "Toggle theme (Day / Evening)", icon: <I.Moon />, kbd: "⇧⌘L" },
  ];
  const recent = window.MockData.RECENT_IDS
    .map((id) => window.MockData.NOTES_LIST.find((n) => n.id === id))
    .filter(Boolean);

  const ql = q.toLowerCase();
  const matchedCmds = q ? cmds.filter((c) => c.label.toLowerCase().includes(ql)) : cmds;
  const matchedNotes = q
    ? window.MockData.NOTES_LIST.filter((n) => n.title.toLowerCase().includes(ql)).slice(0, 6)
    : recent.slice(0, 4);

  const flat = [
    ...matchedCmds.map((c) => ({ kind: "cmd", item: c })),
    ...matchedNotes.map((n) => ({
      kind: "note",
      item: {
        id: n.id,
        label: n.title,
        sub: n.tags.join(", "),
        action: () => {},
      },
    })),
  ];

  function onKey(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSel((s) => Math.min(flat.length - 1, s + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSel((s) => Math.max(0, s - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const cur = flat[sel];
      if (cur && cur.item.action) cur.item.action();
      onClose();
    }
  }

  return (
    <div className="palette-overlay" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <div className="palette-input">
          <I.Search size={15} style={{ color: "var(--muted-foreground)" }} />
          <input
            autoFocus
            placeholder="Search commands, notes, or content…"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={onKey}
          />
          <span className="esc">esc</span>
        </div>
        <div className="palette-body">
          {matchedCmds.length > 0 && (
            <>
              <div className="palette-section-label">{q ? "Actions" : "Quick Actions"}</div>
              {matchedCmds.map((c, i) => {
                const idx = i;
                return (
                  <div
                    key={c.id}
                    className={`palette-row ${sel === idx ? "selected" : ""}`}
                    onMouseEnter={() => setSel(idx)}
                    onClick={() => {
                      if (c.action) c.action();
                      onClose();
                    }}
                  >
                    <span className="pico">{c.icon}</span>
                    <span className="ptext">{c.label}</span>
                    {c.kbd && <span className="pkbd">{c.kbd}</span>}
                  </div>
                );
              })}
            </>
          )}
          {matchedNotes.length > 0 && (
            <>
              <div className="palette-section-label">{q ? "Notes" : "Recent"}</div>
              {matchedNotes.map((n, i) => {
                const idx = matchedCmds.length + i;
                return (
                  <div
                    key={n.id}
                    className={`palette-row ${sel === idx ? "selected" : ""}`}
                    onMouseEnter={() => setSel(idx)}
                    onClick={onClose}
                  >
                    <span className="pico"><I.File /></span>
                    <span className="ptext">{n.title}</span>
                    {n.tags.length > 0 && <span className="psub">{n.tags.join(", ")}</span>}
                  </div>
                );
              })}
            </>
          )}
          {q && matchedCmds.length + matchedNotes.length === 0 && (
            <>
              <div className="palette-section-label">Create</div>
              <div className="palette-row selected">
                <span className="pico" style={{ color: "var(--primary)" }}><I.Plus /></span>
                <span className="ptext">Create note "{q}"</span>
              </div>
              <div className="palette-row">
                <span className="pico"><I.Tag /></span>
                <span className="ptext">Create type "{q}"</span>
              </div>
            </>
          )}
        </div>
        <div className="palette-footer">
          <span><kbd>↑↓</kbd> navigate</span>
          <span><kbd>↵</kbd> select</span>
          <span><kbd>⇥</kbd> filter type</span>
          <span style={{ marginLeft: "auto" }}><kbd>esc</kbd> close</span>
        </div>
      </div>
    </div>
  );
}

window.CommandPalette = CommandPalette;

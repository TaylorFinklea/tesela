// Outliner — daily note view
const { useState: useOlState, useRef: useOlRef, useEffect: useOlEffect } = React;

function renderInlineText(text) {
  // Render #Tag, [[wikilink]], [[wikilink|alias]] inline
  const parts = [];
  let rest = text;
  let key = 0;
  const regex = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_-]*))/g;
  let lastIndex = 0;
  let m;
  while ((m = regex.exec(text)) !== null) {
    if (m.index > lastIndex) {
      parts.push(<span key={key++}>{text.slice(lastIndex, m.index)}</span>);
    }
    if (m[1]) {
      const label = m[3] || m[2];
      parts.push(
        <span key={key++} className="wikilink">
          {label}
        </span>
      );
    } else if (m[4]) {
      const tag = m[5].toLowerCase();
      const cls = ["task", "project", "person", "domain", "issue", "ritual"].includes(tag) ? tag : "";
      parts.push(
        <span key={key++} className={`tag ${cls}`}>
          #{m[5]}
        </span>
      );
    }
    lastIndex = regex.lastIndex;
  }
  if (lastIndex < text.length) {
    parts.push(<span key={key++}>{text.slice(lastIndex)}</span>);
  }
  return parts;
}

function PropChip({ k, v }) {
  const cls = `prop-chip ${k === "status" ? `status-${v}` : ""} ${k === "priority" ? `priority-${v}` : ""}`;
  return (
    <span className={cls}>
      <span className="k">{k}</span>
      <span style={{ opacity: 0.4 }}>::</span>
      <span className="v">{v}</span>
    </span>
  );
}

function Outliner({ blocks, focusedId, onFocus }) {
  const I = window.Icons;
  return (
    <div className="outliner">
      {blocks.map((b, i) => {
        const next = blocks[i + 1];
        const hasChildren = next && next.indent > b.indent;
        const focused = focusedId === b.id;
        return (
          <div
            key={b.id}
            className={`block-row ${hasChildren ? "has-children" : ""} ${focused ? "cursor" : ""}`}
            onClick={() => onFocus(b.id)}
          >
            {/* Indent + thread lines */}
            {b.indent > 0 && (
              <div className="indent-spacer" style={{ width: `calc(var(--indent-step) * ${b.indent})` }}>
                {Array.from({ length: b.indent }).map((_, lvl) => (
                  <span
                    key={lvl}
                    className="thread-line"
                    style={{ left: `calc(var(--indent-step) * ${lvl} + 14px)` }}
                  />
                ))}
              </div>
            )}

            <div className={`block-card ${focused ? "focused" : ""}`}>
              <div className="block-handle">
                <span className="grip"><I.Grip size={12} /></span>
              </div>
              <div className="block-bullet" />
              <div className="block-content">
                <div className="block-text">{renderInlineText(b.text)}</div>
                {b.props && Object.keys(b.props).length > 0 && (
                  <div className="block-properties">
                    {Object.entries(b.props).map(([k, v]) => (
                      <PropChip key={k} k={k} v={v} />
                    ))}
                  </div>
                )}
              </div>
              {b.meta && (
                <div className="block-meta">
                  <span>{b.meta}</span>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

window.Outliner = Outliner;

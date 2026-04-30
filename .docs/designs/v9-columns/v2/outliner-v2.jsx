// v2 — Bold mosaic outliner. Every block a tile, the page a frame.
const { useState: useStateV2, useMemo: useMemoV2 } = React;

// Parse a block's text for #Tag and [[link]] markup
function renderText(text) {
  const parts = [];
  const re = /(\[\[[^\]]+\]\]|#[A-Za-z][A-Za-z0-9_]*)/g;
  let last = 0, m, key = 0;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parts.push(<span key={key++}>{text.slice(last, m.index)}</span>);
    const tok = m[0];
    if (tok.startsWith("[[")) {
      const inner = tok.slice(2, -2);
      const [target, label] = inner.includes("|") ? inner.split("|") : [inner, inner];
      parts.push(<span key={key++} className="v2link">{label}</span>);
    } else {
      const tag = tok.slice(1).toLowerCase();
      const known = ["task", "project", "person", "issue", "ritual", "domain"];
      const cls = known.includes(tag) ? `v2tag ${tag}` : "v2tag";
      parts.push(<span key={key++} className={cls}>{tok}</span>);
    }
    last = re.lastIndex;
  }
  if (last < text.length) parts.push(<span key={key++}>{text.slice(last)}</span>);
  return parts;
}

function V2Outliner({ blocks, focusedId, onFocus }) {
  return (
    <div className="v2-mosaic">
      {blocks.map((b, i) => {
        const isFocused = b.id === focusedId;
        const indentPx = b.indent * 24;
        const isDone = b.props && b.props.status === "done";
        return (
          <div
            key={b.id}
            className={`v2-tile-row ${isFocused ? "focused" : ""}`}
            data-type={b.type || "none"}
            data-done={isDone ? "true" : "false"}
            style={{ "--col-indent": `${indentPx}px` }}
            onClick={() => onFocus(b.id)}
          >
            <div className="v2-tile-indent">
              {b.indent > 0 && (
                <div className="threads">
                  {Array.from({ length: b.indent }).map((_, j) => <span key={j} />)}
                </div>
              )}
            </div>
            <div className="v2-tile-stripe" />
            <div className="v2-tile-content">
              <div className="v2-tile-text">{renderText(b.text)}</div>
              {b.props && Object.keys(b.props).length > 0 && (
                <div className="v2-tile-props">
                  {Object.entries(b.props).map(([k, v]) => {
                    let cls = "pchip";
                    if (k === "status") cls += ` s-${v}`;
                    if (k === "priority") cls += ` p-${v}`;
                    return (
                      <span key={k} className={cls}>
                        <span className="k">{k}</span>
                        <span className="v">{v}</span>
                      </span>
                    );
                  })}
                </div>
              )}
            </div>
            <div className="v2-tile-meta">
              {b.meta && <span className="timestamp">{b.meta.includes("[[") ? renderText(b.meta) : b.meta}</span>}
            </div>
          </div>
        );
      })}
    </div>
  );
}

window.V2Outliner = V2Outliner;

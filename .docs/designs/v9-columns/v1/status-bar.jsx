// Status bar — bottom
function StatusBar({ vimMode = "NORMAL", noteName, blockPos }) {
  const I = window.Icons;
  return (
    <div className="status-bar">
      <span className={`vim-mode ${vimMode.toLowerCase()}`}>{vimMode}</span>
      <span style={{ opacity: 0.7 }}>{noteName}</span>
      <span className="pos">{blockPos}</span>
      <span className="saved">
        <span style={{ display: "inline-block", width: 5, height: 5, borderRadius: "50%", background: "oklch(58% 0.10 150)", marginRight: 6, verticalAlign: "1px" }} />
        saved · 11:43
      </span>
      <span className="spacer" />
      <span className="hint"><kbd>Space</kbd> leader</span>
      <span className="hint"><kbd>⌘K</kbd> palette</span>
      <span className="hint"><kbd>1</kbd> sidebar</span>
      <span className="hint"><kbd>2</kbd> rail</span>
      <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
        <span className="dot" /> connected
      </span>
    </div>
  );
}

window.StatusBar = StatusBar;

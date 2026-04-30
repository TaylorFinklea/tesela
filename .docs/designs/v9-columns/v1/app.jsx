// Main app shell, theme system, view switcher, tweaks wiring
const { useState, useEffect } = React;

// Theme palettes — mirrored from web/src/lib/themes.ts
const THEMES = {
  day: {
    name: "Day", isDark: false,
    vars: {
      "--background": "#faf7f2", "--foreground": "#2c2824", "--foreground-muted": "#5e5650",
      "--surface": "#f3ede4", "--surface-2": "#ebe4d8",
      "--muted": "#e8e0d4", "--muted-foreground": "#8a8078",
      "--accent": "#f0e8dc",
      "--primary": "#c4852c", "--primary-soft": "rgba(196,133,44,0.12)", "--primary-foreground": "#faf7f2",
      "--destructive": "#c45a4a", "--border": "#e8e0d4", "--border-soft": "rgba(120,90,50,0.08)",
      "--popover": "#fffdf8",
      "--block-bg": "#ffffff", "--block-border": "#e8e0d4",
      "--block-shadow": "0 1px 2px rgba(120,90,50,0.04), 0 1px 3px rgba(120,90,50,0.06)",
      "--focus-glow": "0 0 0 3px rgba(196,133,44,0.14)", "--thread-border": "#e0d6c4",
      "--tag-bg": "rgba(196,133,44,0.10)", "--tag-fg": "#8a5e1c",
      "--link-fg": "#6a4ec4", "--link-bg": "rgba(106,78,196,0.08)",
    },
  },
  evening: {
    name: "Evening", isDark: true,
    vars: {
      "--background": "#1e1c24", "--foreground": "#e8e0d4", "--foreground-muted": "#c4baa8",
      "--surface": "#17151c", "--surface-2": "#24222c",
      "--muted": "#2a2830", "--muted-foreground": "#9a918a",
      "--accent": "#2a2830",
      "--primary": "#d4a04a", "--primary-soft": "rgba(212,160,74,0.16)", "--primary-foreground": "#1e1c24",
      "--destructive": "#d46a5a", "--border": "rgba(255,240,210,0.08)", "--border-soft": "rgba(255,240,210,0.06)",
      "--popover": "#24222c",
      "--block-bg": "#24222c", "--block-border": "rgba(255,240,210,0.06)",
      "--block-shadow": "0 1px 3px rgba(0,0,0,0.3)",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.18)", "--thread-border": "rgba(255,240,210,0.10)",
      "--tag-bg": "rgba(212,160,74,0.14)", "--tag-fg": "#d4a04a",
      "--link-fg": "#b6a0e8", "--link-bg": "rgba(150,120,220,0.14)",
    },
  },
  woven: {
    name: "Woven", isDark: true,
    vars: {
      "--background": "#1a1822", "--foreground": "#d8cce8", "--foreground-muted": "#b6a8d0",
      "--surface": "#14121a", "--surface-2": "#201e28",
      "--muted": "#282630", "--muted-foreground": "#8a80a0",
      "--accent": "#282630",
      "--primary": "#d4a04a", "--primary-soft": "rgba(212,160,74,0.14)", "--primary-foreground": "#1a1822",
      "--destructive": "#d46a5a", "--border": "rgba(200,180,240,0.06)", "--border-soft": "rgba(200,180,240,0.06)",
      "--popover": "#201e28",
      "--block-bg": "transparent", "--block-border": "transparent",
      "--block-shadow": "none",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.14)", "--thread-border": "rgba(200,180,240,0.16)",
      "--tag-bg": "rgba(212,160,74,0.14)", "--tag-fg": "#d4a04a",
      "--link-fg": "#c8b6f0", "--link-bg": "rgba(180,150,240,0.12)",
    },
  },
  "tile-grid": {
    name: "Tile Grid", isDark: true,
    vars: {
      "--background": "#141218", "--foreground": "#e4e0ea", "--foreground-muted": "#beb6c6",
      "--surface": "#100e14", "--surface-2": "#1c1a22",
      "--muted": "#24222a", "--muted-foreground": "#8a86a0",
      "--accent": "#1c1a22",
      "--primary": "#d4a04a", "--primary-soft": "rgba(212,160,74,0.16)", "--primary-foreground": "#141218",
      "--destructive": "#d46a5a", "--border": "rgba(255,240,210,0.05)", "--border-soft": "rgba(255,240,210,0.05)",
      "--popover": "#1c1a22",
      "--block-bg": "#1c1a22", "--block-border": "rgba(255,240,210,0.06)",
      "--block-shadow": "0 2px 6px rgba(0,0,0,0.25)",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.18)", "--thread-border": "rgba(255,240,210,0.08)",
      "--tag-bg": "rgba(212,160,74,0.14)", "--tag-fg": "#d4a04a",
      "--link-fg": "#b6a0e8", "--link-bg": "rgba(150,120,220,0.12)",
    },
  },
};

function applyTheme(id) {
  const t = THEMES[id] || THEMES.day;
  const root = document.documentElement;
  Object.entries(t.vars).forEach(([k, v]) => root.style.setProperty(k, v));
  if (t.isDark) document.body.classList.add("dark");
  else document.body.classList.remove("dark");
}

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "day",
  "density": "default",
  "block": "tile",
  "thread": "on",
  "view": "daily",
  "rail": true,
  "railTab": "backlinks"
}/*EDITMODE-END*/;

function App() {
  const I = window.Icons;
  const [t, setTweak] = window.useTweaks(TWEAK_DEFAULTS);
  const setT = (obj) => Object.entries(obj).forEach(([k, v]) => setTweak(k, v));
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [leaderOpen, setLeaderOpen] = useState(false);
  const [focusedBlock, setFocusedBlock] = useState("b2b");
  const [vimMode, setVimMode] = useState("NORMAL");

  // Apply theme + density + block treatment
  useEffect(() => {
    applyTheme(t.theme);
    document.body.setAttribute("data-density", t.density);
    document.body.setAttribute("data-block", t.block);
    document.body.setAttribute("data-thread", t.thread);
  }, [t.theme, t.density, t.block, t.thread]);

  // Global keyboard shortcuts
  useEffect(() => {
    function onKey(e) {
      const tag = (e.target.tagName || "").toLowerCase();
      const editing = tag === "input" || tag === "textarea" || e.target.isContentEditable;
      if (editing) return;
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      } else if (e.key === " " && !paletteOpen && !leaderOpen) {
        e.preventDefault();
        setLeaderOpen(true);
      } else if (e.key === "Escape") {
        setPaletteOpen(false);
        setLeaderOpen(false);
      } else if (e.key === "1") {
        setSidebarCollapsed((v) => !v);
      } else if (e.key === "2") {
        setT({ rail: !t.rail });
      } else if (e.key === "/" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        setPaletteOpen(true);
      } else if (e.key === "i") {
        setVimMode("INSERT");
      } else if (e.key === "v") {
        setVimMode("VISUAL");
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [paletteOpen, leaderOpen, t.rail]);

  const D = window.MockData;
  const view = t.view || "daily";
  const showRail = t.rail && view === "daily";

  return (
    <div className="app">
      <div className={`app-body ${showRail ? "with-rail" : "no-rail"} ${sidebarCollapsed ? "sidebar-collapsed" : ""}`}>
        <window.Sidebar
          collapsed={sidebarCollapsed}
          onToggle={() => setSidebarCollapsed((v) => !v)}
          currentView={view}
          onNavigate={(id) => setT({ view: id })}
          onOpenPalette={() => setPaletteOpen(true)}
        />

        <main className="main">
          {view === "daily" && (
            <>
              <div className="main-header">
                <div className="crumbs">
                  <span className="crumb">Daily</span>
                  <span className="sep">›</span>
                  <span className="crumb">2026</span>
                  <span className="sep">›</span>
                  <span className="crumb">April</span>
                  <span className="sep">›</span>
                  <span className="crumb current">Friday 24</span>
                </div>
                <div className="header-actions">
                  <button className="icon-btn" title="Yesterday"><I.ChevLeft /></button>
                  <button className="icon-btn" title="Today" onClick={() => {}}>
                    <I.Sun />
                  </button>
                  <button className="icon-btn" title="Tomorrow"><I.ChevRight /></button>
                  <span style={{ width: 8 }} />
                  <button
                    className="icon-btn"
                    title="Toggle context rail (2)"
                    onClick={() => setT({ rail: !t.rail })}
                    style={{ color: t.rail ? "var(--primary)" : "var(--muted-foreground)" }}
                  >
                    <I.Layers />
                  </button>
                </div>
              </div>

              <div className="note-canvas">
                <div className="note-canvas-inner">
                  <div className="page-meta">
                    <div className="page-eyebrow">
                      <span className="pill">Daily</span>
                      <span>{D.TODAY_STR}</span>
                      <span style={{ opacity: 0.5 }}>·</span>
                      <span>12 blocks · 184 words</span>
                    </div>
                    <h1 className="page-title">A quiet Friday for shipping search.</h1>
                    <div className="page-properties">
                      <div className="page-property">
                        <span className="k">type</span>
                        <span className="v">Daily</span>
                      </div>
                      <div className="page-property">
                        <span className="k">tags</span>
                        <span className="v">daily, planning</span>
                      </div>
                      <div className="page-property">
                        <span className="k">created</span>
                        <span className="v">08:12</span>
                      </div>
                      <div className="page-property">
                        <span className="k">linked</span>
                        <span className="v">4 backlinks</span>
                      </div>
                    </div>
                  </div>

                  <window.Outliner
                    blocks={D.DAILY_BLOCKS}
                    focusedId={focusedBlock}
                    onFocus={setFocusedBlock}
                  />

                  <div style={{ height: 24 }} />
                  <div
                    style={{
                      display: "flex", alignItems: "center", gap: 8,
                      padding: "8px 12px", borderRadius: 8,
                      color: "var(--muted-foreground)", fontSize: 13,
                      cursor: "text",
                    }}
                  >
                    <I.Plus size={14} /> Add a block — press <span className="kbd">o</span> below or <span className="kbd">O</span> above
                  </div>
                </div>
              </div>
            </>
          )}

          {view === "tasks" && (
            <>
              <div className="main-header">
                <div className="crumbs">
                  <span className="crumb">Types</span>
                  <span className="sep">›</span>
                  <span className="crumb current">Task</span>
                </div>
                <div className="header-actions">
                  <button className="icon-btn" title="Save query"><I.Star /></button>
                  <button className="icon-btn"><I.Settings /></button>
                </div>
              </div>
              <window.TagTable />
            </>
          )}

          {view !== "daily" && view !== "tasks" && (
            <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--muted-foreground)" }}>
              <div style={{ textAlign: "center" }}>
                <div style={{ fontFamily: "var(--font-display)", fontSize: 28, fontWeight: 500, color: "var(--foreground)" }}>
                  {view === "timeline" && "Timeline"}
                  {view === "graph" && "Graph"}
                  {view === "inbox" && "Inbox"}
                </div>
                <div style={{ marginTop: 6, fontSize: 13 }}>
                  Out of scope for this redesign pass — try Today or the Task view.
                </div>
              </div>
            </div>
          )}
        </main>

        {showRail && (
          <window.ContextRail
            activeTab={t.railTab}
            onTabChange={(tab) => setT({ railTab: tab })}
          />
        )}
      </div>

      <window.StatusBar
        vimMode={vimMode}
        noteName={view === "daily" ? "Daily — 2026-04-24" : view === "tasks" ? "#Task query" : view}
        blockPos={view === "daily" ? "blk 4 / 12" : ""}
      />

      <window.CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onNavigate={(id) => setT({ view: id })}
      />
      {leaderOpen && <window.LeaderMenu onClose={() => setLeaderOpen(false)} />}

      {/* Tweaks panel */}
      <window.TweaksPanel title="Tweaks">
        <window.TweakSection label="Theme" />
        <window.TweakRadio
          label="Palette"
          value={t.theme}
          onChange={(v) => setTweak("theme", v)}
          options={[
            { value: "day", label: "Day" },
            { value: "evening", label: "Eve" },
            { value: "woven", label: "Woven" },
            { value: "tile-grid", label: "Tile" },
          ]}
        />
        <window.TweakSection label="Block treatment" />
        <window.TweakRadio
          label="Style"
          value={t.block}
          onChange={(v) => setTweak("block", v)}
          options={[
            { value: "tile", label: "Tile" },
            { value: "rule", label: "Rule" },
            { value: "bare", label: "Bare" },
          ]}
        />
        <window.TweakSection label="Density" />
        <window.TweakRadio
          label="Spacing"
          value={t.density}
          onChange={(v) => setTweak("density", v)}
          options={[
            { value: "compact", label: "Compact" },
            { value: "default", label: "Default" },
            { value: "cozy", label: "Cozy" },
          ]}
        />
        <window.TweakSection label="Layout" />
        <window.TweakToggle
          label="Threading lines"
          value={t.thread === "on"}
          onChange={(v) => setTweak("thread", v ? "on" : "off")}
        />
        <window.TweakToggle
          label="Right context rail"
          value={t.rail}
          onChange={(v) => setTweak("rail", v)}
        />
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
      </window.TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);

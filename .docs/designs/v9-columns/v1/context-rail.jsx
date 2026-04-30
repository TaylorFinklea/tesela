// Right context rail: backlinks / properties / outline
const { useState: useRailState } = React;

function ContextRail({ activeTab, onTabChange }) {
  const I = window.Icons;
  const D = window.MockData;

  return (
    <aside className="context-rail">
      <div className="rail-tabs">
        <div
          className={`rail-tab ${activeTab === "backlinks" ? "active" : ""}`}
          onClick={() => onTabChange("backlinks")}
        >
          <I.Link size={13} /> Backlinks
          <span className="count">{D.BACKLINKS.length}</span>
        </div>
        <div
          className={`rail-tab ${activeTab === "properties" ? "active" : ""}`}
          onClick={() => onTabChange("properties")}
        >
          <I.Properties size={13} /> Properties
        </div>
        <div
          className={`rail-tab ${activeTab === "outline" ? "active" : ""}`}
          onClick={() => onTabChange("outline")}
        >
          <I.Outline size={13} /> Outline
        </div>
      </div>

      <div className="rail-body">
        {activeTab === "backlinks" && (
          <>
            <div className="rail-section">
              <div className="rail-section-title">
                <span>Linked from</span>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: 10, opacity: 0.6 }}>
                  {D.BACKLINKS.length}
                </span>
              </div>
              {D.BACKLINKS.map((b, i) => (
                <div key={i} className="backlink-card">
                  <div className="backlink-source">
                    <I.File size={12} className="ico" />
                    {b.source}
                    <span
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 9.5,
                        marginLeft: "auto",
                        opacity: 0.55,
                        textTransform: "uppercase",
                        letterSpacing: "0.10em",
                      }}
                    >
                      {b.icon}
                    </span>
                  </div>
                  <div
                    className="backlink-snippet"
                    dangerouslySetInnerHTML={{ __html: b.snippet }}
                  />
                </div>
              ))}
            </div>
            <div className="rail-section">
              <div className="rail-section-title">
                <span>Unlinked mentions</span>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: 10, opacity: 0.6 }}>
                  2
                </span>
              </div>
              <div className="backlink-card">
                <div className="backlink-source">
                  <I.File size={12} className="ico" />
                  Outliner keymap reference
                </div>
                <div className="backlink-snippet">
                  …leader-d-d to delete a block in <em>daily — 2026-04-24</em>…
                </div>
              </div>
            </div>
          </>
        )}

        {activeTab === "properties" && (
          <div className="rail-section">
            <div className="rail-section-title">
              <span>Frontmatter</span>
              <I.Plus size={12} style={{ opacity: 0.5, cursor: "pointer" }} />
            </div>
            {D.PAGE_PROPERTIES.map((p) => (
              <div key={p.k} className="prop-row">
                <span className="k">{p.k}</span>
                <span className="v editable">{p.v}</span>
              </div>
            ))}
            <div className="divider" />
            <div className="rail-section-title" style={{ marginTop: 12 }}>
              Inferred
            </div>
            <div className="prop-row">
              <span className="k">block_count</span>
              <span className="v" style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>
                12
              </span>
            </div>
            <div className="prop-row">
              <span className="k">tags_used</span>
              <span className="v">Task, Project, Issue</span>
            </div>
            <div className="prop-row">
              <span className="k">word_count</span>
              <span className="v" style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>
                184
              </span>
            </div>
          </div>
        )}

        {activeTab === "outline" && (
          <div className="rail-section">
            <div className="rail-section-title">Outline</div>
            {D.OUTLINE_LINKS.map((o, i) => (
              <div key={i} className="outline-link" style={{ paddingLeft: 4 + o.depth * 14 }}>
                {o.depth > 0 && <span className="depth-mark" />}
                <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {o.label}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </aside>
  );
}

window.ContextRail = ContextRail;

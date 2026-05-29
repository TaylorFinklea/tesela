/* GRAPHITE — overlay screens: Command Station (⌘K), Leader chord menu
   (Space), and the fullscreen Graph (⌘G). Command + Leader float over a
   dimmed Project; Graph is its own fullscreen layout. */

// ── Command Station (⌘K) ────────────────────────────────────────────────
const GR_CMDS = [
  { grp: 'Jump to', items: [
    { ic: 'sun', lb: 'Today', rk: ['g', 't'], sel: true },
    { ic: 'inbox', lb: 'Inbox', desc: '· 12 unsorted', rk: ['g', 'i'] },
    { ic: 'graph', lb: 'Graph view', rk: ['⌘', 'G'] },
    { ic: 'folder', lb: 'Tesela v5 launch', desc: 'project', rk: [] },
  ] },
  { grp: 'Actions', items: [
    { ic: 'plus', lb: 'New task', desc: 'in current page', rk: ['o', 't'] },
    { ic: 'layoutSidebar', lb: 'Split pane right', rk: ['⌘', '\\'] },
    { ic: 'microphone', lb: 'Voice capture', rk: ['⌘', '⇧', 'V'] },
    { ic: 'adjustments', lb: 'Switch theme…', desc: 'Prism · Tokyo Night · +28', rk: [] },
  ] },
];

function ScreenCommand() {
  const overlay = (
    <div className="gr-scrim">
      <div className="gr-cmdk">
        <div className="gr-cmdk-in">
          <Icon name="search" size={18} color="var(--subtle)" />
          <span className="ph">ship<span className="cur" style={{ display: 'inline-block', verticalAlign: '-3px', marginLeft: 1 }} /></span>
          <kbd>esc</kbd>
        </div>
        <div className="gr-cmdk-body">
          {GR_CMDS.map((g, gi) => (
            <div key={gi}>
              <div className="gr-cmdk-grp">{g.grp}</div>
              {g.items.map((it, i) => (
                <div key={i} className={'gr-cmdk-row' + (it.sel ? ' sel' : '')}>
                  <Icon name={it.ic} size={17} className="ic" />
                  <span className="lb">{it.lb}{it.desc && <span className="desc">{it.desc}</span>}</span>
                  <span className="rk">{it.rk.map((k, j) => (<kbd key={j}>{k}</kbd>))}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
        <div className="gr-cmdk-foot">
          <span><kbd>↑</kbd> <kbd>↓</kbd> navigate</span>
          <span><kbd>↵</kbd> open</span>
          <span><kbd>⌘</kbd><kbd>↵</kbd> open in split</span>
          <span className="sp" />
          <span>Command Station</span>
        </div>
      </div>
    </div>
  );
  return (
    <GraphiteShell activeTab="ship" railActive="ship" overlay={overlay} status={{ mode: 'COMMAND' }}>
      <ProjectPanes />
    </GraphiteShell>
  );
}

// ── Leader chord menu (Space) ───────────────────────────────────────────
const GR_CHORDS = [
  { key: 'o', cl: 'open…', more: true }, { key: 's', cl: 'search…', more: true },
  { key: 'c', cl: 'capture', more: false }, { key: 'g', cl: 'go to…', more: true },
  { key: 't', cl: 'toggle…', more: true }, { key: 'w', cl: 'window…', more: true },
  { key: 'p', cl: 'properties', more: false }, { key: 'd', cl: 'set date…', more: true },
  { key: 'l', cl: 'link block', more: false }, { key: 'r', cl: 'rename', more: false },
];

function ScreenLeader() {
  const overlay = (
    <div className="gr-scrim" style={{ justifyContent: 'flex-end' }}>
      <div className="gr-leader">
        <div className="gr-leader-head"><kbd>Space</kbd><span className="crumb">leader</span><span style={{ marginLeft: 'auto', color: 'var(--faint)' }}>10 actions</span></div>
        <div className="gr-leader-body">
          {GR_CHORDS.map((c, i) => (
            <div key={i} className="gr-chord">
              <span className="key">{c.key}</span>
              <span className="cl">{c.cl}</span>
              {c.more && <Icon name="chevronRight" size={14} className="more" />}
            </div>
          ))}
        </div>
        <div className="gr-leader-foot">Press a key to continue · <span style={{ color: 'var(--fg2)' }}>esc</span> to dismiss</div>
      </div>
    </div>
  );
  return (
    <GraphiteShell activeTab="ship" railActive="ship" overlay={overlay} status={{ mode: 'LEADER' }}>
      <ProjectPanes />
    </GraphiteShell>
  );
}

// ── Graph (⌘G fullscreen) ───────────────────────────────────────────────
const GR_NODES = [
  { id: 'ship', x: 690, y: 360, r: 26, t: 'Ship the docs refresh', kind: 'focus', hub: true },
  { id: 'launch', x: 690, y: 150, r: 19, t: 'Tesela v5 launch', kind: 'project' },
  { id: 'mara', x: 415, y: 240, r: 15, t: 'Mara', kind: 'person' },
  { id: 'domain', x: 455, y: 478, r: 15, t: 'Domain', kind: 'project' },
  { id: 'docs', x: 945, y: 285, r: 16, t: 'Docs', kind: 'note' },
  { id: 'weekly', x: 935, y: 480, r: 14, t: 'Weekly review', kind: 'note' },
  { id: 'daily', x: 520, y: 360, r: 13, t: '2026-04-08', kind: 'daily' },
  { id: 'guide', x: 1075, y: 390, r: 13, t: 'Getting-started guide', kind: 'note' },
  { id: 'bug', x: 770, y: 558, r: 14, t: 'Fix the login bug', kind: 'task' },
  { id: 'reading', x: 300, y: 360, r: 12, t: 'Reading list', kind: 'note' },
];
const GR_EDGES = [['ship', 'launch'], ['ship', 'mara'], ['ship', 'domain'], ['ship', 'docs'], ['ship', 'weekly'], ['ship', 'daily'], ['docs', 'guide'], ['ship', 'bug'], ['daily', 'reading'], ['launch', 'docs'], ['mara', 'domain']];
const GR_KIND_COLOR = { focus: 'var(--coral)', project: 'var(--project)', person: 'var(--person)', note: 'var(--note)', daily: 'var(--event)', task: 'var(--task)' };

function ScreenGraph() {
  const byId = Object.fromEntries(GR_NODES.map((n) => [n.id, n]));
  const legend = [['Project', 'var(--project)'], ['Task', 'var(--task)'], ['Person', 'var(--person)'], ['Note', 'var(--note)'], ['Daily', 'var(--event)']];
  return (
    <div className="gr-root">
      <GrTopBar activeTab="ship" />
      <div className="gr-body">
        <div className="gr-graph">
          <svg viewBox="0 0 1380 740" preserveAspectRatio="xMidYMid meet">
            {GR_EDGES.map(([a, b], i) => {
              const A = byId[a], B = byId[b];
              const focus = a === 'ship' || b === 'ship';
              return <line key={i} x1={A.x} y1={A.y} x2={B.x} y2={B.y}
                stroke={focus ? 'rgba(255,107,90,.28)' : 'rgba(255,255,255,.09)'} strokeWidth={focus ? 1.6 : 1} />;
            })}
            {GR_NODES.map((n) => (
              <g key={n.id}>
                {n.hub && <circle cx={n.x} cy={n.y} r={n.r + 9} fill="none" stroke="var(--coral-line)" strokeWidth="1" opacity="0.5" />}
                <circle cx={n.x} cy={n.y} r={n.r} fill={GR_KIND_COLOR[n.kind]} fillOpacity={n.hub ? 0.95 : 0.85}
                  stroke={n.hub ? 'var(--coral)' : 'rgba(255,255,255,.18)'} strokeWidth={n.hub ? 2 : 1} />
                <text x={n.x} y={n.y + n.r + 16} textAnchor="middle" className={'gr-glabel' + (n.hub ? ' hub' : '')}>{n.t}</text>
              </g>
            ))}
          </svg>
          <div className="gr-graph-tools">
            <div className="gt"><Icon name="search" size={14} color="var(--subtle)" />Filter graph…</div>
            <div className="gt"><Icon name="focus" size={14} color="var(--subtle)" />2 hops</div>
            <div className="gt"><Icon name="layoutGrid" size={14} color="var(--subtle)" />Force</div>
          </div>
          <div className="gr-graph-legend">
            {legend.map(([l, c], i) => (<div key={i} className="lg"><i style={{ background: c }} />{l}</div>))}
          </div>
        </div>
      </div>
      <GrStatus mode="GRAPH" path="graph · 142 nodes · 318 links" keys={[{ k: 'f', label: 'filter' }, { k: '+/-', label: 'zoom' }, { k: 'esc', label: 'close' }]} />
    </div>
  );
}

Object.assign(window, { ScreenCommand, ScreenLeader, ScreenGraph });

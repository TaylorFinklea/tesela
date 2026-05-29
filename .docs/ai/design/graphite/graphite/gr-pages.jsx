/* GRAPHITE — page screens: Tag page (#Task schema + tagged blocks table)
   and Settings (Sync panel). */

// ── Tag page ────────────────────────────────────────────────────────────
const GR_TAG_SCHEMA = [
  { k: 'status', t: 'Choice', ic: 'circleDot' },
  { k: 'priority', t: 'Choice', ic: 'flame' },
  { k: 'deadline', t: 'Date', ic: 'calendar' },
  { k: 'assignee', t: 'Person', ic: 'user' },
  { k: 'project', t: 'Link', ic: 'link' },
  { k: 'estimate', t: 'Number', ic: 'clock' },
];
const GR_TAG_ROWS = [
  { text: 'Ship the docs refresh', status: 'doing', pri: 'high', due: 'Apr 10', urg: true, proj: 'v5 launch' },
  { text: 'Write the getting-started guide', status: 'doing', pri: '—', due: 'Apr 11', proj: 'v5 launch' },
  { text: 'Fix the login bug', status: 'todo', pri: 'high', due: 'Apr 12', proj: 'v5 launch' },
  { text: 'Reproduce on staging', status: 'todo', pri: '—', due: '—', proj: '—' },
  { text: 'Audit empty states', status: 'todo', pri: '—', due: 'Apr 14', proj: 'Design' },
  { text: 'Review empty-state designs', status: 'done', pri: '—', due: 'Apr 9', proj: 'Design' },
];

function ScreenTagPage() {
  const cls = (s) => (s === 'doing' ? 'doing' : s === 'done' ? 'done' : 'todo');
  return (
    <GraphiteShell activeTab="task" railActive="ship"
      tabs={[{ id: 'today', name: 'today' }, { id: 'ship', name: 'ship the docs refresh' }, { id: 'task', name: '#Task' }]}
      status={{ path: 'tags / Task', extra: '23 blocks', keys: [{ k: 'n', label: 'new' }, { k: '/', label: 'filter' }, { k: 'v', label: 'view' }] }}>
      <div className="gr-pane focus">
        <div className="gr-pane-head">
          <Icon name="hash" size={16} color="var(--coral)" />
          <span className="ttl">Task</span>
          <span className="gr-typetag task"><span className="sw" />Tag</span>
          <span className="sp" />
          <span className="meta">extends · Block</span>
          <span className="gr-headbtn cta"><Icon name="plus" size={14} />New task</span>
        </div>
        <div className="gr-tagbody">
          <div className="gr-schema">
            <div className="h"><Icon name="adjustments" size={13} color="var(--faint)" />Schema · 6 properties</div>
            <div className="gr-schema-grid">
              {GR_TAG_SCHEMA.map((p, i) => (
                <div key={i} className="gr-pdef">
                  <Icon name={p.ic} size={15} className="pic" />
                  <span className="pk">{p.k}</span>
                  <span className="pt">{p.t}</span>
                </div>
              ))}
            </div>
          </div>
          <div className="gr-tbl-wrap">
            <table className="gr-tbl">
              <thead><tr><th style={{ width: '40%' }}>Block</th><th>Status</th><th>Priority</th><th>Deadline</th><th>Project</th></tr></thead>
              <tbody>
                {GR_TAG_ROWS.map((r, i) => (
                  <tr key={i}>
                    <td><span className="c-text"><span className={'gr-bull' + (r.status === 'done' ? '' : '')} style={{ background: r.status === 'done' ? 'var(--query)' : 'var(--task)', position: 'static' }} />{r.text}</span></td>
                    <td><span className={'cell-chip ' + cls(r.status)}>{r.status}</span></td>
                    <td>{r.pri === 'high' ? <span className="cell-chip high">high</span> : <span style={{ color: 'var(--faint)' }}>—</span>}</td>
                    <td><span className={'due' + (r.urg ? ' urg' : '')}>{r.due}</span></td>
                    <td>{r.proj === '—' ? <span style={{ color: 'var(--faint)' }}>—</span> : <span style={{ color: 'var(--project)' }}>[[{r.proj}]]</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </GraphiteShell>
  );
}

// ── Settings (Sync) ─────────────────────────────────────────────────────
const GR_SET_NAV = [
  { ic: 'adjustments', l: 'General' },
  { ic: 'sun', l: 'Appearance' },
  { ic: 'graph', l: 'Sync', active: true },
  { ic: 'layoutGrid', l: 'Devices' },
  { ic: 'microphone', l: 'Voice' },
  { ic: 'folder', l: 'Data' },
  { ic: 'bolt', l: 'Plugins' },
];
const GR_DEVICES = [
  { ic: 'layoutSidebar', nm: 'MacBook Pro', ds: 'this device · macOS', live: true, badge: 'connected' },
  { ic: 'fileText', nm: 'iPhone 15 Pro', ds: 'Tesela iOS · synced 2m ago', live: true, badge: 'synced' },
  { ic: 'graph', nm: 'tesela-server', ds: 'localhost:7474 · relay host', live: true, badge: 'live' },
];

function ScreenSettings() {
  return (
    <GraphiteShell activeTab="settings" railActive="ship"
      tabs={[{ id: 'today', name: 'today' }, { id: 'ship', name: 'ship the docs refresh' }, { id: 'settings', name: 'settings' }]}
      status={{ mode: 'SETTINGS', path: 'settings / sync', keys: [{ k: 'j/k', label: 'move' }, { k: '↵', label: 'edit' }, { k: 'esc', label: 'close' }] }}>
      <div className="gr-pane focus">
        <div className="gr-set">
          <div className="gr-set-nav">
            <div className="navh">Settings</div>
            {GR_SET_NAV.map((n, i) => (
              <div key={i} className={'item' + (n.active ? ' active' : '')}><Icon name={n.ic} size={16} className="ic" />{n.l}</div>
            ))}
          </div>
          <div className="gr-set-main">
            <div className="h1">Sync</div>
            <div className="h1-sub">Local-first, always. Tesela syncs encrypted changes through a relay you control.</div>

            <div className="gr-set-sec">
              <div className="sech">Relay</div>
              <div className="gr-field">
                <div className="fl"><div className="ft">Relay URL</div><div className="fd">WebSocket endpoint your devices connect through.</div></div>
                <input className="gr-input" defaultValue="wss://relay.tesela.dev" />
              </div>
              <div className="gr-field">
                <div className="fl"><div className="ft">Live sync</div><div className="fd">Stream changes over WebSocket as you type.</div></div>
                <div className="gr-toggle on" />
              </div>
              <div className="gr-field">
                <div className="fl"><div className="ft">Sync over cellular</div><div className="fd">Allow mobile devices to sync without Wi-Fi.</div></div>
                <div className="gr-toggle" />
              </div>
              <div className="gr-field">
                <div className="fl"><div className="ft">End-to-end encryption</div><div className="fd">Changes are encrypted with your passphrase before they leave the device.</div></div>
                <div className="gr-toggle on" />
              </div>
            </div>

            <div className="gr-set-sec">
              <div className="sech">Devices · 3</div>
              {GR_DEVICES.map((d, i) => (
                <div key={i} className="gr-devrow">
                  <div className="di"><Icon name={d.ic} size={17} /></div>
                  <div className="dn"><div className="nm2">{d.nm}</div><div className="ds">{d.ds}</div></div>
                  <div className="badge"><i />{d.badge}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </GraphiteShell>
  );
}

Object.assign(window, { ScreenTagPage, ScreenSettings });

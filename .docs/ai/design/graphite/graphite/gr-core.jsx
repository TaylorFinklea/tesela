/* GRAPHITE — core screens: Project focus, Daily journal, Inbox, Agenda.
   Each composes <GraphiteShell> with a screen-specific main area. */

// ── data ───────────────────────────────────────────────────────────────
const GR_JOURNAL = [
  {
    date: 'Apr 10', dow: 'Thursday', today: true, blocks: [
      { text: 'Morning pages — felt scattered, narrowed to one thing: ship the docs.', kind: 'note' },
      { text: 'Ship the docs refresh', kind: 'task', tag: 'Task', props: [{ k: 'status', v: 'doing', tone: 'doing' }, { k: 'priority', v: 'high', tone: 'high' }] },
      { text: 'Standup notes', kind: 'note', children: ['Mara unblocked the auth PR', 'Move launch gate to Friday'] },
      { text: 'Call Priya about the v5 launch copy', kind: 'task', tag: 'Task' },
    ],
  },
  {
    date: 'Apr 9', dow: 'Wednesday', today: false, blocks: [
      { text: 'Moved docs refresh to high priority after the launch sync.', kind: 'note' },
      { text: 'Reviewed empty-state designs', kind: 'task', done: true, tag: 'Task' },
      { text: 'Reading: "The Humane Interface" — ch. 4 on modes', kind: 'note', link: 'Reading list' },
    ],
  },
];

const GR_INBOX = window.GR_INBOX;

const GR_EVENTS = [
  { day: 0, t: '10:00', dur: 0.5, title: '1:1 with Priya', kind: 'event' },
  { day: 1, t: '11:00', dur: 1, title: 'Design review', kind: 'event' },
  { day: 2, t: '9:00', dur: 2, title: 'Deep work — docs', kind: 'focus-ev' },
  { day: 3, t: '9:30', dur: 0.5, title: 'Standup', kind: 'event' },
  { day: 3, t: '14:00', dur: 1, title: 'Docs review w/ Mara', kind: 'event' },
  { day: 3, t: '16:00', dur: 0.5, title: 'Ship docs refresh · due', kind: 'task' },
  { day: 4, t: '13:00', dur: 1, title: 'v5 Retro', kind: 'event' },
];
const GR_WEEK = [{ dw: 'Mon', dn: 7 }, { dw: 'Tue', dn: 8 }, { dw: 'Wed', dn: 9 }, { dw: 'Thu', dn: 10, today: true }, { dw: 'Fri', dn: 11 }];
const GR_HOURS = [8, 9, 10, 11, 12, 13, 14, 15, 16];
const GR_SLOT = 62, GR_START = 8;

// ── Project focus (canonical) ───────────────────────────────────────────
function ProjectPanes() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <>
      <div className="gr-pane focus">
        <div className="gr-pane-head">
          <Icon name="arrowLeft" size={16} className="gr-back" />
          <span className="ttl">Ship the docs refresh</span>
          <span className="gr-typetag"><span className="sw" />Project</span>
          <span className="sp" />
          <span className="meta">edited 2m ago</span>
          <Icon name="dotsVertical" size={16} color="var(--faint)" />
        </div>
        <div className="gr-outline">
          {OUTLINE.map((b) => (
            <div key={b.id} className={'gr-blk' + (b.tag === 'Task' ? ' task' : '') + (b.selected ? ' sel' : '')}>
              <div className="gr-blk-main">
                <span className="gr-bull" />
                <div className="gr-blk-body">
                  <div className="gr-blk-text">
                    {b.link ? (<>Weekly sync with <span className="gr-link">[[Domain]]</span> leads</>) : b.text}
                    {b.tag && <span className="gr-tagchip">#{b.tag}</span>}
                    {b.tag2 && <span className="gr-tagchip alt">#{b.tag2}</span>}
                  </div>
                  {b.props && b.props.length > 0 && (
                    <div className="gr-props">
                      {b.props.map((p, i) => (
                        <span key={i} className={'gr-pchip ' + tone(p.tone)}><span className="k">{p.k}</span>{p.link ? <span className="lk">[[{p.v}]]</span> : <span className="v">{p.v}</span>}</span>
                      ))}
                    </div>
                  )}
                  {b.children && b.children.length > 0 && (
                    <div className="gr-kids">
                      {b.children.map((k) => (
                        <div key={k.id} className="gr-kid"><span className="kb" /><span className="kt">{k.mention ? (<>Review with <span className="gr-mention">@Mara</span> on the Domain team</>) : k.text}</span></div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="gr-pane side">
        <div className="gr-pane-head"><span className="ttl" style={{ fontSize: 13.5 }}>Linked references</span><span className="sp" /><span className="meta">3</span></div>
        <div className="gr-side-body">
          {BACKLINKS.map((r, i) => (
            <div key={i} className="gr-refcard">
              <div className="src"><span className={'gr-dot ' + (r.kind === 'project' ? 'project' : r.kind === 'daily' ? 'event' : 'note')} />{r.src}</div>
              <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
            </div>
          ))}
          <div className="gr-proplist">
            <div className="ph">Properties · b1</div>
            {PROPS.map((p, i) => (<div key={i} className="gr-prow"><span className="chord">{p.chord}</span><span className="k">{p.k}</span><span className={'v ' + (p.tone || '')}>{p.v}</span></div>))}
          </div>
        </div>
      </div>
    </>
  );
}

function ScreenProject() {
  return (
    <GraphiteShell activeTab="ship" railActive="ship" status={{ extra: 'block b1' }}>
      <ProjectPanes />
    </GraphiteShell>
  );
}

// ── Daily journal ───────────────────────────────────────────────────────
function ScreenDaily() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <GraphiteShell activeTab="today" railActive="today" status={{ path: 'today', mode: 'NORMAL', extra: 'journal' }}>
      <div className="gr-pane focus">
        <div className="gr-pane-head">
          <Icon name="sun" size={16} color="var(--note)" />
          <span className="ttl">Today</span>
          <span className="sub">April 2026</span>
          <span className="sp" />
          <span className="gr-headbtn"><Icon name="arrowLeft" size={14} />Jump to date</span>
        </div>
        <div className="gr-outline">
          {GR_JOURNAL.map((day, di) => (
            <div key={di}>
              <div className={'gr-dayhdr' + (day.today ? ' today' : '')}>
                <span className="d">{day.date}</span><span className="dow">{day.dow}</span><span className="ln" />
                {day.today && <span className="dow" style={{ color: 'var(--coral)' }}>today</span>}
              </div>
              {day.blocks.map((b, bi) => (
                <div key={bi} className={'gr-blk' + (b.kind === 'task' ? ' task' : '') + (b.done ? ' done' : '')}>
                  <div className="gr-blk-main">
                    <span className="gr-bull" />
                    <div className="gr-blk-body">
                      <div className="gr-blk-text">
                        {b.link ? (<>Reading: "The Humane Interface" — ch. 4 on modes <span className="gr-link">[[Reading list]]</span></>) : b.text}
                        {b.tag && <span className="gr-tagchip">#{b.tag}</span>}
                      </div>
                      {b.props && (
                        <div className="gr-props">
                          {b.props.map((p, i) => (<span key={i} className={'gr-pchip ' + tone(p.tone)}><span className="k">{p.k}</span><span className="v">{p.v}</span></span>))}
                        </div>
                      )}
                      {b.children && (
                        <div className="gr-kids">
                          {b.children.map((c, ci) => (<div key={ci} className="gr-kid"><span className="kb" /><span className="kt">{c}</span></div>))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </GraphiteShell>
  );
}

// ── Inbox ───────────────────────────────────────────────────────────────
function ScreenInbox() {
  const chips = [{ l: 'All', n: 12, active: true }, { l: 'Tasks', n: 5 }, { l: 'Notes', n: 4 }, { l: 'Voice', n: 2 }, { l: 'Events', n: 1 }];
  return (
    <GraphiteShell activeTab="inbox" railActive="inbox" status={{ path: 'inbox', extra: '12 unsorted', keys: [{ k: 'e', label: 'file' }, { k: 't', label: 'tag' }, { k: 'x', label: 'archive' }] }}>
      <div className="gr-pane focus">
        <div className="gr-pane-head">
          <Icon name="inbox" size={16} color="var(--event)" />
          <span className="ttl">Inbox</span>
          <span className="sp" />
          <span className="gr-headbtn cta"><Icon name="bolt" size={14} />Process all</span>
        </div>
        <div className="gr-chipbar">
          {chips.map((c, i) => (<div key={i} className={'gr-chip' + (c.active ? ' active' : '')}>{c.l}<span className="n">{c.n}</span></div>))}
        </div>
        <div className="gr-inbox-body">
          {GR_INBOX.map((it, i) => (
            <div key={i} className={'gr-icard' + (it.sel ? ' sel' : '')}>
              <div className="src"><Icon name={it.src} size={16} /></div>
              <div className="gr-icard-body">
                <div className="txt">{it.text}</div>
                <div className="meta">{it.meta.map((m, j) => (<span key={j} className="pill">{m}</span>))}</div>
              </div>
              <div className="gr-icard-acts">
                <div className="gr-iact" title="File"><Icon name="folder" size={15} /></div>
                <div className="gr-iact" title="Tag"><Icon name="hash" size={15} /></div>
                <div className="gr-iact" title="Snooze"><Icon name="clock" size={15} /></div>
                <div className="gr-iact go" title="Open"><Icon name="cornerDownRight" size={15} /></div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </GraphiteShell>
  );
}

// ── Agenda / week ─────────────────────────────────────────────────────────
function ScreenAgenda() {
  const evStyle = (ev) => {
    const [h, m] = ev.t.split(':').map(Number);
    const top = (h - GR_START) * GR_SLOT + (m / 60) * GR_SLOT;
    return { top: top + 'px', height: ev.dur * GR_SLOT - 5 + 'px' };
  };
  const nowTop = (14 - GR_START) * GR_SLOT + (8 / 60) * GR_SLOT;
  return (
    <GraphiteShell activeTab="today" railActive="today"
      tabs={[{ id: 'today', name: 'today' }, { id: 'agenda', name: 'agenda' }, { id: 'ship', name: 'ship the docs refresh' }]}
      status={{ path: 'agenda / week 15', extra: 'Apr 7 – 11', keys: [{ k: 'h/l', label: 'week' }, { k: 'd', label: 'day view' }, { k: 't', label: 'today' }] }}>
      <div className="gr-pane focus">
        <div className="gr-pane-head">
          <Icon name="calendar" size={16} color="var(--event)" />
          <span className="ttl">April 2026</span>
          <span className="sub">Week 15 · Apr 7 – 11</span>
          <span className="sp" />
          <span className="gr-headbtn"><Icon name="chevronRight" size={14} style={{ transform: 'rotate(180deg)' }} /></span>
          <span className="gr-headbtn">Today</span>
          <span className="gr-headbtn"><Icon name="chevronRight" size={14} /></span>
        </div>
        <div className="gr-agenda">
          <div style={{ display: 'grid', gridTemplateColumns: '56px repeat(5,1fr)' }}>
            <div className="gr-ag-corner" />
            {GR_WEEK.map((d, i) => (
              <div key={i} className={'gr-ag-colhdr' + (d.today ? ' today' : '')}><span className="dw">{d.dw}</span><span className="dn">{d.dn}</span></div>
            ))}
          </div>
          <div className="gr-agrid" style={{ flex: 1 }}>
            <div className="gr-ag-times">
              {GR_HOURS.map((h) => (<div key={h} className="gr-ag-time">{h > 12 ? h - 12 : h}{h >= 12 ? 'p' : 'a'}</div>))}
            </div>
            {GR_WEEK.map((d, di) => (
              <div key={di} className="gr-ag-col">
                <div className="gr-ag-daycol">{GR_HOURS.map((h) => (<div key={h} className="gr-ag-slot" />))}</div>
                {GR_EVENTS.filter((e) => e.day === di).map((ev, ei) => (
                  <div key={ei} className={'gr-ev ' + ev.kind} style={evStyle(ev)}>
                    <div className="et">{ev.title}</div>
                    <div className="em">{ev.t}</div>
                  </div>
                ))}
                {d.today && <div className="gr-now" style={{ top: nowTop + 'px' }} />}
              </div>
            ))}
          </div>
        </div>
      </div>
    </GraphiteShell>
  );
}

Object.assign(window, { ProjectPanes, ScreenProject, ScreenDaily, ScreenInbox, ScreenAgenda });

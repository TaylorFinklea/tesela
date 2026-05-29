/* GRAPHITE MOBILE — iPhone screens. Each renders inside <GrmPhone>. */

// ── Daily (journal) — primary tab ───────────────────────────────────────
const GRM_DAY = [
  { text: 'Morning pages — narrowed it down to one thing: ship the docs.', kind: 'note' },
  { text: 'Ship the docs refresh', kind: 'task', tag: 'Task', props: [{ k: 'status', v: 'doing', tone: 'doing' }, { k: 'priority', v: 'high', tone: 'high' }] },
  { text: 'Standup notes', kind: 'note', children: ['Mara unblocked the auth PR', 'Move launch gate to Friday'] },
  { text: 'Call Priya about the v5 launch copy', kind: 'task' },
];
const GRM_DAY_PREV = [
  { text: 'Reviewed empty-state designs', kind: 'task', done: true, tag: 'Task' },
  { text: 'Reading: "The Humane Interface", ch. 4', kind: 'note', link: 'Reading list' },
];

function PhoneDaily() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  const renderBlocks = (blocks) => blocks.map((b, i) => (
    <div key={i} className={'grm-blk' + (b.kind === 'task' ? ' task' : '') + (b.done ? ' done' : '')}>
      <span className="grm-bull" />
      <div className="grm-blk-body">
        <div className="grm-blk-text">
          {b.link ? (<>Reading: "The Humane Interface", ch. 4 <span className="grm-link">[[Reading list]]</span></>) : b.text}
          {b.tag && <span className="grm-tagchip">#{b.tag}</span>}
        </div>
        {b.props && <div className="grm-props">{b.props.map((p, j) => (<span key={j} className={'grm-pchip ' + tone(p.tone)}><span className="k">{p.k}</span><span className="v">{p.v}</span></span>))}</div>}
        {b.children && <div className="grm-kids">{b.children.map((c, j) => (<div key={j} className="grm-kid"><span className="kb" /><span className="kt">{c}</span></div>))}</div>}
      </div>
    </div>
  ));
  return (
    <GrmPhone>
      <GrmHeader title="Today" sub="Thursday · April 10" calendar />
      <div className="grm-body">
        <div className="grm-scroll">
          <div className="grm-dayhdr today"><span className="d">Apr 10</span><span className="dow">Thu</span><span className="ln" /><span className="dow" style={{ color: 'var(--coral)' }}>today</span></div>
          {renderBlocks(GRM_DAY)}
          <div className="grm-dayhdr"><span className="d">Apr 9</span><span className="dow">Wed</span><span className="ln" /></div>
          {renderBlocks(GRM_DAY_PREV)}
        </div>
        <GrmBottomBar active="daily" />
      </div>
    </GrmPhone>
  );
}

// ── Inbox ───────────────────────────────────────────────────────────────
function PhoneInbox() {
  const chips = [{ l: 'All', n: 12, active: true }, { l: 'Tasks', n: 5 }, { l: 'Notes', n: 4 }, { l: 'Voice', n: 2 }, { l: 'Events', n: 1 }];
  return (
    <GrmPhone>
      <GrmHeader title="Inbox" sub="12 unsorted" />
      <div className="grm-body">
        <div className="grm-scroll">
          <div className="grm-chips">{chips.map((c, i) => (<div key={i} className={'grm-chip' + (c.active ? ' active' : '')}>{c.l}<span className="n">{c.n}</span></div>))}</div>
          {GR_INBOX.map((it, i) => (
            <div key={i} className={'grm-icard' + (it.sel ? ' sel' : '')}>
              <div className="src"><Icon name={it.src} size={17} /></div>
              <div className="grm-icard-body">
                <div className="txt">{it.text}</div>
                <div className="meta">{it.meta.map((m, j) => (<span key={j} className="pill">{m}</span>))}</div>
                {it.sel && (
                  <div className="acts">
                    <span className="grm-iact"><Icon name="folder" size={15} />File</span>
                    <span className="grm-iact"><Icon name="hash" size={15} />Tag</span>
                    <span className="grm-iact"><Icon name="clock" size={15} />Snooze</span>
                    <span className="grm-iact go"><Icon name="cornerDownRight" size={15} />Open</span>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
        <GrmBottomBar active="inbox" />
      </div>
    </GrmPhone>
  );
}

// ── Library — workspace widget grid (the AnyType direction) ──────────────
const GRM_AMBIENTS = [
  { icon: 'calendar', title: 'Calendar', hint: 'tap a day → daily', tint: 'var(--event)' },
  { icon: 'squareCheck', title: 'In Progress', hint: 'open tasks across the mosaic', tint: 'var(--query)' },
  { icon: 'layoutGrid', title: 'Dashboard', hint: 'pinned widgets', tint: 'var(--project)' },
  { icon: 'sparkles', title: 'AI', hint: 'coming later', tint: 'var(--person)', soon: true },
];

function PhoneLibrary() {
  return (
    <GrmPhone>
      <GrmHeader title="Library" />
      <div className="grm-body">
        <div className="grm-scroll">
          <div className="grm-seg">
            <div className="grm-segbtn active">Workspace</div>
            <div className="grm-segbtn">Pages</div>
            <div className="grm-segbtn">Tags</div>
          </div>
          <div className="grm-grid">
            {GRM_AMBIENTS.map((a, i) => (
              <div key={i} className="grm-acard">
                <div className="top">
                  <div className="gl" style={{ background: 'color-mix(in srgb,' + a.tint + ' 18%, transparent)' }}><Icon name={a.icon} size={19} color={a.tint} /></div>
                  {a.soon && <span className="soon">soon</span>}
                </div>
                <div className="at">{a.title}</div>
                <div className="ah">{a.hint}</div>
              </div>
            ))}
          </div>
          <div className="grm-w">
            <div className="grm-w-head"><Icon name="pin" size={14} className="ic" /><span className="ti">Pinned</span></div>
            <div className="grm-w-body">
              {WIDGETS.pinned.items.map((it, i) => (<div key={i} className="grm-row"><Icon name={it.icon} size={17} className="ic" /><span className="lb">{it.label}</span><Icon name="chevronRight" size={16} color="var(--faint)" /></div>))}
            </div>
          </div>
          <div className="grm-w">
            <div className="grm-w-head"><Icon name="clock" size={14} className="ic" /><span className="ti">Recent</span></div>
            <div className="grm-w-body">
              {[['fileText', 'Standup notes'], ['folder', 'Tesela v5 launch'], ['hash', 'Task']].map((r, i) => (<div key={i} className="grm-row"><Icon name={r[0]} size={17} className="ic" /><span className="lb">{r[1]}</span><Icon name="chevronRight" size={16} color="var(--faint)" /></div>))}
            </div>
          </div>
        </div>
        <GrmBottomBar active="library" />
      </div>
    </GrmPhone>
  );
}

// ── Page / Project (pushed view) ────────────────────────────────────────
function PhonePage() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <GrmPhone>
      <div className="grm-pageback"><Icon name="chevronRight" size={20} style={{ transform: 'rotate(180deg)' }} />Today</div>
      <div className="grm-pagehead">
        <div className="ttl">Ship the docs refresh</div>
        <div className="metarow"><span className="grm-typetag"><span className="sw" />Project</span><span className="when">edited 2m ago · 2 linked</span></div>
      </div>
      <div className="grm-body">
        <div className="grm-scroll" style={{ paddingTop: 4 }}>
          {OUTLINE.map((b) => (
            <div key={b.id} className={'grm-blk' + (b.tag === 'Task' ? ' task' : '')} style={b.selected ? { background: 'var(--raised)', borderRadius: 11, borderLeft: '2px solid var(--coral)' } : null}>
              <span className="grm-bull" />
              <div className="grm-blk-body">
                <div className="grm-blk-text">
                  {b.link ? (<>Weekly sync with <span className="grm-link">[[Domain]]</span> leads</>) : b.text}
                  {b.tag && <span className="grm-tagchip">#{b.tag}</span>}
                  {b.tag2 && <span className="grm-tagchip" style={{ background: 'rgba(232,105,127,.15)', color: 'var(--task)' }}>#{b.tag2}</span>}
                </div>
                {b.props && b.props.length > 0 && <div className="grm-props">{b.props.map((p, i) => (<span key={i} className={'grm-pchip ' + tone(p.tone)}><span className="k">{p.k}</span>{p.link ? <span className="v" style={{ color: 'var(--project)' }}>[[{p.v}]]</span> : <span className="v">{p.v}</span>}</span>))}</div>}
                {b.children && b.children.length > 0 && <div className="grm-kids">{b.children.map((k) => (<div key={k.id} className="grm-kid"><span className="kb" /><span className="kt">{k.mention ? (<>Review with <span className="grm-mention">@Mara</span> on the Domain team</>) : k.text}</span></div>))}</div>}
              </div>
            </div>
          ))}
          <div className="grm-refhdr">Linked references · 3</div>
          {BACKLINKS.map((r, i) => (
            <div key={i} className="grm-refcard">
              <div className="src"><span className={'grm-dot ' + (r.kind === 'project' ? 'project' : r.kind === 'daily' ? 'event' : 'note')} />{r.src}</div>
              <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
            </div>
          ))}
        </div>
        <GrmBottomBar active="daily" />
      </div>
    </GrmPhone>
  );
}

// ── Capture sheet (voice + compose) over Daily ──────────────────────────
function PhoneCapture() {
  const waveH = [10, 18, 28, 22, 34, 16, 26, 38, 20, 30, 14, 24, 36, 18, 28, 22, 32, 12, 26, 20, 30, 16, 24, 34, 18, 28, 14, 22, 32, 20];
  return (
    <GrmPhone>
      <GrmHeader title="Today" sub="Thursday · April 10" calendar />
      <div className="grm-body">
        <div className="grm-scroll" style={{ opacity: 0.5 }}>
          <div className="grm-dayhdr today"><span className="d">Apr 10</span><span className="dow">Thu</span><span className="ln" /></div>
          {GRM_DAY.slice(0, 2).map((b, i) => (<div key={i} className={'grm-blk' + (b.kind === 'task' ? ' task' : '')}><span className="grm-bull" /><div className="grm-blk-body"><div className="grm-blk-text">{b.text}</div></div></div>))}
        </div>
        <div className="grm-sheetdim" />
        <div className="grm-sheet">
          <div className="grm-grab" />
          <div className="grm-sheet-head">
            <span className="h">Quick capture</span>
            <span className="to"><Icon name="inbox" size={14} color="var(--event)" />to Inbox</span>
          </div>
          <div className="grm-compose">
            Add a keyboard cheatsheet to the <span className="parsed-tag">#docs</span> before launch, due <span className="parsed-date">Friday</span><span className="grm-cur" />
          </div>
          <div className="grm-wave">{waveH.map((h, i) => (<i key={i} style={{ height: h, opacity: 0.35 + (h / 38) * 0.6 }} />))}</div>
          <div className="grm-parsed">
            <span className="grm-pchip"><span className="k">tag</span><span className="v" style={{ color: 'var(--coral)' }}>#docs</span></span>
            <span className="grm-pchip"><span className="k">due</span><span className="v" style={{ color: 'var(--event)' }}>2026-04-11</span></span>
            <span className="grm-pchip"><span className="k">type</span><span className="v">task</span></span>
          </div>
          <div className="grm-sheet-foot">
            <div className="grm-recbtn"><Icon name="microphone" size={22} /></div>
            <div className="grm-recmeta"><div className="l1">Listening…</div><div className="l2">0:08 · Parakeet on-device</div></div>
            <div className="grm-sendbtn"><Icon name="cornerDownRight" size={16} />Save</div>
          </div>
        </div>
      </div>
    </GrmPhone>
  );
}

// ── Agenda (day) ────────────────────────────────────────────────────────
const GRM_AG_WEEK = [{ dw: 'M', dn: 7 }, { dw: 'T', dn: 8 }, { dw: 'W', dn: 9 }, { dw: 'T', dn: 10, today: true }, { dw: 'F', dn: 11 }, { dw: 'S', dn: 12 }, { dw: 'S', dn: 13 }];
const GRM_AG_EVENTS = [
  { time: '9:30', label: 'Standup', kind: 'event' },
  { time: '9:00', label: 'Deep work — docs refresh', kind: 'focus-ev', span: true },
  { time: '14:00', label: 'Docs review w/ Mara', kind: 'event' },
  { time: '16:00', label: 'Ship docs refresh · due', kind: 'task' },
];

function PhoneAgenda() {
  return (
    <GrmPhone>
      <GrmHeader title="Agenda" sub="Week 15 · Apr 7 – 13" calendar />
      <div className="grm-body">
        <div className="grm-scroll">
          <div className="grm-agstrip">
            {GRM_AG_WEEK.map((d, i) => (<div key={i} className={'grm-agday' + (d.today ? ' today' : '')}><span className="dw">{d.dw}</span><span className="dn">{d.dn}</span></div>))}
          </div>
          <div className="grm-dayhdr today" style={{ paddingTop: 14 }}><span className="d">Thursday</span><span className="dow">Apr 10</span><span className="ln" /></div>
          <div className="grm-agtl">
            {[['9 AM', [GRM_AG_EVENTS[1], GRM_AG_EVENTS[0]]], ['12 PM', []], ['2 PM', [GRM_AG_EVENTS[2]]], ['4 PM', [GRM_AG_EVENTS[3]]]].map((slot, i) => (
              <div key={i} className="grm-agrow">
                <span className="grm-agtime">{slot[0]}</span>
                {slot[1].map((ev, j) => (<div key={j} className={'grm-agev ' + ev.kind}><div className="et">{ev.label}</div><div className="em">{ev.time}{ev.span ? ' – 11:00' : ''}</div></div>))}
              </div>
            ))}
          </div>
        </div>
        <GrmBottomBar active="agenda" />
      </div>
    </GrmPhone>
  );
}

Object.assign(window, { PhoneDaily, PhoneInbox, PhoneLibrary, PhonePage, PhoneCapture, PhoneAgenda });

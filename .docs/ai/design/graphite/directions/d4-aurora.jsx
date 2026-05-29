/* Direction 4 — AURORA
   The "designed-app" take (Craft/Notion-grade) without the warm editorial.
   Cool indigo-slate base with a faint aurora wash, genuine soft elevation
   (floating cards + shadows), generous radii and spacing, and a coral +
   periwinkle duotone. Coral = primary/active; periwinkle = links, focus
   rings, secondary. The roomiest, most polished-product direction. */

(function () {
  if (document.getElementById('au-styles')) return;
  const css = `
  .au-root{
    --bg:#0C0E16; --surface:#11131F; --card:#181B2A; --card-2:#1F2333; --raised:#262B3D;
    --line:rgba(255,255,255,.05); --line-2:rgba(255,255,255,.09);
    --fg:#ECEEF6; --muted:#9DA3B6; --subtle:#6B7185; --faint:#474D61;
    --coral:#FF6A5D; --coral-soft:rgba(255,106,93,.14); --peri:#8C9EFF; --peri-soft:rgba(140,158,255,.14);
    --task:#F0758C; --event:#67C0D6; --note:#E7B36A; --project:#8C9EFF; --person:#B79CF0; --query:#86C266;
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    --sh-1:0 1px 2px rgba(0,0,0,.3); --sh-2:0 4px 14px rgba(0,0,0,.30); --sh-3:0 10px 34px rgba(0,0,0,.38);
    position:absolute; inset:0; color:var(--fg); font-family:var(--sans); font-size:14px; line-height:1.55;
    display:grid; grid-template-rows:54px 1fr 30px; overflow:hidden; -webkit-font-smoothing:antialiased;
    background:var(--bg);
  }
  .au-root::before{content:""; position:absolute; inset:0; pointer-events:none; z-index:0;
    background:
      radial-gradient(900px 460px at 16% -8%, rgba(140,158,255,.10), transparent 60%),
      radial-gradient(760px 420px at 92% 4%, rgba(255,106,93,.07), transparent 58%);}
  .au-root > *{position:relative; z-index:1;}
  .au-root *{box-sizing:border-box;}

  /* ── top bar ── */
  .au-top{display:grid; grid-template-columns:auto 1fr auto; align-items:center; gap:18px; padding:0 18px; border-bottom:1px solid var(--line);}
  .au-brand{display:flex; align-items:center; gap:10px;}
  .au-brand .nm{font-size:14.5px; font-weight:600; letter-spacing:-.015em;}
  .au-tabs{display:flex; align-items:center; gap:5px; margin-left:6px;}
  .au-tab{display:flex; align-items:center; gap:8px; height:32px; padding:0 13px; border-radius:10px; white-space:nowrap;
    color:var(--subtle); font-size:13px; cursor:pointer; transition:all .16s;}
  .au-tab:hover{color:var(--muted); background:var(--card);}
  .au-tab.active{color:var(--fg); background:var(--card); box-shadow:var(--sh-1), inset 0 0 0 1px var(--line-2);}
  .au-tab .kdot{width:7px;height:7px;border-radius:50%;background:var(--coral);box-shadow:0 0 8px var(--coral);}
  .au-cmd{justify-self:center; width:min(460px,100%); display:flex; align-items:center; gap:10px; height:36px; padding:0 14px;
    border-radius:11px; background:var(--card); border:1px solid var(--line-2); color:var(--subtle); font-size:13px; box-shadow:var(--sh-1);}
  .au-cmd .ph{flex:1;} .au-cmd kbd{font-family:var(--mono); font-size:11px; color:var(--subtle); background:var(--raised); border-radius:6px; padding:2px 7px;}
  .au-icons{display:flex; align-items:center; gap:3px;}
  .au-ic{width:34px;height:34px;display:grid;place-items:center;border-radius:10px;color:var(--subtle);cursor:pointer;transition:all .16s;}
  .au-ic:hover{color:var(--fg); background:var(--card);}
  .au-conn{width:34px;display:grid;place-items:center;} .au-conn i{width:8px;height:8px;border-radius:50%;background:var(--query);box-shadow:0 0 10px var(--query);}

  /* ── body ── */
  .au-body{display:flex; min-height:0; overflow:hidden; padding:14px; gap:14px;}
  .au-rail{width:262px; flex-shrink:0; display:flex; flex-direction:column; gap:12px; overflow:hidden;}
  .au-w{background:var(--card); border:1px solid var(--line); border-radius:15px; padding:13px 13px 11px; box-shadow:var(--sh-2);}
  .au-w-head{display:flex; align-items:center; gap:10px; margin-bottom:9px; padding:0 2px;}
  .au-w-head .gl{width:24px;height:24px;border-radius:8px;display:grid;place-items:center;flex-shrink:0;color:#fff;}
  .au-w-head .ti{flex:1; font-size:12px; font-weight:600; letter-spacing:.01em; color:var(--muted);}
  .au-w-head .bd{font-family:var(--mono); font-size:10.5px; color:var(--subtle); background:var(--card-2); border-radius:7px; padding:2px 8px; white-space:nowrap;}
  .au-capture{display:flex; align-items:center; gap:9px; padding:10px 12px; border-radius:11px; background:var(--bg); border:1px solid var(--line); color:var(--faint); font-size:13px;}
  .au-capture .pl{flex:1;}
  .au-row{display:flex; align-items:center; gap:11px; padding:7px 9px; border-radius:10px; cursor:pointer; color:var(--muted); font-size:13px; transition:background .14s;}
  .au-row:hover{background:var(--card-2);}
  .au-row .ic{color:var(--faint);}
  .au-row .lb{flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .au-row .mt{font-family:var(--mono); font-size:11px; color:var(--faint); white-space:nowrap;} .au-row .mt.urg{color:var(--coral);}
  .au-dot{width:8px;height:8px;border-radius:50%;flex-shrink:0;}
  .au-dot.event{background:var(--event);} .au-dot.task{background:var(--task);} .au-dot.note{background:var(--note);} .au-dot.project{background:var(--project);}
  .au-sub{font-family:var(--mono); font-size:10px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:8px 9px 4px;}
  .au-check{width:16px;height:16px;border-radius:6px;border:1.5px solid var(--faint);flex-shrink:0;} .au-check.task{border-color:var(--task);}
  .au-addw{display:flex;align-items:center;justify-content:center;gap:8px;margin-top:auto;padding:11px;border-radius:13px; white-space:nowrap;
    border:1px dashed var(--line-2); color:var(--faint); font-size:13px; cursor:pointer;}
  .au-addw:hover{color:var(--muted); background:var(--card);}

  /* ── panes ── */
  .au-main{flex:1; display:flex; gap:14px; min-width:0;}
  .au-focus{flex:1.7; min-width:0; display:flex; flex-direction:column; background:var(--surface); border:1px solid var(--line-2); border-radius:18px; overflow:hidden; box-shadow:var(--sh-3);}
  .au-focus::before{content:""; position:absolute; inset:0; background:radial-gradient(600px 280px at 30% -10%, var(--peri-soft), transparent 60%); pointer-events:none;}
  .au-focus{position:relative;}
  .au-refs{flex:1; min-width:0; display:flex; flex-direction:column; background:var(--surface); border:1px solid var(--line); border-radius:18px; overflow:hidden; box-shadow:var(--sh-2);}
  .au-pane-head{display:flex; align-items:center; gap:13px; padding:18px 22px 16px; border-bottom:1px solid var(--line); position:relative;}
  .au-pane-head .ttl{font-size:19px; font-weight:650; letter-spacing:-.02em;}
  .au-pane-head .sp{flex:1;} .au-pane-head .meta{font-family:var(--mono); font-size:11px; color:var(--faint);}
  .au-typetag{display:inline-flex; align-items:center; gap:7px; height:24px; padding:0 11px; border-radius:8px; font-size:11.5px; font-weight:500;
    background:var(--peri-soft); color:var(--peri);}
  .au-typetag .sw{width:7px;height:7px;border-radius:50%;background:var(--peri);}

  .au-outline{flex:1; overflow:hidden; padding:18px 22px; position:relative;}
  .au-blk{padding:4px 0;}
  .au-blk-main{display:flex; align-items:flex-start; gap:13px; padding:11px 14px; border-radius:13px; transition:all .16s;}
  .au-blk.sel > .au-blk-main{background:var(--card); box-shadow:var(--sh-2); border-left:3px solid var(--coral);}
  .au-bull{width:9px;height:9px;border-radius:50%;background:var(--faint);margin-top:7px;flex-shrink:0;}
  .au-blk.task > .au-blk-main > .au-bull{background:var(--task); box-shadow:0 0 0 4px rgba(240,117,140,.14);}
  .au-blk-body{flex:1 1 0%; min-width:0;}
  .au-blk-text{font-size:15.5px; color:var(--fg); line-height:1.5; letter-spacing:-.01em;}
  .au-tagchip{display:inline-flex; align-items:center; height:20px; padding:0 8px; margin-left:8px; border-radius:7px; font-size:11px; font-weight:500;
    vertical-align:2px; background:var(--coral-soft); color:var(--coral);}
  .au-tagchip.alt{background:rgba(240,117,140,.15); color:var(--task);}
  .au-props{display:flex; flex-wrap:wrap; gap:8px; margin-top:11px;}
  .au-pchip{display:inline-flex; align-items:center; gap:8px; height:27px; padding:0 12px; border-radius:9px; white-space:nowrap; flex-shrink:0;
    background:var(--card-2); font-family:var(--mono); font-size:11.5px;}
  .au-pchip .k{color:var(--faint);} .au-pchip .v{color:var(--muted);} .au-pchip.doing .v{color:var(--coral); font-weight:600;}
  .au-pchip.high .v{color:var(--task); font-weight:600;} .au-pchip .lk{color:var(--peri);}
  .au-kids{margin:8px 0 2px 22px; display:flex; flex-direction:column; gap:3px;}
  .au-kid{display:flex; align-items:flex-start; gap:11px; padding:7px 12px; border-radius:10px; cursor:pointer; transition:background .14s;}
  .au-kid:hover{background:var(--card-2);}
  .au-kid .kb{width:6px;height:6px;border-radius:50%;background:var(--faint);margin-top:8px;flex-shrink:0;}
  .au-kid .kt{font-size:14px; color:var(--muted); flex:1 1 0%; min-width:0;}
  .au-mention{color:var(--person); background:rgba(183,156,240,.15); padding:0 5px; border-radius:5px;}
  .au-link{color:var(--peri); background:var(--peri-soft); padding:0 5px; border-radius:5px;}

  .au-refs-body{flex:1; overflow:hidden; padding:16px 18px; display:flex; flex-direction:column; gap:11px;}
  .au-refcard{padding:13px 15px; border-radius:13px; background:var(--card); border:1px solid var(--line); box-shadow:var(--sh-1);}
  .au-refcard .src{display:flex; align-items:center; gap:9px; font-family:var(--mono); font-size:11px; color:var(--muted); margin-bottom:6px;}
  .au-refcard .snip{font-size:13.5px; color:var(--subtle); line-height:1.55;}
  .au-refcard .snip em{font-style:normal; color:var(--fg); background:var(--coral-soft); padding:0 4px; border-radius:5px;}
  .au-refs-props{margin-top:3px; padding-top:13px; border-top:1px solid var(--line);}
  .au-refs-props .h{font-family:var(--mono); font-size:10px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:0 3px 8px;}
  .au-prow{display:grid; grid-template-columns:22px 82px 1fr; gap:10px; align-items:center; padding:6px 9px; border-radius:9px; font-family:var(--mono); font-size:12px;}
  .au-prow:hover{background:var(--card-2);}
  .au-prow .chord{color:var(--peri); text-align:center; background:var(--card-2); border-radius:6px; padding:3px 0;}
  .au-prow .k{color:var(--subtle);} .au-prow .v{color:var(--muted); white-space:nowrap;} .au-prow .v.doing{color:var(--coral);} .au-prow .v.high{color:var(--task);}

  /* ── status ── */
  .au-status{display:flex; align-items:center; gap:14px; padding:0 18px; font-family:var(--mono); font-size:11px; color:var(--subtle); border-top:1px solid var(--line); white-space:nowrap; overflow:hidden;}
  .au-status .mode{display:inline-flex; align-items:center; height:18px; padding:0 10px; border-radius:9px; background:var(--coral); color:#0C0E16; font-weight:700; letter-spacing:.10em; font-size:9.5px;}
  .au-status .sep{color:var(--faint);}
  .au-status .keys{margin-left:auto; display:flex; gap:16px;} .au-status .keys span{color:var(--faint);} .au-status .keys kbd{color:var(--peri);}
  .au-status .clk{color:var(--faint); display:flex; align-items:center; gap:5px;}
  `;
  const el = document.createElement('style'); el.id = 'au-styles'; el.textContent = css; document.head.appendChild(el);
})();

function Aurora() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <div className="au-root">
      {/* top bar */}
      <div className="au-top">
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <div className="au-brand"><MosaicMark size={20} tile="#8C9EFF" accent="#FF6A5D" gap={1.6} /><span className="nm">tesela</span></div>
          <div className="au-tabs">
            {TABS.map((t) => (
              <div key={t.id} className={'au-tab' + (t.active ? ' active' : '')}>
                {t.active && <span className="kdot" />}<span>{t.name}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="au-cmd"><Icon name="search" size={16} /><span className="ph">Search or run a command…</span><kbd>⌘K</kbd></div>
        <div className="au-icons">
          <div className="au-ic"><Icon name="microphone" size={17} /></div>
          <div className="au-conn"><i /></div>
          <div className="au-ic"><Icon name="graph" size={17} /></div>
          <div className="au-ic"><Icon name="settings" size={17} /></div>
        </div>
      </div>

      {/* body */}
      <div className="au-body">
        <div className="au-rail">
          <div className="au-w">
            <div className="au-w-head"><span className="gl" style={{ background: 'linear-gradient(135deg,#FF8A6D,#FF6A5D)' }}><Icon name="bolt" size={14} /></span><span className="ti">Quick capture</span></div>
            <div className="au-capture"><span className="pl">Capture a thought…</span><Icon name="plus" size={15} /></div>
          </div>
          <div className="au-w">
            <div className="au-w-head"><span className="gl" style={{ background: 'linear-gradient(135deg,#F58AA0,#F0758C)' }}><Icon name="pin" size={14} /></span><span className="ti">Pinned</span></div>
            {WIDGETS.pinned.items.map((it, i) => (
              <div key={i} className="au-row"><Icon name={it.icon} size={15} className="ic" /><span className="lb">{it.label}</span></div>
            ))}
          </div>
          <div className="au-w">
            <div className="au-w-head"><span className="gl" style={{ background: 'linear-gradient(135deg,#F0C57E,#E7B36A)' }}><Icon name="sun" size={14} /></span><span className="ti">Today</span><span className="bd">Apr 10</span></div>
            {WIDGETS.today.items.map((it, i) => (
              <div key={i} className="au-row"><span className={'au-dot ' + it.kind} /><span className="lb">{it.label}</span><span className={'mt' + (it.urgent ? ' urg' : '')}>{it.meta}</span></div>
            ))}
          </div>
          <div className="au-w">
            <div className="au-w-head"><span className="gl" style={{ background: 'linear-gradient(135deg,#7FCFE2,#67C0D6)' }}><Icon name="squareCheck" size={14} /></span><span className="ti">Tasks</span><span className="bd">8 open</span></div>
            {WIDGETS.tasks.groups.map((g, i) => (
              <div key={i}>
                <div className="au-sub">{g.sub}</div>
                {g.items.map((it, j) => (
                  <div key={j} className="au-row"><span className="au-check task" /><span className="lb">{it.label}</span>{it.pri === 'high' && <Icon name="flame" size={14} color="var(--task)" />}</div>
                ))}
              </div>
            ))}
          </div>
          <div className="au-addw"><Icon name="plus" size={15} />Add widget</div>
        </div>

        <div className="au-main">
          <div className="au-focus">
            <div className="au-pane-head">
              <span className="ttl">Ship the docs refresh</span>
              <span className="au-typetag"><span className="sw" />Project</span>
              <span className="sp" /><span className="meta">2 linked</span>
              <Icon name="dotsVertical" size={17} color="var(--faint)" />
            </div>
            <div className="au-outline">
              {OUTLINE.map((b) => (
                <div key={b.id} className={'au-blk' + (b.tag === 'Task' ? ' task' : '') + (b.selected ? ' sel' : '')}>
                  <div className="au-blk-main">
                    <span className="au-bull" />
                    <div className="au-blk-body">
                      <div className="au-blk-text">
                        {b.link ? (<>Weekly sync with <span className="au-link">[[Domain]]</span> leads</>) : b.text}
                        {b.tag && <span className="au-tagchip">#{b.tag}</span>}
                        {b.tag2 && <span className="au-tagchip alt">#{b.tag2}</span>}
                      </div>
                      {b.props && b.props.length > 0 && (
                        <div className="au-props">
                          {b.props.map((p, i) => (
                            <span key={i} className={'au-pchip ' + tone(p.tone)}>
                              <span className="k">{p.k}</span>
                              {p.link ? <span className="lk">[[{p.v}]]</span> : <span className="v">{p.v}</span>}
                            </span>
                          ))}
                        </div>
                      )}
                      {b.children && b.children.length > 0 && (
                        <div className="au-kids">
                          {b.children.map((k) => (
                            <div key={k.id} className="au-kid">
                              <span className="kb" />
                              <span className="kt">{k.mention ? (<>Review with <span className="au-mention">@Mara</span> on the Domain team</>) : k.text}</span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="au-refs">
            <div className="au-pane-head"><span className="ttl" style={{ fontSize: 15 }}>Linked references</span><span className="sp" /><span className="meta">3</span></div>
            <div className="au-refs-body">
              {BACKLINKS.map((r, i) => (
                <div key={i} className="au-refcard">
                  <div className="src"><span className={'au-dot ' + (r.kind === 'project' ? 'project' : r.kind === 'daily' ? 'event' : 'note')} />{r.src}</div>
                  <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
                </div>
              ))}
              <div className="au-refs-props">
                <div className="h">Properties · b1</div>
                {PROPS.map((p, i) => (
                  <div key={i} className="au-prow"><span className="chord">{p.chord}</span><span className="k">{p.k}</span><span className={'v ' + (p.tone || '')}>{p.v}</span></div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* status */}
      <div className="au-status">
        <span className="mode">{STATUS.mode}</span>
        <span>{STATUS.path}</span>
        <span className="keys">
          {STATUS.keys.map((k, i) => (<span key={i}><kbd>{k.k}</kbd> {k.label}</span>))}
          <span className="clk"><Icon name="clock" size={12} color="var(--faint)" />14:08</span>
        </span>
      </div>
    </div>
  );
}

Object.assign(window, { Aurora });

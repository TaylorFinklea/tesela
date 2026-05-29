/* Direction 1 — GRAPHITE
   Linear/Things-grade restraint. Cool graphite surfaces, hairline borders,
   one disciplined coral accent, Geist-forward type with mono only for
   metadata. Density ~60: efficient but breathing. Keyboard hints stay quiet
   (hover/focus). The "safe, premium baseline." */

(function () {
  if (document.getElementById('gr-styles')) return;
  const css = `
  .gr-root{
    --bg:#0E1014; --surface:#14171D; --raised:#1A1E26; --raised-2:#20242D;
    --line:rgba(255,255,255,.06); --line-2:rgba(255,255,255,.10);
    --fg:#E7E9ED; --muted:#9AA0AB; --subtle:#6A707B; --faint:#474D58;
    --coral:#FF6B5A; --coral-dim:rgba(255,107,90,.14);
    --task:#E5677F; --event:#5FB6CC; --note:#E0A862; --project:#6E8FE6; --person:#A98BE0; --query:#82B85E;
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    position:absolute; inset:0; background:var(--bg); color:var(--fg);
    font-family:var(--sans); font-size:13.5px; line-height:1.5;
    display:grid; grid-template-rows:48px 1fr 30px; overflow:hidden;
    -webkit-font-smoothing:antialiased;
  }
  .gr-root *{box-sizing:border-box;}
  .gr-mono{font-family:var(--mono);}

  /* ── top bar ── */
  .gr-top{display:grid; grid-template-columns:auto 1fr auto; align-items:center; gap:18px;
    padding:0 16px; border-bottom:1px solid var(--line); background:var(--surface);}
  .gr-brand{display:flex; align-items:center; gap:9px;}
  .gr-brand .nm{font-size:13.5px; font-weight:600; letter-spacing:-.01em; color:var(--fg);}
  .gr-tabs{display:flex; align-items:center; gap:4px; margin-left:6px;}
  .gr-tab{display:flex; align-items:center; gap:7px; height:30px; padding:0 11px; border-radius:8px; white-space:nowrap;
    color:var(--subtle); cursor:pointer; font-size:12.5px; border:1px solid transparent; transition:all .14s;}
  .gr-tab:hover{color:var(--muted); background:var(--raised);}
  .gr-tab.active{color:var(--fg); background:var(--raised); border-color:var(--line-2);}
  .gr-tab .kdot{width:6px; height:6px; border-radius:50%; background:var(--coral);}
  .gr-tab.active .nm{font-weight:550;}
  .gr-cmd{justify-self:center; width:min(440px,100%); display:flex; align-items:center; gap:9px;
    height:32px; padding:0 11px; border-radius:9px; background:var(--bg); border:1px solid var(--line-2);
    color:var(--subtle); cursor:text; transition:border-color .14s;}
  .gr-cmd:hover{border-color:rgba(255,255,255,.16);}
  .gr-cmd .ph{flex:1; font-size:12.5px;}
  .gr-cmd kbd{font-family:var(--mono); font-size:10.5px; color:var(--subtle); background:var(--raised);
    border:1px solid var(--line); border-radius:5px; padding:2px 6px; line-height:1;}
  .gr-icons{display:flex; align-items:center; gap:2px;}
  .gr-ic{width:30px; height:30px; display:grid; place-items:center; border-radius:8px; color:var(--subtle);
    cursor:pointer; transition:all .14s;}
  .gr-ic:hover{color:var(--fg); background:var(--raised);}
  .gr-conn{width:30px; height:30px; display:grid; place-items:center;}
  .gr-conn i{width:7px; height:7px; border-radius:50%; background:var(--query); box-shadow:0 0 0 3px rgba(130,184,94,.14);}

  /* ── body ── */
  .gr-body{display:flex; min-height:0; overflow:hidden;}
  .gr-rail{width:256px; flex-shrink:0; background:var(--surface); border-right:1px solid var(--line);
    display:flex; flex-direction:column; min-height:0;}
  .gr-rail-scroll{flex:1; overflow:hidden; padding:12px 10px; display:flex; flex-direction:column; gap:8px;}

  .gr-w{background:var(--raised); border:1px solid var(--line); border-radius:11px; overflow:hidden;}
  .gr-w-head{display:flex; align-items:center; gap:8px; padding:9px 11px 7px;}
  .gr-w-head .ti{flex:1; font-size:11px; font-weight:600; letter-spacing:.04em; text-transform:uppercase; color:var(--subtle);}
  .gr-w-head .ic{color:var(--faint);}
  .gr-w-head .bd{font-family:var(--mono); font-size:10px; color:var(--faint); background:var(--bg); white-space:nowrap;
    border:1px solid var(--line); border-radius:5px; padding:1px 6px;}
  .gr-w-body{padding:2px 7px 9px;}

  .gr-capture{display:flex; align-items:center; gap:8px; margin:0 4px 4px; padding:9px 11px; border-radius:8px;
    background:var(--bg); border:1px solid var(--line); color:var(--faint); font-size:12.5px;}
  .gr-capture .pl{flex:1;}
  .gr-capture .pk{font-family:var(--mono); font-size:10px; color:var(--faint);}

  .gr-row{display:flex; align-items:center; gap:9px; padding:6px 8px; border-radius:7px; cursor:pointer;
    color:var(--muted); font-size:12.5px; transition:background .12s;}
  .gr-row:hover{background:var(--raised-2);}
  .gr-row .ic{color:var(--faint);}
  .gr-row .lb{flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .gr-row .mt{font-family:var(--mono); font-size:10.5px; color:var(--faint); font-variant-numeric:tabular-nums;}
  .gr-row .mt.urg{color:var(--coral);}
  .gr-dot{width:6px; height:6px; border-radius:50%; flex-shrink:0;}
  .gr-dot.event{background:var(--event);} .gr-dot.task{background:var(--task);}
  .gr-dot.note{background:var(--note);} .gr-dot.project{background:var(--project);}
  .gr-sub{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase;
    color:var(--faint); padding:7px 8px 3px;}
  .gr-check{width:14px; height:14px; border-radius:4px; border:1.5px solid var(--faint); flex-shrink:0;}
  .gr-check.task{border-color:color-mix(in srgb,var(--task) 70%,transparent);}
  .gr-addw{display:flex; align-items:center; justify-content:center; gap:7px; margin-top:auto;
    padding:9px; border-radius:8px; border:1px dashed var(--line-2); color:var(--faint); font-size:12px; cursor:pointer;}
  .gr-addw:hover{color:var(--muted); border-color:rgba(255,255,255,.18);}

  /* ── main split ── */
  .gr-main{flex:1; display:flex; min-width:0; min-height:0;}
  .gr-focus{flex:1.7; min-width:0; display:flex; flex-direction:column; background:var(--bg);}
  .gr-refs{flex:1; min-width:0; display:flex; flex-direction:column; background:var(--surface); border-left:1px solid var(--line);}

  .gr-pane-head{display:flex; align-items:center; gap:11px; padding:14px 18px 12px; border-bottom:1px solid var(--line);}
  .gr-back{color:var(--faint); cursor:pointer;}
  .gr-pane-head .ttl{font-size:16px; font-weight:600; letter-spacing:-.01em; color:var(--fg);}
  .gr-typetag{display:inline-flex; align-items:center; gap:6px; height:21px; padding:0 9px; border-radius:6px;
    font-family:var(--mono); font-size:10.5px; letter-spacing:.02em; background:var(--raised); border:1px solid var(--line-2); color:var(--project);}
  .gr-typetag .sw{width:6px; height:6px; border-radius:2px; background:var(--project);}
  .gr-pane-head .sp{flex:1;}
  .gr-pane-head .meta{font-family:var(--mono); font-size:10.5px; color:var(--faint);}

  .gr-outline{flex:1; overflow:hidden; padding:14px 18px;}
  .gr-blk{position:relative; padding:7px 10px 7px 8px; border-radius:9px; border-left:2px solid transparent;}
  .gr-blk.sel{background:var(--raised); border-left-color:var(--coral);}
  .gr-blk-main{display:flex; align-items:flex-start; gap:10px;}
  .gr-bull{width:7px; height:7px; border-radius:50%; background:var(--faint); margin-top:7px; flex-shrink:0;}
  .gr-blk.task > .gr-blk-main > .gr-bull{background:var(--task);}
  .gr-blk-body{flex:1 1 0%; min-width:0;}
  .gr-blk-text{font-size:14.5px; color:var(--fg); line-height:1.45; letter-spacing:-.005em;}
  .gr-tagchip{display:inline-flex; align-items:center; height:18px; padding:0 7px; margin-left:7px; border-radius:5px;
    font-family:var(--mono); font-size:10.5px; vertical-align:1px; background:var(--coral-dim); color:var(--coral);}
  .gr-tagchip.alt{background:rgba(229,103,127,.14); color:var(--task);}
  .gr-props{display:flex; flex-wrap:wrap; gap:6px; margin-top:8px;}
  .gr-pchip{display:inline-flex; align-items:center; gap:7px; height:23px; padding:0 9px; border-radius:7px; white-space:nowrap; flex-shrink:0;
    background:var(--surface); border:1px solid var(--line); font-family:var(--mono); font-size:11px;}
  .gr-pchip .k{color:var(--faint);} .gr-pchip .v{color:var(--muted);}
  .gr-pchip.doing .v{color:var(--coral); font-weight:600;}
  .gr-pchip.high .v{color:var(--task); font-weight:600;}
  .gr-pchip .lk{color:var(--project);}
  .gr-kids{margin:6px 0 2px 18px; padding-left:14px; border-left:1px solid var(--line);}
  .gr-kid{display:flex; align-items:center; gap:9px; padding:5px 6px; border-radius:7px; cursor:pointer;}
  .gr-kid:hover{background:var(--raised);}
  .gr-kid .kb{width:5px; height:5px; border-radius:50%; background:var(--faint); flex-shrink:0;}
  .gr-kid .kt{font-size:13px; color:var(--muted); flex:1 1 0%; min-width:0;}
  .gr-mention{color:var(--person); background:rgba(169,139,224,.13); padding:0 4px; border-radius:4px;}
  .gr-link{color:var(--project); background:rgba(110,143,230,.13); padding:0 4px; border-radius:4px;}

  /* ── refs pane ── */
  .gr-refs .gr-pane-head{padding:14px 16px 12px;}
  .gr-refs-ttl{font-size:12.5px; font-weight:600; color:var(--muted); letter-spacing:.01em;}
  .gr-refs-body{flex:1; overflow:hidden; padding:12px 14px; display:flex; flex-direction:column; gap:9px;}
  .gr-refcard{padding:10px 12px; border-radius:10px; background:var(--raised); border:1px solid var(--line);}
  .gr-refcard .src{display:flex; align-items:center; gap:7px; font-family:var(--mono); font-size:10.5px; color:var(--muted); margin-bottom:5px;}
  .gr-refcard .snip{font-size:12.5px; color:var(--subtle); line-height:1.5;}
  .gr-refcard .snip em{font-style:normal; color:var(--fg); background:var(--coral-dim); padding:0 3px; border-radius:3px;}
  .gr-proplist{margin-top:4px;}
  .gr-proplist .ph{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:4px 2px 7px;}
  .gr-prow{display:grid; grid-template-columns:18px 78px 1fr; align-items:center; gap:8px; padding:5px 7px; border-radius:7px;}
  .gr-prow:hover{background:var(--raised);}
  .gr-prow .chord{font-family:var(--mono); font-size:9.5px; text-align:center; color:var(--faint);
    background:var(--surface); border:1px solid var(--line); border-radius:4px; padding:2px 0;}
  .gr-prow .k{font-family:var(--mono); font-size:11px; color:var(--subtle);}
  .gr-prow .v{font-family:var(--mono); font-size:11px; color:var(--muted); justify-self:start; white-space:nowrap;}
  .gr-prow .v.doing{color:var(--coral);} .gr-prow .v.high{color:var(--task);}

  /* ── status line ── */
  .gr-status{display:flex; align-items:center; gap:12px; padding:0 14px; background:var(--surface); white-space:nowrap; overflow:hidden;
    border-top:1px solid var(--line); font-family:var(--mono); font-size:11px; color:var(--subtle);}
  .gr-status .mode{color:var(--coral); font-weight:700; letter-spacing:.10em; font-size:10px;}
  .gr-status .sep{color:var(--faint);}
  .gr-status .keys{margin-left:auto; display:flex; gap:14px;}
  .gr-status .keys span{color:var(--faint);} .gr-status .keys kbd{color:var(--muted); font-family:var(--mono);}
  .gr-status .clk{color:var(--faint); display:flex; align-items:center; gap:5px;}
  `;
  const el = document.createElement('style');
  el.id = 'gr-styles';
  el.textContent = css;
  document.head.appendChild(el);
})();

function Graphite() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <div className="gr-root">
      {/* top bar */}
      <div className="gr-top">
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <div className="gr-brand">
            <MosaicMark size={18} tile="#7E8AA8" accent="#FF6B5A" />
            <span className="nm">tesela</span>
          </div>
          <div className="gr-tabs">
            {TABS.map((t) => (
              <div key={t.id} className={'gr-tab' + (t.active ? ' active' : '')}>
                {t.active && <span className="kdot" />}
                <span className="nm">{t.name}</span>
              </div>
            ))}
            <div className="gr-ic" style={{ width: 26, height: 26 }}><Icon name="plus" size={15} /></div>
          </div>
        </div>
        <div className="gr-cmd">
          <Icon name="search" size={15} />
          <span className="ph">Search or run a command…</span>
          <kbd>⌘K</kbd>
        </div>
        <div className="gr-icons">
          <div className="gr-ic"><Icon name="microphone" size={16} /></div>
          <div className="gr-conn"><i /></div>
          <div className="gr-ic"><Icon name="graph" size={16} /></div>
          <div className="gr-ic"><Icon name="settings" size={16} /></div>
        </div>
      </div>

      {/* body */}
      <div className="gr-body">
        {/* widget rail */}
        <div className="gr-rail">
          <div className="gr-rail-scroll">
            {/* capture */}
            <div className="gr-w">
              <div className="gr-w-head"><Icon name="bolt" size={13} className="ic" /><span className="ti">Quick capture</span></div>
              <div className="gr-w-body">
                <div className="gr-capture"><span className="pl">Capture a thought…</span><span className="pk">C</span></div>
              </div>
            </div>
            {/* pinned */}
            <div className="gr-w">
              <div className="gr-w-head"><Icon name="pin" size={13} className="ic" /><span className="ti">Pinned</span></div>
              <div className="gr-w-body">
                {WIDGETS.pinned.items.map((it, i) => (
                  <div key={i} className="gr-row"><Icon name={it.icon} size={14} className="ic" /><span className="lb">{it.label}</span></div>
                ))}
              </div>
            </div>
            {/* today */}
            <div className="gr-w">
              <div className="gr-w-head"><Icon name="sun" size={13} className="ic" /><span className="ti">Today</span><span className="bd">{WIDGETS.today.badge}</span></div>
              <div className="gr-w-body">
                {WIDGETS.today.items.map((it, i) => (
                  <div key={i} className="gr-row">
                    <span className={'gr-dot ' + it.kind} />
                    <span className="lb">{it.label}</span>
                    <span className={'mt' + (it.urgent ? ' urg' : '')}>{it.meta}</span>
                  </div>
                ))}
              </div>
            </div>
            {/* tasks */}
            <div className="gr-w">
              <div className="gr-w-head"><Icon name="squareCheck" size={13} className="ic" /><span className="ti">Tasks</span><span className="bd">{WIDGETS.tasks.badge}</span></div>
              <div className="gr-w-body">
                {WIDGETS.tasks.groups.map((g, i) => (
                  <div key={i}>
                    <div className="gr-sub">{g.sub}</div>
                    {g.items.map((it, j) => (
                      <div key={j} className="gr-row"><span className="gr-check task" /><span className="lb">{it.label}</span>{it.pri === 'high' && <Icon name="flame" size={13} color="var(--task)" />}</div>
                    ))}
                  </div>
                ))}
              </div>
            </div>
            <div className="gr-addw"><Icon name="plus" size={14} />Add widget</div>
          </div>
        </div>

        {/* focus + refs */}
        <div className="gr-main">
          <div className="gr-focus">
            <div className="gr-pane-head">
              <Icon name="chevronRight" size={16} className="gr-back" style={{ transform: 'rotate(180deg)' }} />
              <span className="ttl">Ship the docs refresh</span>
              <span className="gr-typetag"><span className="sw" />Project</span>
              <span className="sp" />
              <span className="meta">2 linked</span>
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
                            <span key={i} className={'gr-pchip ' + tone(p.tone)}>
                              <span className="k">{p.k}</span>
                              {p.link ? <span className="lk">[[{p.v}]]</span> : <span className="v">{p.v}</span>}
                            </span>
                          ))}
                        </div>
                      )}
                      {b.children && b.children.length > 0 && (
                        <div className="gr-kids">
                          {b.children.map((k) => (
                            <div key={k.id} className="gr-kid">
                              <span className="kb" />
                              <span className="kt">{k.mention ? (<>Review with <span className="gr-mention">@Mara</span> on the Domain team</>) : k.text}</span>
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

          <div className="gr-refs">
            <div className="gr-pane-head"><span className="gr-refs-ttl">Linked references</span><span className="sp" /><span className="meta">3</span></div>
            <div className="gr-refs-body">
              {BACKLINKS.map((r, i) => (
                <div key={i} className="gr-refcard">
                  <div className="src"><span className={'gr-dot ' + (r.kind === 'project' ? 'project' : r.kind === 'daily' ? 'event' : 'note')} />{r.src}</div>
                  <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
                </div>
              ))}
              <div className="gr-proplist">
                <div className="ph">Properties</div>
                {PROPS.map((p, i) => (
                  <div key={i} className="gr-prow">
                    <span className="chord">{p.chord}</span>
                    <span className="k">{p.k}</span>
                    <span className={'v ' + (p.tone || '')}>{p.v}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* status */}
      <div className="gr-status">
        <span className="mode">{STATUS.mode}</span>
        <span className="sep">·</span>
        <span>{STATUS.path}</span>
        <span className="keys">
          {STATUS.keys.map((k, i) => (<span key={i}><kbd>{k.k}</kbd> {k.label}</span>))}
          <span className="clk"><Icon name="clock" size={12} color="var(--faint)" />14:08</span>
        </span>
      </div>
    </div>
  );
}

Object.assign(window, { Graphite });

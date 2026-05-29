/* Direction 5 — SPARK
   Austere, near-monochrome zinc. Coral is the ONLY color, spent with
   discipline: the mode, the active tab, the selected block's accent + its
   status value, one highlighted reference. Everything else is grayscale —
   kinds read through shade, weight, and Tabler iconography, not hue.
   Hairline dividers, generous negative space. Vercel/Linear-grade restraint;
   the biggest departure from warm Prism. Chrome is quiet but legible. */

(function () {
  if (document.getElementById('sp-styles')) return;
  const css = `
  .sp-root{
    --bg:#0A0A0B; --surface:#0E0E10; --panel:#141416; --raised:#1A1A1D; --raised-2:#202023;
    --line:rgba(255,255,255,.07); --line-2:rgba(255,255,255,.12);
    --fg:#FAFAFA; --muted:#A1A1A6; --subtle:#727279; --faint:#52525B; --ghost:#3A3A3F;
    --coral:#FF5640; --coral-soft:rgba(255,86,64,.13);
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    position:absolute; inset:0; background:var(--bg); color:var(--fg);
    font-family:var(--sans); font-size:13.5px; line-height:1.55;
    display:grid; grid-template-rows:50px 1fr 28px; overflow:hidden; -webkit-font-smoothing:antialiased;
  }
  .sp-root *{box-sizing:border-box;}

  /* ── top bar ── */
  .sp-top{display:grid; grid-template-columns:auto 1fr auto; align-items:center; gap:20px; padding:0 18px; border-bottom:1px solid var(--line);}
  .sp-brand{display:flex; align-items:center; gap:10px;}
  .sp-brand .nm{font-size:14px; font-weight:600; letter-spacing:-.01em;}
  .sp-tabs{display:flex; align-items:center; gap:2px; margin-left:10px;}
  .sp-tab{display:flex; align-items:center; gap:8px; height:30px; padding:0 12px; border-radius:7px; white-space:nowrap;
    color:var(--subtle); font-size:13px; cursor:pointer; transition:all .14s;}
  .sp-tab:hover{color:var(--muted);}
  .sp-tab.active{color:var(--fg);}
  .sp-tab .kdot{width:6px;height:6px;border-radius:50%;background:var(--coral);}
  .sp-tab.active{position:relative;}
  .sp-tab.active::after{content:""; position:absolute; left:12px; right:12px; bottom:-1px; height:2px; background:var(--coral); border-radius:2px;}
  .sp-cmd{justify-self:center; width:min(440px,100%); display:flex; align-items:center; gap:10px; height:34px; padding:0 13px;
    border-radius:9px; background:var(--panel); border:1px solid var(--line); color:var(--subtle); font-size:13px;}
  .sp-cmd .ph{flex:1;} .sp-cmd kbd{font-family:var(--mono); font-size:10.5px; color:var(--subtle); background:var(--raised); border:1px solid var(--line); border-radius:5px; padding:2px 6px;}
  .sp-icons{display:flex; align-items:center; gap:2px;}
  .sp-ic{width:32px;height:32px;display:grid;place-items:center;border-radius:8px;color:var(--subtle);cursor:pointer;transition:all .14s;}
  .sp-ic:hover{color:var(--fg); background:var(--panel);}
  .sp-conn{width:32px;display:grid;place-items:center;} .sp-conn i{width:7px;height:7px;border-radius:50%;background:var(--muted);box-shadow:0 0 0 3px rgba(255,255,255,.05);}

  /* ── body ── */
  .sp-body{display:flex; min-height:0; overflow:hidden;}
  .sp-rail{width:262px; flex-shrink:0; background:var(--surface); border-right:1px solid var(--line); display:flex; flex-direction:column; overflow:hidden;}
  .sp-rail-scroll{flex:1; overflow:hidden; padding:8px 12px; display:flex; flex-direction:column;}
  .sp-w{padding:12px 0; border-bottom:1px solid var(--line);}
  .sp-w:first-child{padding-top:6px;}
  .sp-w-head{display:flex; align-items:center; gap:9px; padding:0 4px 9px;}
  .sp-w-head .ic{color:var(--faint);}
  .sp-w-head .ti{flex:1; font-size:10.5px; font-weight:600; letter-spacing:.12em; text-transform:uppercase; color:var(--subtle);}
  .sp-w-head .bd{font-family:var(--mono); font-size:10px; color:var(--faint); white-space:nowrap;}
  .sp-capture{display:flex; align-items:center; gap:9px; padding:9px 11px; border-radius:8px; background:var(--panel); border:1px solid var(--line); color:var(--faint); font-size:13px;}
  .sp-capture .pl{flex:1;} .sp-capture .pk{font-family:var(--mono); font-size:10px; color:var(--ghost);}
  .sp-row{display:flex; align-items:center; gap:11px; padding:6px 5px; border-radius:7px; cursor:pointer; color:var(--muted); font-size:13px; transition:background .12s;}
  .sp-row:hover{background:var(--panel);}
  .sp-row .ic{color:var(--faint);}
  .sp-row .lb{flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .sp-row .mt{font-family:var(--mono); font-size:10.5px; color:var(--faint); white-space:nowrap;} .sp-row .mt.urg{color:var(--coral);}
  .sp-tick{width:5px;height:5px;border-radius:50%;background:var(--ghost);flex-shrink:0;}
  .sp-tick.urg{background:var(--coral);}
  .sp-sub{font-family:var(--mono); font-size:9.5px; letter-spacing:.12em; text-transform:uppercase; color:var(--faint); padding:7px 5px 4px;}
  .sp-check{width:15px;height:15px;border-radius:5px;border:1.5px solid var(--faint);flex-shrink:0;}
  .sp-addw{display:flex;align-items:center;justify-content:center;gap:8px;margin-top:auto;padding:12px;color:var(--faint);font-size:12.5px;cursor:pointer;border-top:1px solid var(--line); white-space:nowrap;}
  .sp-addw:hover{color:var(--muted);}

  /* ── panes ── */
  .sp-main{flex:1; display:flex; min-width:0;}
  .sp-focus{flex:1.7; min-width:0; display:flex; flex-direction:column; background:var(--bg);}
  .sp-refs{flex:1; min-width:0; display:flex; flex-direction:column; background:var(--surface); border-left:1px solid var(--line);}
  .sp-pane-head{display:flex; align-items:center; gap:14px; padding:20px 26px 17px; border-bottom:1px solid var(--line);}
  .sp-pane-head .ttl{font-size:20px; font-weight:650; letter-spacing:-.025em;}
  .sp-pane-head .sp{flex:1;} .sp-pane-head .meta{font-family:var(--mono); font-size:11px; color:var(--faint);}
  .sp-typetag{display:inline-flex; align-items:center; height:22px; padding:0 10px; border-radius:6px; font-family:var(--mono); font-size:10.5px; letter-spacing:.02em;
    background:var(--raised); color:var(--muted); border:1px solid var(--line);}

  .sp-outline{flex:1; overflow:hidden; padding:18px 26px;}
  .sp-blk{padding:2px 0;}
  .sp-blk-main{display:flex; align-items:flex-start; gap:14px; padding:10px 14px; border-radius:9px; border-left:2px solid transparent;}
  .sp-blk.sel > .sp-blk-main{background:var(--panel); border-left-color:var(--coral);}
  .sp-bull{width:7px;height:7px;border-radius:50%;background:var(--ghost);margin-top:8px;flex-shrink:0;}
  .sp-blk.sel > .sp-blk-main > .sp-bull{background:var(--coral);}
  .sp-blk-body{flex:1 1 0%; min-width:0;}
  .sp-blk-text{font-size:15.5px; color:var(--fg); line-height:1.5; letter-spacing:-.012em;}
  .sp-tagchip{display:inline-flex; align-items:center; height:19px; padding:0 8px; margin-left:9px; border-radius:6px; font-family:var(--mono); font-size:10.5px;
    vertical-align:2px; background:var(--raised); color:var(--muted); border:1px solid var(--line);}
  .sp-blk.sel .sp-tagchip{background:var(--coral-soft); color:var(--coral); border-color:transparent;}
  .sp-props{display:flex; flex-wrap:wrap; gap:8px; margin-top:11px;}
  .sp-pchip{display:inline-flex; align-items:center; gap:8px; height:25px; padding:0 11px; border-radius:7px; white-space:nowrap; flex-shrink:0;
    background:var(--raised); border:1px solid var(--line); font-family:var(--mono); font-size:11px;}
  .sp-pchip .k{color:var(--faint);} .sp-pchip .v{color:var(--muted);}
  .sp-pchip.doing .v{color:var(--coral); font-weight:600;} .sp-pchip.high .v{color:var(--fg); font-weight:600;}
  .sp-pchip .lk{color:var(--fg);}
  .sp-kids{margin:8px 0 2px 21px; padding-left:16px; border-left:1px solid var(--line);}
  .sp-kid{display:flex; align-items:flex-start; gap:12px; padding:6px 8px; border-radius:7px; cursor:pointer;}
  .sp-kid:hover{background:var(--panel);}
  .sp-kid .kb{width:5px;height:5px;border-radius:50%;background:var(--ghost);margin-top:9px;flex-shrink:0;}
  .sp-kid .kt{font-size:14px; color:var(--muted); flex:1 1 0%; min-width:0;}
  .sp-mention{color:var(--fg); border-bottom:1px solid var(--line-2);}
  .sp-link{color:var(--fg); border-bottom:1px solid var(--line-2);}

  .sp-refs-body{flex:1; overflow:hidden; padding:16px 18px; display:flex; flex-direction:column;}
  .sp-refcard{padding:13px 6px; border-bottom:1px solid var(--line);}
  .sp-refcard:first-child{padding-top:4px;}
  .sp-refcard .src{display:flex; align-items:center; gap:10px; font-family:var(--mono); font-size:11px; color:var(--muted); margin-bottom:6px;}
  .sp-refcard .src .dot{width:5px;height:5px;border-radius:50%;background:var(--ghost);}
  .sp-refcard .snip{font-size:13.5px; color:var(--subtle); line-height:1.55;}
  .sp-refcard .snip em{font-style:normal; color:var(--fg); background:var(--coral-soft); padding:0 4px; border-radius:4px;}
  .sp-refs-props{margin-top:16px;}
  .sp-refs-props .h{font-family:var(--mono); font-size:9.5px; letter-spacing:.12em; text-transform:uppercase; color:var(--faint); padding:0 3px 9px;}
  .sp-prow{display:grid; grid-template-columns:20px 84px 1fr; gap:10px; align-items:center; padding:6px 9px; border-radius:8px; font-family:var(--mono); font-size:11.5px;}
  .sp-prow:hover{background:var(--panel);}
  .sp-prow .chord{color:var(--subtle); text-align:center; background:var(--raised); border:1px solid var(--line); border-radius:5px; padding:2px 0;}
  .sp-prow .k{color:var(--subtle);} .sp-prow .v{color:var(--muted); white-space:nowrap;} .sp-prow .v.doing{color:var(--coral);} .sp-prow .v.high{color:var(--fg);}

  /* ── status ── */
  .sp-status{display:flex; align-items:center; gap:14px; padding:0 18px; background:var(--surface); border-top:1px solid var(--line); white-space:nowrap; overflow:hidden;
    font-family:var(--mono); font-size:11px; color:var(--subtle);}
  .sp-status .mode{display:inline-flex; align-items:center; height:17px; padding:0 9px; border-radius:5px; background:var(--coral-soft); color:var(--coral); font-weight:700; letter-spacing:.12em; font-size:9.5px;}
  .sp-status .sep{color:var(--ghost);}
  .sp-status .keys{margin-left:auto; display:flex; gap:16px;} .sp-status .keys span{color:var(--faint);} .sp-status .keys kbd{color:var(--muted);}
  .sp-status .clk{color:var(--faint); display:flex; align-items:center; gap:5px;}
  `;
  const el = document.createElement('style'); el.id = 'sp-styles'; el.textContent = css; document.head.appendChild(el);
})();

function Spark() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <div className="sp-root">
      {/* top bar */}
      <div className="sp-top">
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <div className="sp-brand"><MosaicMark size={18} tile="#71717A" accent="#FF5640" gap={1.6} /><span className="nm">tesela</span></div>
          <div className="sp-tabs">
            {TABS.map((t) => (
              <div key={t.id} className={'sp-tab' + (t.active ? ' active' : '')}>
                {t.active && <span className="kdot" />}<span>{t.name}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="sp-cmd"><Icon name="search" size={15} /><span className="ph">Search or run a command…</span><kbd>⌘K</kbd></div>
        <div className="sp-icons">
          <div className="sp-ic"><Icon name="microphone" size={16} /></div>
          <div className="sp-conn"><i /></div>
          <div className="sp-ic"><Icon name="graph" size={16} /></div>
          <div className="sp-ic"><Icon name="settings" size={16} /></div>
        </div>
      </div>

      {/* body */}
      <div className="sp-body">
        <div className="sp-rail">
          <div className="sp-rail-scroll">
            <div className="sp-w">
              <div className="sp-w-head"><Icon name="bolt" size={13} className="ic" /><span className="ti">Quick capture</span></div>
              <div className="sp-capture"><span className="pl">Capture a thought…</span><span className="pk">C</span></div>
            </div>
            <div className="sp-w">
              <div className="sp-w-head"><Icon name="pin" size={13} className="ic" /><span className="ti">Pinned</span></div>
              {WIDGETS.pinned.items.map((it, i) => (
                <div key={i} className="sp-row"><Icon name={it.icon} size={15} className="ic" /><span className="lb">{it.label}</span></div>
              ))}
            </div>
            <div className="sp-w">
              <div className="sp-w-head"><Icon name="sun" size={13} className="ic" /><span className="ti">Today</span><span className="bd">Apr 10</span></div>
              {WIDGETS.today.items.map((it, i) => (
                <div key={i} className="sp-row"><span className={'sp-tick' + (it.urgent ? ' urg' : '')} /><span className="lb">{it.label}</span><span className={'mt' + (it.urgent ? ' urg' : '')}>{it.meta}</span></div>
              ))}
            </div>
            <div className="sp-w">
              <div className="sp-w-head"><Icon name="squareCheck" size={13} className="ic" /><span className="ti">Tasks</span><span className="bd">8</span></div>
              {WIDGETS.tasks.groups.map((g, i) => (
                <div key={i}>
                  <div className="sp-sub">{g.sub}</div>
                  {g.items.map((it, j) => (
                    <div key={j} className="sp-row"><span className="sp-check" /><span className="lb">{it.label}</span>{it.pri === 'high' && <Icon name="flame" size={13} color="var(--subtle)" />}</div>
                  ))}
                </div>
              ))}
            </div>
            <div className="sp-addw"><Icon name="plus" size={14} />Add widget</div>
          </div>
        </div>

        <div className="sp-main">
          <div className="sp-focus">
            <div className="sp-pane-head">
              <span className="ttl">Ship the docs refresh</span>
              <span className="sp-typetag">Project</span>
              <span className="sp" /><span className="meta">2 linked</span>
              <Icon name="dotsVertical" size={16} color="var(--faint)" />
            </div>
            <div className="sp-outline">
              {OUTLINE.map((b) => (
                <div key={b.id} className={'sp-blk' + (b.tag === 'Task' ? ' task' : '') + (b.selected ? ' sel' : '')}>
                  <div className="sp-blk-main">
                    <span className="sp-bull" />
                    <div className="sp-blk-body">
                      <div className="sp-blk-text">
                        {b.link ? (<>Weekly sync with <span className="sp-link">[[Domain]]</span> leads</>) : b.text}
                        {b.tag && <span className="sp-tagchip">#{b.tag}</span>}
                        {b.tag2 && <span className="sp-tagchip">#{b.tag2}</span>}
                      </div>
                      {b.props && b.props.length > 0 && (
                        <div className="sp-props">
                          {b.props.map((p, i) => (
                            <span key={i} className={'sp-pchip ' + tone(p.tone)}>
                              <span className="k">{p.k}</span>
                              {p.link ? <span className="lk">[[{p.v}]]</span> : <span className="v">{p.v}</span>}
                            </span>
                          ))}
                        </div>
                      )}
                      {b.children && b.children.length > 0 && (
                        <div className="sp-kids">
                          {b.children.map((k) => (
                            <div key={k.id} className="sp-kid">
                              <span className="kb" />
                              <span className="kt">{k.mention ? (<>Review with <span className="sp-mention">@Mara</span> on the Domain team</>) : k.text}</span>
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

          <div className="sp-refs">
            <div className="sp-pane-head"><span className="ttl" style={{ fontSize: 15 }}>Linked references</span><span className="sp" /><span className="meta">3</span></div>
            <div className="sp-refs-body">
              {BACKLINKS.map((r, i) => (
                <div key={i} className="sp-refcard">
                  <div className="src"><span className="dot" />{r.src}</div>
                  <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
                </div>
              ))}
              <div className="sp-refs-props">
                <div className="h">Properties · b1</div>
                {PROPS.map((p, i) => (
                  <div key={i} className="sp-prow"><span className="chord">{p.chord}</span><span className="k">{p.k}</span><span className={'v ' + (p.tone || '')}>{p.v}</span></div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* status */}
      <div className="sp-status">
        <span className="mode">{STATUS.mode}</span>
        <span className="sep">·</span><span>{STATUS.path}</span>
        <span className="keys">
          {STATUS.keys.map((k, i) => (<span key={i}><kbd>{k.k}</kbd> {k.label}</span>))}
          <span className="clk"><Icon name="clock" size={12} color="var(--faint)" />14:08</span>
        </span>
      </div>
    </div>
  );
}

Object.assign(window, { Spark });

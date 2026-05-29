/* Direction 3 — MOSAIC
   The brand made structural. Tesela = mosaic tiles (tesserae); the tagline is
   "drop thought-tiles." So here the UI IS a mosaic: every widget, pane, and
   *block* is a tile separated by thin seams. Focus = a coral tile-edge with a
   soft glow. Bullets are tiny tesserae colored by type. Homage to Prism's
   slate/coral, modernized cooler and deeper. */

(function () {
  if (document.getElementById('mo-styles')) return;
  const css = `
  .mo-root{
    --seam:#0B0C12; --bg:#0E0F17; --tile:#171925; --tile-2:#1E2130; --tile-hi:#252837;
    --line:rgba(255,255,255,.06); --line-2:rgba(255,255,255,.10);
    --fg:#E9E8F2; --muted:#A2A3B8; --subtle:#6E7084; --faint:#4A4C5E;
    --coral:#FF5630; --coral-soft:rgba(255,86,48,.13); --blue:#7E9BE6;
    --task:#E36A82; --event:#5FB6CC; --note:#E0A862; --project:#7E9BE6; --person:#A98BE0; --query:#82B85E;
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    position:absolute; inset:0; background:var(--seam); color:var(--fg);
    font-family:var(--sans); font-size:13.5px; line-height:1.5;
    display:grid; grid-template-rows:auto 1fr auto; gap:5px; padding:5px; overflow:hidden; -webkit-font-smoothing:antialiased;
  }
  .mo-root *{box-sizing:border-box;}
  .mo-tile{background:var(--tile); border:1px solid var(--line); border-radius:7px;}

  /* tesserae bullet */
  .mo-tess{width:8px;height:8px;border-radius:2px;flex-shrink:0;background:var(--faint);transform:rotate(0deg);}
  .mo-tess.task{background:var(--task);} .mo-tess.event{background:var(--event);} .mo-tess.note{background:var(--note);}
  .mo-tess.project{background:var(--project);} .mo-tess.person{background:var(--person);}

  /* ── top bar tile ── */
  .mo-top{height:46px; display:grid; grid-template-columns:auto 1fr auto; align-items:center; gap:16px; padding:0 14px;}
  .mo-brand{display:flex; align-items:center; gap:10px;}
  .mo-brand .nm{font-size:13.5px; font-weight:600; letter-spacing:-.01em;}
  .mo-tabs{display:flex; align-items:center; gap:5px; margin-left:4px;}
  .mo-tab{display:flex; align-items:center; gap:7px; height:28px; padding:0 11px; border-radius:6px; white-space:nowrap;
    color:var(--subtle); font-size:12.5px; cursor:pointer; background:var(--tile-2); border:1px solid transparent;}
  .mo-tab:hover{color:var(--muted);}
  .mo-tab.active{color:var(--fg); background:var(--tile-hi); border-color:var(--coral); box-shadow:0 0 0 3px var(--coral-soft);}
  .mo-cmd{justify-self:center; width:min(420px,100%); display:flex; align-items:center; gap:9px; height:32px; padding:0 12px;
    border-radius:7px; background:var(--bg); border:1px solid var(--line-2); color:var(--subtle); font-size:12.5px;}
  .mo-cmd .ph{flex:1;} .mo-cmd kbd{font-family:var(--mono); font-size:10.5px; color:var(--subtle); background:var(--tile-2); border:1px solid var(--line); border-radius:5px; padding:2px 6px;}
  .mo-icons{display:flex; align-items:center; gap:2px;}
  .mo-ic{width:30px;height:30px;display:grid;place-items:center;border-radius:7px;color:var(--subtle);cursor:pointer;}
  .mo-ic:hover{color:var(--fg); background:var(--tile-2);}
  .mo-conn{width:30px;display:grid;place-items:center;} .mo-conn i{width:7px;height:7px;border-radius:2px;background:var(--query);box-shadow:0 0 0 3px rgba(130,184,94,.16);}

  /* ── body ── */
  .mo-body{display:flex; gap:5px; min-height:0; overflow:hidden;}
  .mo-rail{width:256px; flex-shrink:0; display:flex; flex-direction:column; gap:5px; overflow:hidden;}
  .mo-w{overflow:hidden; padding:9px 10px;}
  .mo-w-head{display:flex; align-items:center; gap:9px; margin-bottom:7px;}
  .mo-w-head .gl{width:20px;height:20px;border-radius:5px;display:grid;place-items:center;flex-shrink:0;color:var(--seam);}
  .mo-w-head .ti{flex:1; font-size:11px; font-weight:600; letter-spacing:.05em; text-transform:uppercase; color:var(--subtle);}
  .mo-w-head .bd{font-family:var(--mono); font-size:10px; color:var(--faint); background:var(--bg); border:1px solid var(--line); border-radius:5px; padding:1px 6px; white-space:nowrap;}
  .mo-capture{display:flex; align-items:center; gap:8px; padding:8px 10px; border-radius:6px; background:var(--bg); border:1px solid var(--line); color:var(--faint); font-size:12.5px;}
  .mo-capture .pl{flex:1;}
  .mo-row{display:flex; align-items:center; gap:9px; padding:5px 7px; border-radius:6px; cursor:pointer; color:var(--muted); font-size:12.5px;}
  .mo-row:hover{background:var(--tile-2);}
  .mo-row .lb{flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .mo-row .mt{font-family:var(--mono); font-size:10.5px; color:var(--faint); white-space:nowrap;} .mo-row .mt.urg{color:var(--coral);}
  .mo-row .ic{color:var(--faint);}
  .mo-sub{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:6px 7px 3px;}
  .mo-check{width:14px;height:14px;border-radius:4px;border:1.5px solid var(--faint);flex-shrink:0;} .mo-check.task{border-color:var(--task);}
  .mo-addw{display:flex;align-items:center;justify-content:center;gap:7px;margin-top:auto;padding:10px;border-radius:7px; white-space:nowrap;
    border:1px dashed var(--line-2); color:var(--faint); font-size:12px; cursor:pointer; background:transparent;}
  .mo-addw:hover{color:var(--muted); background:var(--tile);}

  /* ── panes ── */
  .mo-main{flex:1; display:flex; gap:5px; min-width:0;}
  .mo-pane{display:flex; flex-direction:column; min-width:0; overflow:hidden;}
  .mo-pane.focus{flex:1.7;}
  .mo-pane.refs{flex:1;}
  .mo-pane-head{display:flex; align-items:center; gap:11px; padding:13px 16px 12px; border-bottom:1px solid var(--line);}
  .mo-pane-head .ttl{font-size:16px; font-weight:600; letter-spacing:-.01em;}
  .mo-pane-head .sp{flex:1;} .mo-pane-head .meta{font-family:var(--mono); font-size:10.5px; color:var(--faint); white-space:nowrap;}
  .mo-typetag{display:inline-flex; align-items:center; gap:6px; height:21px; padding:0 9px; border-radius:5px; font-family:var(--mono); font-size:10.5px;
    background:rgba(126,155,230,.15); color:var(--project); border:1px solid rgba(126,155,230,.3);}

  .mo-canvas{flex:1; overflow:hidden; padding:12px; display:flex; flex-direction:column; gap:7px; background:var(--bg);}
  /* each block is a thought-tile */
  .mo-block{background:var(--tile); border:1px solid var(--line); border-radius:8px; padding:11px 13px; transition:box-shadow .18s, border-color .18s;}
  .mo-block.sel{border-color:var(--coral); box-shadow:0 0 0 3px var(--coral-soft), 0 6px 20px rgba(0,0,0,.25);}
  .mo-block-main{display:flex; align-items:flex-start; gap:11px;}
  .mo-block-body{flex:1 1 0%; min-width:0;}
  .mo-block-text{font-size:14.5px; color:var(--fg); line-height:1.45; letter-spacing:-.005em;}
  .mo-block .mo-tess{margin-top:5px;}
  .mo-tagchip{display:inline-flex; align-items:center; height:18px; padding:0 7px; margin-left:7px; border-radius:4px;
    font-family:var(--mono); font-size:10.5px; vertical-align:1px; background:var(--coral-soft); color:var(--coral);}
  .mo-tagchip.alt{background:rgba(227,106,130,.15); color:var(--task);}
  .mo-props{display:flex; flex-wrap:wrap; gap:6px; margin-top:9px;}
  .mo-pchip{display:inline-flex; align-items:center; gap:6px; height:24px; padding:0 9px; border-radius:6px; white-space:nowrap; flex-shrink:0;
    background:var(--tile-2); border:1px solid var(--line); font-family:var(--mono); font-size:11px;}
  .mo-pchip .k{color:var(--faint);} .mo-pchip .v{color:var(--muted);} .mo-pchip.doing .v{color:var(--coral); font-weight:600;}
  .mo-pchip.high .v{color:var(--task); font-weight:600;} .mo-pchip .lk{color:var(--project);}
  /* child sub-tiles */
  .mo-kids{display:flex; flex-direction:column; gap:5px; margin-top:9px; margin-left:19px;}
  .mo-kid{display:flex; align-items:flex-start; gap:9px; padding:7px 10px; border-radius:6px; background:var(--bg); border:1px solid var(--line); cursor:pointer;}
  .mo-kid:hover{border-color:var(--line-2);}
  .mo-kid .mo-tess{margin-top:4px; width:6px; height:6px;}
  .mo-kid .kt{font-size:13px; color:var(--muted); flex:1 1 0%; min-width:0;}
  .mo-mention{color:var(--person); background:rgba(169,139,224,.14); padding:0 4px; border-radius:4px;}
  .mo-link{color:var(--project); background:rgba(126,155,230,.14); padding:0 4px; border-radius:4px;}

  .mo-refs-body{flex:1; overflow:hidden; padding:11px; display:flex; flex-direction:column; gap:6px;}
  .mo-refcard{padding:10px 12px; border-radius:7px; background:var(--tile-2); border:1px solid var(--line);}
  .mo-refcard .src{display:flex; align-items:center; gap:8px; font-family:var(--mono); font-size:10.5px; color:var(--muted); margin-bottom:5px;}
  .mo-refcard .snip{font-size:12.5px; color:var(--subtle); line-height:1.5;}
  .mo-refcard .snip em{font-style:normal; color:var(--fg); background:var(--coral-soft); padding:0 3px; border-radius:3px;}
  .mo-refs-props{margin-top:3px; padding-top:9px; border-top:1px solid var(--line);}
  .mo-refs-props .h{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:0 2px 6px;}
  .mo-prow{display:grid; grid-template-columns:18px 74px 1fr; gap:8px; align-items:center; padding:5px 7px; border-radius:6px; font-family:var(--mono); font-size:11px;}
  .mo-prow:hover{background:var(--tile-2);}
  .mo-prow .chord{color:var(--coral); text-align:center; background:var(--bg); border:1px solid var(--line); border-radius:4px; padding:2px 0;}
  .mo-prow .k{color:var(--subtle);} .mo-prow .v{color:var(--muted); white-space:nowrap;} .mo-prow .v.doing{color:var(--coral);} .mo-prow .v.high{color:var(--task);}

  /* ── status tile ── */
  .mo-status{height:28px; display:flex; align-items:center; gap:12px; padding:0 14px; font-family:var(--mono); font-size:11px; color:var(--subtle); white-space:nowrap; overflow:hidden;}
  .mo-status .mode{display:flex; align-items:center; gap:6px; color:var(--coral); font-weight:700; letter-spacing:.10em; font-size:10px;}
  .mo-status .mode .mo-tess{background:var(--coral); width:7px; height:7px;}
  .mo-status .sep{color:var(--faint);}
  .mo-status .keys{margin-left:auto; display:flex; gap:14px;} .mo-status .keys span{color:var(--faint);} .mo-status .keys kbd{color:var(--muted);}
  .mo-status .clk{color:var(--faint); display:flex; align-items:center; gap:5px;}
  `;
  const el = document.createElement('style'); el.id = 'mo-styles'; el.textContent = css; document.head.appendChild(el);
})();

function Mosaic() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  const glyph = { pinned: 'var(--task)', today: 'var(--note)', tasks: 'var(--event)' };
  return (
    <div className="mo-root">
      {/* top bar tile */}
      <div className="mo-tile mo-top">
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <div className="mo-brand"><MosaicMark size={19} tile="#8C95B8" accent="#FF5630" gap={1.6} /><span className="nm">tesela</span></div>
          <div className="mo-tabs">
            {TABS.map((t) => (
              <div key={t.id} className={'mo-tab' + (t.active ? ' active' : '')}>
                <span className={'mo-tess ' + (t.kind === 'project' ? 'project' : t.kind === 'inbox' ? 'event' : 'note')} />
                <span>{t.name}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="mo-cmd"><Icon name="search" size={15} /><span className="ph">Search or run a command…</span><kbd>⌘K</kbd></div>
        <div className="mo-icons">
          <div className="mo-ic"><Icon name="microphone" size={16} /></div>
          <div className="mo-conn"><i /></div>
          <div className="mo-ic"><Icon name="graph" size={16} /></div>
          <div className="mo-ic"><Icon name="settings" size={16} /></div>
        </div>
      </div>

      {/* body */}
      <div className="mo-body">
        <div className="mo-rail">
          <div className="mo-tile mo-w">
            <div className="mo-w-head"><span className="gl" style={{ background: 'var(--note)' }}><Icon name="bolt" size={13} /></span><span className="ti">Quick capture</span></div>
            <div className="mo-capture"><span className="pl">Capture a thought…</span><Icon name="plus" size={14} /></div>
          </div>
          <div className="mo-tile mo-w">
            <div className="mo-w-head"><span className="gl" style={{ background: 'var(--task)' }}><Icon name="pin" size={13} /></span><span className="ti">Pinned</span></div>
            {WIDGETS.pinned.items.map((it, i) => (
              <div key={i} className="mo-row"><Icon name={it.icon} size={14} className="ic" /><span className="lb">{it.label}</span></div>
            ))}
          </div>
          <div className="mo-tile mo-w">
            <div className="mo-w-head"><span className="gl" style={{ background: 'var(--note)' }}><Icon name="sun" size={13} /></span><span className="ti">Today</span><span className="bd">Apr 10</span></div>
            {WIDGETS.today.items.map((it, i) => (
              <div key={i} className="mo-row"><span className={'mo-tess ' + it.kind} /><span className="lb">{it.label}</span><span className={'mt' + (it.urgent ? ' urg' : '')}>{it.meta}</span></div>
            ))}
          </div>
          <div className="mo-tile mo-w">
            <div className="mo-w-head"><span className="gl" style={{ background: 'var(--event)' }}><Icon name="squareCheck" size={13} /></span><span className="ti">Tasks</span><span className="bd">8</span></div>
            {WIDGETS.tasks.groups.map((g, i) => (
              <div key={i}>
                <div className="mo-sub">{g.sub}</div>
                {g.items.map((it, j) => (
                  <div key={j} className="mo-row"><span className="mo-check task" /><span className="lb">{it.label}</span>{it.pri === 'high' && <Icon name="flame" size={13} color="var(--task)" />}</div>
                ))}
              </div>
            ))}
          </div>
          <div className="mo-addw"><Icon name="plus" size={14} />Add widget</div>
        </div>

        <div className="mo-main">
          <div className="mo-tile mo-pane focus">
            <div className="mo-pane-head">
              <span className="ttl">Ship the docs refresh</span>
              <span className="mo-typetag"><span className="mo-tess project" />Project</span>
              <span className="sp" /><span className="meta">2 linked · ⌘\ split</span>
            </div>
            <div className="mo-canvas">
              {OUTLINE.map((b) => (
                <div key={b.id} className={'mo-block' + (b.selected ? ' sel' : '')}>
                  <div className="mo-block-main">
                    <span className={'mo-tess ' + (b.tag === 'Task' ? 'task' : b.link ? 'project' : 'note')} />
                    <div className="mo-block-body">
                      <div className="mo-block-text">
                        {b.link ? (<>Weekly sync with <span className="mo-link">[[Domain]]</span> leads</>) : b.text}
                        {b.tag && <span className="mo-tagchip">#{b.tag}</span>}
                        {b.tag2 && <span className="mo-tagchip alt">#{b.tag2}</span>}
                      </div>
                      {b.props && b.props.length > 0 && (
                        <div className="mo-props">
                          {b.props.map((p, i) => (
                            <span key={i} className={'mo-pchip ' + tone(p.tone)}>
                              <span className="k">{p.k}</span>
                              {p.link ? <span className="lk">[[{p.v}]]</span> : <span className="v">{p.v}</span>}
                            </span>
                          ))}
                        </div>
                      )}
                      {b.children && b.children.length > 0 && (
                        <div className="mo-kids">
                          {b.children.map((k) => (
                            <div key={k.id} className="mo-kid">
                              <span className="mo-tess" />
                              <span className="kt">{k.mention ? (<>Review with <span className="mo-mention">@Mara</span> on the Domain team</>) : k.text}</span>
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

          <div className="mo-tile mo-pane refs">
            <div className="mo-pane-head"><span className="ttl" style={{ fontSize: 14 }}>Linked references</span><span className="sp" /><span className="meta">3</span></div>
            <div className="mo-refs-body">
              {BACKLINKS.map((r, i) => (
                <div key={i} className="mo-refcard">
                  <div className="src"><span className={'mo-tess ' + (r.kind === 'project' ? 'project' : r.kind === 'daily' ? 'event' : 'note')} />{r.src}</div>
                  <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
                </div>
              ))}
              <div className="mo-refs-props">
                <div className="h">Properties · b1</div>
                {PROPS.map((p, i) => (
                  <div key={i} className="mo-prow"><span className="chord">{p.chord}</span><span className="k">{p.k}</span><span className={'v ' + (p.tone || '')}>{p.v}</span></div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* status tile */}
      <div className="mo-tile mo-status">
        <span className="mode"><span className="mo-tess" />{STATUS.mode}</span>
        <span className="sep">·</span><span>{STATUS.path}</span>
        <span className="keys">
          {STATUS.keys.map((k, i) => (<span key={i}><kbd>{k.k}</kbd> {k.label}</span>))}
          <span className="clk"><Icon name="clock" size={12} color="var(--faint)" />14:08</span>
        </span>
      </div>
    </div>
  );
}

Object.assign(window, { Mosaic });

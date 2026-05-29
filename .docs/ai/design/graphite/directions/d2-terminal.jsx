/* Direction 2 — TERMINAL PRO
   The terminal/tmux/Neovim soul, made premium. Mono-forward chrome, panes
   framed and numbered like tmux windows, inline key::value props (Logseq
   grammar), a true powerline status bar. Keyboard surfacing LOUD — hotkeys,
   leader, mode all visible. The power-user badge of honor. */

(function () {
  if (document.getElementById('tp-styles')) return;
  const css = `
  .tp-root{
    --bg:#0A0B0E; --surface:#101216; --panel:#14171C; --panel-2:#181C22;
    --line:#21262E; --line-2:#2A313B;
    --fg:#D7DBE0; --muted:#98A0AB; --subtle:#687078; --faint:#464D57;
    --coral:#FF5C49; --amber:#E8B86B;
    --task:#E5677F; --event:#6DBACC; --note:#E8B86B; --project:#6E8FE6; --person:#A98BE0; --query:#88B85E;
    --mono:'JetBrains Mono',ui-monospace,monospace; --sans:'Geist',system-ui,sans-serif;
    position:absolute; inset:0; background:var(--bg); color:var(--fg);
    font-family:var(--mono); font-size:12px; line-height:1.5;
    display:grid; grid-template-rows:38px 1fr 26px; overflow:hidden; -webkit-font-smoothing:antialiased;
    background-image:radial-gradient(rgba(255,255,255,.022) 1px, transparent 1px); background-size:14px 14px;
  }
  .tp-root *{box-sizing:border-box;}

  /* ── top bar ── */
  .tp-top{display:flex; align-items:center; gap:14px; padding:0 12px; border-bottom:1px solid var(--line); background:var(--surface);}
  .tp-brand{display:flex; align-items:center; gap:8px;}
  .tp-brand .nm{font-size:12.5px; font-weight:600; color:var(--fg); letter-spacing:.02em;}
  .tp-session{color:var(--subtle); font-size:11px;}
  .tp-session b{color:var(--query); font-weight:500;}
  .tp-tabs{display:flex; align-items:center; gap:2px; margin-left:6px;}
  .tp-tab{display:flex; align-items:center; gap:6px; height:24px; padding:0 9px; border-radius:3px; white-space:nowrap;
    color:var(--subtle); cursor:pointer; font-size:11.5px; border:1px solid transparent;}
  .tp-tab .n{color:var(--faint);}
  .tp-tab:hover{background:var(--panel);}
  .tp-tab.active{color:var(--fg); background:var(--panel); border-color:var(--line-2);}
  .tp-tab.active .n{color:var(--coral);}
  .tp-spacer{flex:1;}
  .tp-cmd{display:flex; align-items:center; gap:8px; height:26px; width:300px; padding:0 10px; border-radius:3px;
    background:var(--bg); border:1px solid var(--line-2); color:var(--subtle); font-size:11px;}
  .tp-cmd .pr{color:var(--coral);} .tp-cmd .ph{flex:1;} .tp-cmd kbd{color:var(--faint);}
  .tp-icons{display:flex; align-items:center; gap:1px;}
  .tp-ic{width:27px; height:27px; display:grid; place-items:center; border-radius:3px; color:var(--subtle); cursor:pointer;}
  .tp-ic:hover{color:var(--fg); background:var(--panel);}
  .tp-conn{width:24px; display:grid; place-items:center;} .tp-conn i{width:6px;height:6px;border-radius:50%;background:var(--query);box-shadow:0 0 6px var(--query);}

  /* ── body ── */
  .tp-body{display:flex; min-height:0; overflow:hidden; padding:6px; gap:6px;}
  .tp-rail{width:248px; flex-shrink:0; display:flex; flex-direction:column; gap:6px; overflow:hidden;}
  .tp-w{background:var(--surface); border:1px solid var(--line); border-radius:4px; overflow:hidden;}
  .tp-w-head{display:flex; align-items:center; gap:8px; padding:6px 9px; background:var(--panel); border-bottom:1px solid var(--line);}
  .tp-w-head .hk{color:var(--amber); font-size:10px; border:1px solid var(--line-2); border-radius:3px; padding:0 4px; background:var(--bg);}
  .tp-w-head .ti{flex:1; font-size:10.5px; font-weight:600; letter-spacing:.08em; text-transform:uppercase; color:var(--muted);}
  .tp-w-head .bd{color:var(--faint); font-size:10px; white-space:nowrap;}
  .tp-w-body{padding:5px 6px;}
  .tp-capture{display:flex; align-items:center; gap:7px; padding:7px 9px; border-radius:3px; background:var(--bg);
    border:1px dashed var(--line-2); color:var(--faint); font-size:11px;}
  .tp-capture .pr{color:var(--coral);} .tp-capture .pl{flex:1;}
  .tp-row{display:flex; align-items:center; gap:8px; padding:4px 7px; border-radius:3px; cursor:pointer; color:var(--muted); font-size:11.5px;}
  .tp-row:hover{background:var(--panel);}
  .tp-row .ix{color:var(--faint); width:12px;}
  .tp-row .lb{flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .tp-row .mt{color:var(--faint); white-space:nowrap;} .tp-row .mt.urg{color:var(--coral);}
  .tp-dot{width:6px;height:6px;border-radius:1px;flex-shrink:0;}
  .tp-dot.event{background:var(--event);} .tp-dot.task{background:var(--task);} .tp-dot.note{background:var(--note);} .tp-dot.project{background:var(--project);}
  .tp-sub{color:var(--faint); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; padding:6px 7px 2px;}
  .tp-check{width:13px;height:13px;border-radius:2px;border:1px solid var(--faint);flex-shrink:0;}
  .tp-check.task{border-color:var(--task);}
  .tp-addw{display:flex;align-items:center;justify-content:center;gap:6px;margin-top:auto;padding:8px;border-radius:4px; white-space:nowrap;
    border:1px dashed var(--line-2); color:var(--faint); font-size:11px; cursor:pointer;}
  .tp-addw:hover{color:var(--muted);}

  /* ── panes (tmux windows) ── */
  .tp-main{flex:1; display:flex; gap:6px; min-width:0;}
  .tp-pane{display:flex; flex-direction:column; min-width:0; background:var(--surface); border:1px solid var(--line); border-radius:4px; overflow:hidden;}
  .tp-pane.focus{flex:1.7; border-color:color-mix(in srgb,var(--coral) 55%,var(--line));}
  .tp-pane.refs{flex:1;}
  .tp-pane-head{display:flex; align-items:center; gap:9px; padding:6px 11px; background:var(--panel); border-bottom:1px solid var(--line);}
  .tp-pane.focus .tp-pane-head{background:linear-gradient(var(--panel-2),var(--panel));}
  .tp-pane-head .pn{color:var(--coral); font-weight:700;}
  .tp-pane.refs .tp-pane-head .pn{color:var(--subtle);}
  .tp-pane-head .pt{color:var(--fg); font-size:11.5px; white-space:nowrap;}
  .tp-pane-head .pt .ext{color:var(--faint);}
  .tp-tag-sq{padding:0 6px; height:17px; display:inline-flex; align-items:center; border-radius:2px; font-size:10px;
    background:rgba(110,143,230,.16); color:var(--project); border:1px solid rgba(110,143,230,.3);}
  .tp-pane-head .sp{flex:1;} .tp-pane-head .hint{color:var(--faint); font-size:10.5px; white-space:nowrap;}
  .tp-pane-head .hint kbd{color:var(--amber);}

  .tp-outline{flex:1; overflow:hidden; padding:12px 14px; font-family:var(--sans);}
  .tp-blk{padding:3px 0;}
  .tp-blk-line{display:flex; align-items:flex-start; gap:9px; padding:4px 8px; border-radius:3px; border-left:2px solid transparent;}
  .tp-blk.sel > .tp-blk-line{background:var(--panel); border-left-color:var(--coral);}
  .tp-bull{font-family:var(--mono); color:var(--faint); font-size:12px; margin-top:2px;}
  .tp-blk.task > .tp-blk-line > .tp-bull{color:var(--task);}
  .tp-blk-body{flex:1 1 0%; min-width:0;}
  .tp-blk-text{font-size:14px; color:var(--fg); line-height:1.45;}
  .tp-hash{font-family:var(--mono); font-size:12px; margin-left:7px; color:var(--coral);}
  .tp-hash.alt{color:var(--task);}
  .tp-props{font-family:var(--mono); font-size:11.5px; margin-top:4px; padding-left:1px; display:flex; flex-wrap:wrap; gap:14px;}
  .tp-prop{white-space:nowrap;} .tp-prop .k{color:var(--faint);} .tp-prop .v{color:var(--muted);}
  .tp-prop.doing .v{color:var(--coral);} .tp-prop.high .v{color:var(--task);} .tp-prop .lk{color:var(--project);}
  .tp-kids{margin-left:9px; padding-left:14px; border-left:1px solid var(--line-2); margin-top:2px;}
  .tp-kid{display:flex; align-items:flex-start; gap:9px; padding:3px 8px; border-radius:3px; cursor:pointer;}
  .tp-kid:hover{background:var(--panel);}
  .tp-kid .kb{font-family:var(--mono); color:var(--faint); font-size:11px; margin-top:1px;}
  .tp-kid .kt{font-size:13px; color:var(--muted); flex:1 1 0%; min-width:0;}
  .tp-mention{color:var(--person);} .tp-link{color:var(--project);}

  .tp-refs-body{flex:1; overflow:hidden; padding:10px 12px; font-family:var(--sans); display:flex; flex-direction:column; gap:2px;}
  .tp-refcard{padding:8px 10px; border-radius:3px; border-left:2px solid var(--line-2);}
  .tp-refcard:hover{background:var(--panel);}
  .tp-refcard .src{font-family:var(--mono); font-size:10.5px; color:var(--muted); display:flex; align-items:center; gap:7px; margin-bottom:3px;}
  .tp-refcard .snip{font-size:12px; color:var(--subtle); line-height:1.45;}
  .tp-refcard .snip em{font-style:normal; color:var(--amber);}
  .tp-refs-props{margin-top:8px; border-top:1px solid var(--line); padding-top:8px;}
  .tp-refs-props .h{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:0 2px 6px;}
  .tp-prow{display:grid; grid-template-columns:16px 70px 1fr; gap:8px; align-items:center; padding:3px 7px; border-radius:3px; font-family:var(--mono); font-size:11px;}
  .tp-prow:hover{background:var(--panel);}
  .tp-prow .chord{color:var(--amber); text-align:center; border:1px solid var(--line-2); border-radius:2px;}
  .tp-prow .k{color:var(--subtle);} .tp-prow .v{color:var(--muted); white-space:nowrap;}
  .tp-prow .v.doing{color:var(--coral);} .tp-prow .v.high{color:var(--task);}

  /* ── powerline status ── */
  .tp-status{display:flex; align-items:stretch; font-family:var(--mono); font-size:10.5px; background:var(--surface); border-top:1px solid var(--line); white-space:nowrap; overflow:hidden;}
  .tp-seg{display:flex; align-items:center; gap:6px; padding:0 12px;}
  .tp-seg.mode{background:var(--coral); color:#0A0B0E; font-weight:700; letter-spacing:.12em;}
  .tp-seg.path{background:var(--panel-2); color:var(--muted);}
  .tp-seg.blk{color:var(--subtle);}
  .tp-seg.sp{flex:1;}
  .tp-seg.keys{color:var(--faint); gap:14px;} .tp-seg.keys b{color:var(--muted); font-weight:400;} .tp-seg.keys kbd{color:var(--amber);}
  .tp-seg.clk{background:var(--panel-2); color:var(--subtle);}
  .tp-arr{width:0;height:0;align-self:center;border-top:13px solid transparent;border-bottom:13px solid transparent;}
  `;
  const el = document.createElement('style'); el.id = 'tp-styles'; el.textContent = css; document.head.appendChild(el);
})();

function TerminalPro() {
  const tone = (t) => (t === 'doing' ? 'doing' : t === 'high' ? 'high' : '');
  return (
    <div className="tp-root">
      {/* top bar */}
      <div className="tp-top">
        <div className="tp-brand">
          <MosaicMark size={16} tile="#7E8AA8" accent="#FF5C49" gap={1.6} />
          <span className="nm">tesela</span>
        </div>
        <span className="tp-session">session <b>mosaic:main</b></span>
        <div className="tp-tabs">
          {TABS.map((t, i) => (
            <div key={t.id} className={'tp-tab' + (t.active ? ' active' : '')}>
              <span className="n">{i + 1}:</span><span>{t.name}</span>
            </div>
          ))}
        </div>
        <span className="tp-spacer" />
        <div className="tp-cmd"><span className="pr">:</span><span className="ph">command — verbs, search, dashboard…</span><kbd>⌘K</kbd></div>
        <div className="tp-icons">
          <div className="tp-ic"><Icon name="microphone" size={15} /></div>
          <div className="tp-conn"><i /></div>
          <div className="tp-ic"><Icon name="graph" size={15} /></div>
          <div className="tp-ic"><Icon name="settings" size={15} /></div>
        </div>
      </div>

      {/* body */}
      <div className="tp-body">
        {/* rail */}
        <div className="tp-rail">
          <div className="tp-w">
            <div className="tp-w-head"><span className="hk">c</span><span className="ti">Quick capture</span></div>
            <div className="tp-w-body"><div className="tp-capture"><span className="pr">›</span><span className="pl">capture a thought…</span></div></div>
          </div>
          <div className="tp-w">
            <div className="tp-w-head"><span className="hk">p</span><span className="ti">Pinned</span></div>
            <div className="tp-w-body">
              {WIDGETS.pinned.items.map((it, i) => (
                <div key={i} className="tp-row"><Icon name={it.icon} size={13} color="var(--faint)" /><span className="lb">{it.label}</span></div>
              ))}
            </div>
          </div>
          <div className="tp-w">
            <div className="tp-w-head"><span className="hk">t</span><span className="ti">Today</span><span className="bd">Apr 10</span></div>
            <div className="tp-w-body">
              {WIDGETS.today.items.map((it, i) => (
                <div key={i} className="tp-row"><span className={'tp-dot ' + it.kind} /><span className="lb">{it.label}</span><span className={'mt' + (it.urgent ? ' urg' : '')}>{it.meta}</span></div>
              ))}
            </div>
          </div>
          <div className="tp-w">
            <div className="tp-w-head"><span className="hk">g</span><span className="ti">Tasks</span><span className="bd">8 open</span></div>
            <div className="tp-w-body">
              {WIDGETS.tasks.groups.map((g, i) => (
                <div key={i}>
                  <div className="tp-sub">{g.sub}</div>
                  {g.items.map((it, j) => (
                    <div key={j} className="tp-row"><span className="tp-check task" /><span className="lb">{it.label}</span>{it.pri === 'high' && <Icon name="flame" size={12} color="var(--task)" />}</div>
                  ))}
                </div>
              ))}
            </div>
          </div>
          <div className="tp-addw"><Icon name="plus" size={13} />add widget</div>
        </div>

        {/* panes */}
        <div className="tp-main">
          <div className="tp-pane focus">
            <div className="tp-pane-head">
              <span className="pn">1</span>
              <span className="pt">ship-docs-refresh<span className="ext">.md</span></span>
              <span className="tp-tag-sq">Project</span>
              <span className="sp" />
              <span className="hint"><kbd>⌘\\</kbd> vsplit · <kbd>⌘i</kbd> peek</span>
            </div>
            <div className="tp-outline">
              {OUTLINE.map((b) => (
                <div key={b.id} className={'tp-blk' + (b.tag === 'Task' ? ' task' : '') + (b.selected ? ' sel' : '')}>
                  <div className="tp-blk-line">
                    <span className="tp-bull">{b.tag === 'Task' ? '▸' : '•'}</span>
                    <div className="tp-blk-body">
                      <div className="tp-blk-text">
                        {b.link ? (<>Weekly sync with <span className="tp-link">[[Domain]]</span> leads</>) : b.text}
                        {b.tag && <span className="tp-hash">#{b.tag}</span>}
                        {b.tag2 && <span className="tp-hash alt">#{b.tag2}</span>}
                      </div>
                      {b.props && b.props.length > 0 && (
                        <div className="tp-props">
                          {b.props.map((p, i) => (
                            <span key={i} className={'tp-prop ' + tone(p.tone)}>
                              <span className="k">{p.k}:: </span>
                              {p.link ? <span className="lk">[[{p.v}]]</span> : <span className="v">{p.v}</span>}
                            </span>
                          ))}
                        </div>
                      )}
                      {b.children && b.children.length > 0 && (
                        <div className="tp-kids">
                          {b.children.map((k) => (
                            <div key={k.id} className="tp-kid">
                              <span className="kb">•</span>
                              <span className="kt">{k.mention ? (<>Review with <span className="tp-mention">@Mara</span> on the Domain team</>) : k.text}</span>
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

          <div className="tp-pane refs">
            <div className="tp-pane-head"><span className="pn">2</span><span className="pt">linked refs</span><span className="sp" /><span className="hint">3 found</span></div>
            <div className="tp-refs-body">
              {BACKLINKS.map((r, i) => (
                <div key={i} className="tp-refcard">
                  <div className="src"><span className={'tp-dot ' + (r.kind === 'project' ? 'project' : r.kind === 'daily' ? 'event' : 'note')} />{r.src}</div>
                  <div className="snip" dangerouslySetInnerHTML={{ __html: r.snippet.replace('docs refresh', '<em>docs refresh</em>') }} />
                </div>
              ))}
              <div className="tp-refs-props">
                <div className="h">properties · b1</div>
                {PROPS.map((p, i) => (
                  <div key={i} className="tp-prow"><span className="chord">{p.chord}</span><span className="k">{p.k}</span><span className={'v ' + (p.tone || '')}>{p.v}</span></div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* powerline status */}
      <div className="tp-status">
        <div className="tp-seg mode">NORMAL</div>
        <div className="tp-arr" style={{ borderLeft: '13px solid var(--coral)' }} />
        <div className="tp-seg path">{STATUS.path}</div>
        <div className="tp-arr" style={{ borderLeft: '13px solid var(--panel-2)' }} />
        <div className="tp-seg blk">block b1 · 2 children</div>
        <div className="tp-seg sp" />
        <div className="tp-seg keys">
          <span><kbd>Space</kbd> <b>leader</b></span>
          <span><kbd>⌘K</kbd> <b>command</b></span>
          <span><kbd>⌘\\</kbd> <b>split</b></span>
        </div>
        <div className="tp-arr" style={{ borderRight: '13px solid var(--panel-2)' }} />
        <div className="tp-seg clk"><Icon name="clock" size={11} color="var(--subtle)" />14:08</div>
      </div>
    </div>
  );
}

Object.assign(window, { TerminalPro });

// === v8 — Mosaic Curio. Light, tactile, paper-tinted columns. ===
const { useState } = React;
const D8 = window.COL_DATA;

function v8Rich(s) {
  if (!s) return null;
  const out = []; const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0, m, i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) out.push(<span key={`l${i++}`} className="v8link">{m[3] || m[2]}</span>);
    else out.push(<span key={`t${i++}`} className="v8tag">#{m[5]}</span>);
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

function V8Rail({ active, onPick }) {
  const groups = [
    { label: "Pinned", items: ["today","tasks","inbox","calendar"] },
    { label: "Types",  items: ["projects","people"] },
    { label: "Saved",  items: ["queries","recent","pinned"] },
  ];
  const byId = Object.fromEntries(D8.WIDGETS.map(w=>[w.id,w]));
  return (
    <>
      <div className="v8-head-pane">
        <div className="kicker">Cabinet</div>
        <div className="t">Widgets</div>
        <div className="s">Pinned views & saved drawers</div>
      </div>
      <div className="v8-body v8-rail">
        {groups.map(g => (
          <React.Fragment key={g.label}>
            <div className="group">{g.label}</div>
            {g.items.map(id => {
              const w = byId[id]; if (!w) return null;
              return (
                <div key={id} className={`w ${active===id?"active":""}`} data-icon={w.icon} onClick={()=>onPick(id)}>
                  <span className="gl">{w.label[0]}</span>
                  <span className="name">{w.label}</span>
                  <span className="badge">{w.badge || w.count || ""}</span>
                </div>
              );
            })}
          </React.Fragment>
        ))}
        <div className="add">+ pin a query</div>
      </div>
    </>
  );
}

function V8Listing({ data, selected, onSelect }) {
  if (!data) return null;
  if (data.kind === "blocks") {
    return (
      <div className="v8-bl" style={{padding:"14px 22px 14px 36px"}}>
        {data.blocks.map(b => (
          <div key={b.id} className={`blk indent-${b.indent} k-${b.kind}`} onClick={()=>onSelect(b.id)} style={{cursor:"pointer"}}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v8Rich(b.text)}</span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div>
      {data.groups.map(g => (
        <div key={g.label}>
          <div className="v8-grp"><span>{g.label}</span><span className="rule"></span><span className="n">{g.items.length}</span></div>
          {g.items.map(it => {
            const urgent = ["Apr 25","Apr 26"].includes(it.deadline);
            return (
              <div key={it.id} className={`v8-tile ${it.id===selected?"selected":""}`} onClick={()=>onSelect(it.id)}>
                <div className="text">{it.text}</div>
                <div className="meta">
                  {it.priority && it.priority!=="—" && <span className={`pri p-${it.priority}`}>{it.priority}</span>}
                  {it.deadline && it.deadline!=="—" && <span className={`due ${urgent?"urgent":""}`}>{it.deadline}</span>}
                  {it.src && <span className="src">{it.src}</span>}
                </div>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}

function V8Focus({ focus }) {
  if (!focus) return (
    <div style={{padding:"60px 36px", color:"#b9ad94", fontFamily:"Newsreader", fontStyle:"italic", fontSize:18}}>
      Select an entry to bring its specimen card forward.
    </div>
  );
  const propsToShow = focus.props.slice(0, 6);
  return (
    <>
      <div className="v8-specimen">
        <div className="label">
          <span className="typetag"><span className="sw"></span>{focus.meta.type} <span style={{color:"#b9ad94", margin:"0 6px"}}>·</span> <span className="status">{focus.meta.status}</span></span>
          <span className="cat-no">cat. № 04-29 / t1</span>
        </div>
        <div className="title">{focus.title}</div>
        <div className="props">
          {propsToShow.map(p => (
            <div key={p.k} className={`pchip ${p.k==="status"?"s-"+p.v:""} ${p.k==="priority"?"p-"+p.v:""}`}>
              <div className="k">{p.k}</div>
              <div className="v">{p.v}</div>
            </div>
          ))}
        </div>
        <div className="v8-bl">
          {focus.tree.map((b,i) => (
            <div key={i} className={`blk indent-${b.indent} k-${b.kind}`}>
              <span className="bull">{b.indent===0?"●":"·"}</span>
              <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v8Rich(b.text)}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="v8-strip">
        <div className="card">
          <div className="h">Referenced in <span className="n">· {focus.backlinks.length}</span></div>
          {focus.backlinks.map((b,i) => (
            <div key={i} className="item">
              <div className="src">{b.src}<span className="lbl">{b.label}</span></div>
              <div className="snip" dangerouslySetInnerHTML={{__html:"“"+b.snippet.replace(/\*([^*]+)\*/g,"<em>$1</em>")+"”"}}/>
            </div>
          ))}
        </div>
        <div className="card out">
          <div className="h">Outline <span className="n">· {focus.outline.length}</span></div>
          {focus.outline.map((o,i) => (
            <div key={i} className={`row indent-${o.indent}`}>
              <span className="b">{o.indent===0?"●":"·"}</span>
              <span>{o.text}</span>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}

function V8() {
  const [widget, setWidget] = useState("tasks");
  const [selected, setSelected] = useState("t1");
  const listing = D8.LISTINGS[widget];
  const focus = D8.FOCUS[selected] || null;
  const widgetLabel = D8.WIDGETS.find(w=>w.id===widget)?.label;
  return (
    <div className="v8" data-screen-label="01 Mosaic Curio">
      <header className="v8-head">
        <div className="mark"><span className="seal"></span>Tesela</div>
        <div className="crumb">
          <span className="seg">{widgetLabel}</span>
          <span className="sep">›</span>
          <span className="seg">{listing?.groups?.[0]?.label || "All"}</span>
          <span className="sep">›</span>
          <span className="seg curr">{focus?.title || "—"}</span>
        </div>
        <div className="right">
          <span className="pill"><span className="k">collection</span>type:Task</span>
          <span className="pill"><span className="k">today</span>Fri 29 Apr</span>
        </div>
      </header>

      <main className="v8-stage">
        <section className="v8-col tint-rail">
          <div className="spine"><span className="lbl">Cabinet</span></div>
          <V8Rail active={widget} onPick={(id)=>{setWidget(id); setSelected(null);}}/>
        </section>

        <section className="v8-col tint-list">
          <div className="spine"><span className="lbl">Drawer · {widgetLabel}</span></div>
          <div className="v8-head-pane">
            <div className="kicker">{listing?.kind==="blocks"?"Daily note":"Drawer"}</div>
            <div className="t">{listing?.title}</div>
            <div className="s">{listing?.subtitle}</div>
            {listing?.query && <span className="q">{listing.query}</span>}
          </div>
          <div className="v8-body">
            <V8Listing data={listing} selected={selected} onSelect={setSelected}/>
          </div>
        </section>

        <section className="v8-col tint-focus">
          <div className="spine"><span className="lbl">Specimen</span></div>
          <div className="v8-body">
            <V8Focus focus={focus}/>
          </div>
        </section>
      </main>

      <footer className="v8-foot">
        <div className="l">
          <span>Tesela · Mosaic Curio</span>
          <span>type:Task · 38</span>
          <span>{focus ? "1 specimen" : "—"}</span>
        </div>
        <div className="r">
          <span><kbd>↑↓</kbd> entry</span>
          <span><kbd>←→</kbd> drawer</span>
          <span><kbd>↵</kbd> open</span>
          <span><kbd>⌘K</kbd> jump</span>
        </div>
      </footer>
    </div>
  );
}

window.V8 = V8;

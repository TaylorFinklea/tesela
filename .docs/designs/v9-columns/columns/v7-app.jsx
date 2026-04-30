// === v7 — Atelier. Editorial light, three columns. ===
const { useState } = React;
const D7 = window.COL_DATA;

function v7Rich(s) {
  if (!s) return null;
  const out = []; const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0, m, i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) out.push(<span key={`l${i++}`} className="v7link">{m[3] || m[2]}</span>);
    else out.push(<span key={`t${i++}`} className="v7tag">#{m[5]}</span>);
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

function V7Rail({ active, onPick }) {
  const groups = [
    { label: "Today",  items: ["today","tasks","inbox","calendar"] },
    { label: "Types",  items: ["projects","people"] },
    { label: "Saved",  items: ["queries","recent","pinned"] },
  ];
  const byId = Object.fromEntries(D7.WIDGETS.map(w=>[w.id,w]));
  return (
    <nav className="v7-rail">
      {groups.map(g => (
        <React.Fragment key={g.label}>
          <div className="group">{g.label}</div>
          {g.items.map(id => {
            const w = byId[id]; if (!w) return null;
            return (
              <div key={id} className={`w ${active===id?"active":""}`} onClick={()=>onPick(id)}>
                <span className="name">{w.label}</span>
                <span className="badge">{w.badge || w.count || ""}</span>
              </div>
            );
          })}
        </React.Fragment>
      ))}
      <div className="add">+ new widget</div>
    </nav>
  );
}

function V7Listing({ data, selected, onSelect }) {
  if (!data) return null;
  if (data.kind === "blocks") {
    return (
      <div className="v7-bl">
        {data.blocks.map(b => (
          <div key={b.id} className={`blk indent-${b.indent} k-${b.kind}`} onClick={()=>onSelect(b.id)} style={{cursor:"pointer"}}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v7Rich(b.text)}</span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div>
      {data.groups.map(g => (
        <div key={g.label}>
          <div className="v7-grp"><span>{g.label}</span><span className="rule"></span><span className="n">{g.items.length}</span></div>
          {g.items.map(it => {
            const urgent = ["Apr 25","Apr 26"].includes(it.deadline);
            return (
              <div key={it.id} className={`v7-row ${it.id===selected?"selected":""} p-${it.priority}`} onClick={()=>onSelect(it.id)}>
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

function V7Focus({ focus }) {
  if (!focus) return (
    <div style={{padding:"60px 32px",color:"#b4ab95",fontFamily:"Newsreader",fontStyle:"italic",fontSize:18}}>
      Select an entry to bring it forward.
    </div>
  );
  return (
    <>
      <div className="v7-pane-head">
        <div className="kicker"><span className="typetag"><span className="sw"></span>{focus.meta.type} <span style={{color:"#b4ab95",margin:"0 8px"}}>·</span> <span className="status">{focus.meta.status}</span></span></div>
        <div className="t">{focus.title}</div>
        <div className="props">
          {focus.props.slice(0,5).map(p => (
            <span key={p.k} className={`pchip ${p.k==="status"?"s-"+p.v:""} ${p.k==="priority"?"p-"+p.v:""}`}>
              <span className="k">{p.k}</span><span className="v">{p.v}</span>
            </span>
          ))}
        </div>
      </div>
      <div className="v7-bl">
        {focus.tree.map((b,i) => (
          <div key={i} className={`blk indent-${b.indent} k-${b.kind}`}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v7Rich(b.text)}</span>
          </div>
        ))}
      </div>
      <div className="v7-back">
        <div className="h">Referenced in</div>
        {focus.backlinks.map((b,i) => (
          <div key={i} className="item">
            <div className="src">{b.src}<span className="lbl">{b.label}</span></div>
            <div className="snip" dangerouslySetInnerHTML={{__html:"“"+b.snippet.replace(/\*([^*]+)\*/g,"<em>$1</em>").replace(/^…/,"…").replace(/…$/,"…")+"”"}}/>
          </div>
        ))}
      </div>
    </>
  );
}

function V7() {
  const [widget, setWidget] = useState("tasks");
  const [selected, setSelected] = useState("t1");
  const listing = D7.LISTINGS[widget];
  const focus = D7.FOCUS[selected] || null;
  const widgetLabel = D7.WIDGETS.find(w=>w.id===widget)?.label;
  return (
    <div className="v7" data-screen-label="01 Atelier">
      <header className="v7-head">
        <div className="mark"><span className="dot"></span>Tesela</div>
        <div className="crumb">
          <span className="seg">{widgetLabel}</span>
          <span className="sep">/</span>
          <span className="seg">{listing?.groups?.[0]?.label || "All"}</span>
          <span className="sep">/</span>
          <span className="seg curr">{focus?.title || "—"}</span>
        </div>
        <div className="right"><span className="date">Friday, 29 April 2026</span></div>
      </header>

      <V7Rail active={widget} onPick={(id)=>{setWidget(id); setSelected(null);}}/>

      <section className="v7-middle">
        <div className="v7-pane-head">
          <div className="kicker">{listing?.kind==="blocks"?"Daily note":"Listing"}</div>
          <div className="t">{listing?.title}</div>
          <div className="s">{listing?.subtitle}</div>
          {listing?.query && <span className="q">{listing.query}</span>}
        </div>
        <div className="v7-pane-body">
          <V7Listing data={listing} selected={selected} onSelect={setSelected}/>
        </div>
      </section>

      <section className="v7-focus">
        <V7Focus focus={focus}/>
      </section>

      <footer className="v7-foot">
        <span>Tesela — Atelier</span>
        <span>type:Task · 38 entries</span>
        <div className="right">
          <span><kbd>↑↓</kbd> entry</span>
          <span><kbd>←→</kbd> column</span>
          <span><kbd>↵</kbd> open</span>
          <span><kbd>⌘K</kbd> jump</span>
        </div>
      </footer>
    </div>
  );
}

window.V7 = V7;

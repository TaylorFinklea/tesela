// === v6 — xplr Monolith. Dark, dense, column-view. ===
const { useState } = React;
const D = window.COL_DATA;

function v6Rich(s) {
  if (!s) return null;
  const out = []; const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0, m, i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) out.push(<span key={`l${i++}`} className="v6link">{m[3] || m[2]}</span>);
    else out.push(<span key={`t${i++}`} className="v6tag">#{m[5]}</span>);
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

function V6Rail({ active, onPick }) {
  const groups = [
    { label: "Pinned", items: ["today","tasks","inbox","calendar"] },
    { label: "Types",  items: ["projects","people"] },
    { label: "Saved",  items: ["queries","recent","pinned"] },
  ];
  const byId = Object.fromEntries(D.WIDGETS.map(w=>[w.id,w]));
  return (
    <div className="v6-rail">
      {groups.map(g => (
        <React.Fragment key={g.label}>
          <div className="group">{g.label}</div>
          {g.items.map(id => {
            const w = byId[id]; if (!w) return null;
            return (
              <div key={id} className={`w ${active===id?"active":""}`} data-icon={w.icon} onClick={()=>onPick(id)}>
                <span className="gl">{w.label[0]}</span>
                <span>{w.label}</span>
                <span className="badge">{w.badge || w.count || ""}</span>
              </div>
            );
          })}
        </React.Fragment>
      ))}
      <div className="add">+ add widget</div>
    </div>
  );
}

function V6Listing({ data, selected, onSelect }) {
  if (!data) return null;
  if (data.kind === "blocks") {
    return (
      <div className="v6-bl">
        {data.blocks.map(b => (
          <div key={b.id} className={`blk indent-${b.indent} k-${b.kind} ${b.id===selected?"selected":""}`} onClick={()=>onSelect(b.id)}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v6Rich(b.text)}</span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div>
      {data.groups.map(g => (
        <div key={g.label}>
          <div className="v6-grp">{g.label} <span style={{opacity:0.55}}>· {g.items.length}</span></div>
          {g.items.map(it => (
            <div key={it.id} className={`v6-row ${it.id===selected?"selected":""}`} onClick={()=>onSelect(it.id)}>
              <span className="marker">{it.id===selected?"▸":" "}</span>
              <span className="text">{it.text}</span>
              <span className={`pri p-${it.priority}`}>{it.priority!=="—"?it.priority:""}</span>
              <span className={`due ${["Apr 25","Apr 26"].includes(it.deadline)?"urgent":""}`}>{it.deadline}</span>
              <span className="src">↳ {it.src}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function V6Focus({ focus }) {
  if (!focus) return <div style={{padding:"30px 14px",color:"#5a5246",fontFamily:"JetBrains Mono"}}>— select a row to focus —</div>;
  return (
    <>
      <div className="v6-pane-head">
        <span className="typetag"><span className="sw"></span>{focus.meta.type} · {focus.meta.status}</span>
        <span className="t">{focus.title}</span>
      </div>
      <div className="props">
        {focus.props.map(p => (
          <span key={p.k} className={`pchip ${p.k==="status"?"s-"+p.v:""} ${p.k==="priority"?"p-"+p.v:""}`}>
            <span className="k">{p.k}</span><span className="v">{p.v}</span>
          </span>
        ))}
      </div>
      <div className="v6-bl" style={{paddingTop:14}}>
        {focus.tree.map((b,i) => (
          <div key={i} className={`blk indent-${b.indent} k-${b.kind}`}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v6Rich(b.text)}</span>
          </div>
        ))}
      </div>
    </>
  );
}

function V6Bottom({ focus, tab, setTab }) {
  const tabs = [
    { id: "back",  label: "Backlinks",  n: focus?.backlinks.length||0 },
    { id: "props", label: "Properties", n: focus?.props.length||0 },
    { id: "hist",  label: "History",    n: focus?.history.length||0 },
    { id: "out",   label: "Outline",    n: focus?.outline.length||0 },
    { id: "tasks", label: "Linked tasks", n: focus?.linkedTasks.length||0 },
  ];
  return (
    <div className="v6-bottom">
      <div className="tabs">
        {tabs.map(t => (
          <span key={t.id} className={`tab ${t.id===tab?"active":""}`} onClick={()=>setTab(t.id)}>
            {t.label} <span className="n">{t.n}</span>
          </span>
        ))}
      </div>
      <div className="body">
        {!focus ? null : tab==="back" ? focus.backlinks.map((b,i) => (
          <div key={i} className="v6-bl-card">
            <span className="src"><span className="lbl">{b.src}</span>· {b.label}</span>
            <span className="snip" dangerouslySetInnerHTML={{__html:b.snippet.replace(/\*([^*]+)\*/g,"<em>$1</em>")}}/>
          </div>
        )) : tab==="props" ? (
          <div style={{display:"grid",gridTemplateColumns:"100px 1fr",gap:"4px 14px",fontFamily:"JetBrains Mono",fontSize:12}}>
            {focus.props.map(p => (<React.Fragment key={p.k}><span style={{color:"#8c806a"}}>{p.k}</span><span style={{color:"#efe6d3"}}>{p.v}</span></React.Fragment>))}
          </div>
        ) : tab==="hist" ? focus.history.map((h,i) => (
          <div key={i} style={{fontFamily:"JetBrains Mono",fontSize:11.5,padding:"3px 0",color:"#c5b89e",display:"grid",gridTemplateColumns:"60px 40px 1fr",gap:8}}>
            <span style={{color:"#5a5246"}}>{h.when}</span><span style={{color:"#d49b5e"}}>{h.who}</span><span>{h.what}</span>
          </div>
        )) : tab==="out" ? focus.outline.map((o,i) => (
          <div key={i} style={{paddingLeft: o.indent*14, fontSize:12, color:"#c5b89e", padding:"3px 0"}}>· {o.text}</div>
        )) : focus.linkedTasks.map((t,i) => (
          <div key={i} style={{display:"grid",gridTemplateColumns:"1fr 60px 60px",gap:14,fontSize:12,padding:"4px 0",borderBottom:"1px dashed #322c24"}}>
            <span>{t.text}</span><span style={{color:"#d49b5e",fontFamily:"JetBrains Mono",fontSize:10.5,textTransform:"uppercase"}}>{t.status}</span><span style={{color:"#8c806a",fontFamily:"JetBrains Mono",fontSize:10.5}}>{t.deadline}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function V6() {
  const [widget, setWidget] = useState("tasks");
  const [selected, setSelected] = useState("t1");
  const [bottomOpen, setBottomOpen] = useState(true);
  const [bottomTab, setBottomTab] = useState("back");

  const listing = D.LISTINGS[widget];
  const focus = D.FOCUS[selected] || null;
  const widgetLabel = D.WIDGETS.find(w=>w.id===widget)?.label;

  return (
    <div className={`v6 ${bottomOpen?"with-bottom":""}`} data-screen-label="01 Columns xplr">
      <div className="v6-crumb">
        <span className="seg">Tesela</span><span className="sep">›</span>
        <span className="seg">{widgetLabel}</span><span className="sep">›</span>
        <span className="seg">{listing?.groups?.[0]?.label || "All"}</span><span className="sep">›</span>
        <span className="seg curr">{focus?.title || "—"}</span>
        <span className="sp"></span>
        <span className="end"><kbd>⌘K</kbd> jump · <kbd>⌃w</kbd>+<kbd>hjkl</kbd> split · <kbd>b</kbd> bottom</span>
      </div>

      <V6Rail active={widget} onPick={(id)=>{setWidget(id); setSelected(null);}}/>

      <div className="v6-middle">
        <div className="v6-pane-head">
          <span className="t">{listing?.title}</span>
          <span className="s">{listing?.subtitle}</span>
          {listing?.query && <span className="q">{listing.query}</span>}
        </div>
        <div className="v6-pane-body">
          <V6Listing data={listing} selected={selected} onSelect={setSelected}/>
        </div>
      </div>

      <div className="v6-focus">
        <V6Focus focus={focus}/>
      </div>

      {bottomOpen && <V6Bottom focus={focus} tab={bottomTab} setTab={setBottomTab}/>}

      <div className="v6-status">
        <span className="mode">NORMAL</span>
        <span>type:Task</span>
        <span className="sep">·</span>
        <span>{focus ? "1 focused" : "—"}</span>
        <span className={`toggle ${bottomOpen?"on":""}`} onClick={()=>setBottomOpen(v=>!v)}>[{bottomOpen?"×":"+"}] bottom panel</span>
        <span className="keys">
          <span><kbd>j</kbd>/<kbd>k</kbd> row</span>
          <span><kbd>h</kbd>/<kbd>l</kbd> col</span>
          <span><kbd>↵</kbd> drill</span>
          <span><kbd>:</kbd> cmd</span>
        </span>
      </div>
    </div>
  );
}

window.V6 = V6;

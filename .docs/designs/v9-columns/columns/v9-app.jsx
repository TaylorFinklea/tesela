// === v9 — xplr Monolith. Tokyo Night, cohesive UI fonts. ===
const { useState } = React;
const D = window.COL_DATA;

function v9Rich(s) {
  if (!s) return null;
  const out = []; const re = /(\[\[([^\]|]+)(?:\|([^\]]+))?\]\])|(#([A-Za-z][A-Za-z0-9_/-]*))/g;
  let last = 0, m, i = 0;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1]) out.push(<span key={`l${i++}`} className="v9link">{m[3] || m[2]}</span>);
    else out.push(<span key={`t${i++}`} className="v9tag">#{m[5]}</span>);
    last = re.lastIndex;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

// Build flat preview rows for a widget — used for inline expansion in the rail.
function v9Preview(widgetId, max = 6) {
  const L = D.LISTINGS[widgetId];
  if (!L) return [];
  const out = [];
  if (L.kind === "blocks") {
    // Daily / today: show top-level blocks only
    for (const b of L.blocks) {
      if (b.indent !== 0) continue;
      out.push({
        id: b.id,
        text: b.text,
        kind: b.kind === "task" ? "task" : "block",
        done: b.status === "done",
        urgent: b.priority === "high" || b.priority === "critical",
      });
      if (out.length >= max) break;
    }
    return out;
  }
  // Grouped listings: pull a couple from each group, label the group
  for (const g of L.groups || []) {
    out.push({ sub: g.label });
    for (const it of g.items) {
      out.push({
        id: it.id,
        text: it.text,
        kind: widgetId === "projects" ? "project"
            : widgetId === "people" ? "person"
            : widgetId === "queries" ? "query"
            : widgetId === "recent" ? "recent"
            : widgetId === "pinned" ? "pin"
            : widgetId === "inbox" ? "inbox"
            : widgetId === "tasks" ? "task" : "block",
        done: false,
        meta: it.deadline && it.deadline !== "—" ? it.deadline : (it.priority && it.priority !== "—" ? it.priority : ""),
        urgent: ["Apr 25","Apr 26"].includes(it.deadline) || it.priority === "critical",
      });
      if (out.length >= max + 2) break; // +2 to account for sub headers
    }
    if (out.length >= max + 2) break;
  }
  return out;
}

function V9MiniCal({ selected, onPick }) {
  // Anchor: Apr 29, 2026 (today in this mock)
  const TODAY = { y: 2026, m: 3, d: 29 }; // m is 0-indexed (3 = Apr)
  const [view, setView] = useState({ y: TODAY.y, m: TODAY.m });
  const monthName = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"][view.m];
  const first = new Date(view.y, view.m, 1);
  const startDow = first.getDay(); // 0 = Sun
  const daysInMonth = new Date(view.y, view.m + 1, 0).getDate();
  const daysInPrev = new Date(view.y, view.m, 0).getDate();

  // Marks for our mock month (April 2026)
  const marks = view.y === 2026 && view.m === 3 ? {
    25: ["task"],
    26: ["task","event"],
    28: ["note"],
    29: ["task","event","note"],
    30: ["task"],
  } : {};

  const cells = [];
  // leading muted days
  for (let i = startDow - 1; i >= 0; i--) cells.push({ d: daysInPrev - i, muted: true });
  // current month
  for (let d = 1; d <= daysInMonth; d++) cells.push({ d, muted: false });
  // trailing muted days to fill 6 rows × 7 cols = 42 cells, but cap to fill last row only
  const trailing = (7 - (cells.length % 7)) % 7;
  for (let d = 1; d <= trailing; d++) cells.push({ d, muted: true });

  const sel = selected || `${TODAY.y}-${TODAY.m}-${TODAY.d}`;

  return (
    <div className="v9-cal">
      <div className="cal-head">
        <span className="month">{monthName} {view.y}</span>
        <span className="nav">
          <span onClick={()=>setView(v => v.m === 0 ? {y:v.y-1,m:11} : {y:v.y,m:v.m-1})}>‹</span>
          <span onClick={()=>setView({y:TODAY.y, m:TODAY.m})} title="today">●</span>
          <span onClick={()=>setView(v => v.m === 11 ? {y:v.y+1,m:0} : {y:v.y,m:v.m+1})}>›</span>
        </span>
      </div>
      <div className="cal-grid">
        {["S","M","T","W","T","F","S"].map((d,i) => <div key={`dow${i}`} className="dow">{d}</div>)}
        {cells.map((c, i) => {
          const isToday = !c.muted && view.y === TODAY.y && view.m === TODAY.m && c.d === TODAY.d;
          const id = `${view.y}-${view.m}-${c.d}`;
          const isSel = !c.muted && id === sel;
          const dayMarks = !c.muted ? (marks[c.d] || []) : [];
          return (
            <div
              key={i}
              className={`day ${c.muted?"muted":""} ${isToday?"today":""} ${isSel?"selected":""}`}
              onClick={()=>!c.muted && onPick(id)}
            >
              {c.d}
              {dayMarks.length > 0 && (
                <span className="marks">
                  {dayMarks.map((k, j) => <i key={j} className={k}></i>)}
                </span>
              )}
            </div>
          );
        })}
      </div>
      <div className="cal-foot">
        <span className="lg task"><i></i>tasks</span>
        <span className="lg event"><i></i>events</span>
        <span className="lg note"><i></i>notes</span>
      </div>
    </div>
  );
}

function V9Rail({ active, expanded, onToggle, onPickItem, selected }) {
  const groups = [
    { label: "Pinned", items: ["today","tasks","inbox","calendar"] },
    { label: "Types",  items: ["projects","people"] },
    { label: "Saved",  items: ["queries","recent","pinned"] },
  ];
  const byId = Object.fromEntries(D.WIDGETS.map(w=>[w.id,w]));
  return (
    <div className="v9-rail">
      <div className="v9-rail-scroll">
      {groups.map(g => (
        <React.Fragment key={g.label}>
          <div className="group">{g.label}</div>
          {g.items.map(id => {
            const w = byId[id]; if (!w) return null;
            const isExpanded = expanded.has(id);
            const isActive = active === id;
            const preview = isExpanded ? v9Preview(id) : [];
            return (
              <React.Fragment key={id}>
                <div
                  className={`w ${isActive?"active":""} ${isExpanded?"expanded":""}`}
                  data-icon={w.icon}
                  onClick={()=>onToggle(id)}
                >
                  <span className="gl">{w.label[0]}</span>
                  <span>{w.label}</span>
                  <span className="badge">{w.badge || w.count || ""}</span>
                  <span className="caret">▸</span>
                </div>
                {isExpanded && (
                  <div className="preview">
                    {preview.length === 0 && <div className="seeall empty">— empty —</div>}
                    {preview.map((p, i) => {
                      if (p.sub) return <div key={`s${i}`} className="sub">{p.sub}</div>;
                      const sel = p.id === selected;
                      return (
                        <div
                          key={p.id}
                          className={`pi k-${p.kind} ${p.done?"done":""} ${sel?"selected":""} ${p.urgent?"urgent":""}`}
                          onClick={(e)=>{ e.stopPropagation(); onPickItem(id, p.id); }}
                        >
                          {p.kind === "task"
                            ? <span className="check"></span>
                            : <span className="dot"></span>}
                          <span className="label">{p.text}</span>
                          {p.meta && <span className="badge">{p.meta}</span>}
                        </div>
                      );
                    })}
                    <div className="seeall" onClick={(e)=>{ e.stopPropagation(); onPickItem(id, null); }}>see all in {w.label.toLowerCase()} →</div>
                  </div>
                )}
              </React.Fragment>
            );
          })}
        </React.Fragment>
      ))}
      <div className="add">+ add widget</div>
      </div>
      <V9MiniCal onPick={(id)=>onPickItem("calendar", null)}/>
    </div>
  );
}

function V9Listing({ data, selected, onSelect }) {
  if (!data) return null;
  if (data.kind === "blocks") {
    return (
      <div className="v9-bl">
        {data.blocks.map(b => (
          <div key={b.id} className={`blk indent-${b.indent} k-${b.kind} ${b.id===selected?"selected":""}`} onClick={()=>onSelect(b.id)}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v9Rich(b.text)}</span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div>
      {data.groups.map(g => (
        <div key={g.label}>
          <div className="v9-grp">{g.label} <span style={{opacity:0.55}}>· {g.items.length}</span></div>
          {g.items.map(it => (
            <div key={it.id} className={`v9-row ${it.id===selected?"selected":""}`} onClick={()=>onSelect(it.id)}>
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

function V9Focus({ focus }) {
  if (!focus) return <div style={{padding:"30px 14px",color:"#545c7e",fontFamily:"JetBrains Mono"}}>— select a row to focus —</div>;
  return (
    <>
      <div className="v9-pane-head">
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
      <div className="v9-bl" style={{paddingTop:14}}>
        {focus.tree.map((b,i) => (
          <div key={i} className={`blk indent-${b.indent} k-${b.kind}`}>
            <span className="bull">{b.indent===0?"●":"·"}</span>
            <span>{b.kind!=="note" && <span className="ctype">{b.kind}</span>}{v9Rich(b.text)}</span>
          </div>
        ))}
      </div>
    </>
  );
}

function V9Bottom({ focus, tab, setTab }) {
  const tabs = [
    { id: "back",  label: "Backlinks",  n: focus?.backlinks.length||0 },
    { id: "props", label: "Properties", n: focus?.props.length||0 },
    { id: "hist",  label: "History",    n: focus?.history.length||0 },
    { id: "out",   label: "Outline",    n: focus?.outline.length||0 },
    { id: "tasks", label: "Linked tasks", n: focus?.linkedTasks.length||0 },
  ];
  return (
    <div className="v9-bottom">
      <div className="tabs">
        {tabs.map(t => (
          <span key={t.id} className={`tab ${t.id===tab?"active":""}`} onClick={()=>setTab(t.id)}>
            {t.label} <span className="n">{t.n}</span>
          </span>
        ))}
      </div>
      <div className="body">
        {!focus ? null : tab==="back" ? focus.backlinks.map((b,i) => (
          <div key={i} className="v9-bl-card">
            <span className="src"><span className="lbl">{b.src}</span>· {b.label}</span>
            <span className="snip" dangerouslySetInnerHTML={{__html:b.snippet.replace(/\*([^*]+)\*/g,"<em>$1</em>")}}/>
          </div>
        )) : tab==="props" ? (
          <div style={{display:"grid",gridTemplateColumns:"100px 1fr",gap:"4px 14px",fontFamily:"JetBrains Mono",fontSize:12}}>
            {focus.props.map(p => (<React.Fragment key={p.k}><span style={{color:"#737aa2"}}>{p.k}</span><span style={{color:"#c0caf5"}}>{p.v}</span></React.Fragment>))}
          </div>
        ) : tab==="hist" ? focus.history.map((h,i) => (
          <div key={i} style={{fontFamily:"JetBrains Mono",fontSize:11.5,padding:"3px 0",color:"#a9b1d6",display:"grid",gridTemplateColumns:"60px 40px 1fr",gap:8}}>
            <span style={{color:"#545c7e"}}>{h.when}</span><span style={{color:"#ff9e64"}}>{h.who}</span><span>{h.what}</span>
          </div>
        )) : tab==="out" ? focus.outline.map((o,i) => (
          <div key={i} style={{paddingLeft: o.indent*14, fontSize:12, color:"#a9b1d6", padding:"3px 0"}}>· {o.text}</div>
        )) : focus.linkedTasks.map((t,i) => (
          <div key={i} style={{display:"grid",gridTemplateColumns:"1fr 60px 60px",gap:14,fontSize:12,padding:"4px 0",borderBottom:"1px dashed #2f334d"}}>
            <span>{t.text}</span><span style={{color:"#ff9e64",fontFamily:"JetBrains Mono",fontSize:10.5,textTransform:"uppercase"}}>{t.status}</span><span style={{color:"#737aa2",fontFamily:"JetBrains Mono",fontSize:10.5}}>{t.deadline}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function V9() {
  const [widget, setWidget] = useState("tasks");
  const [selected, setSelected] = useState("t1");
  const [bottomOpen, setBottomOpen] = useState(true);
  const [bottomTab, setBottomTab] = useState("back");
  const [expanded, setExpanded] = useState(new Set(["tasks","today"]));
  const toggleExpand = (id) => {
    setExpanded(prev => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id); else n.add(id);
      return n;
    });
    setWidget(id);
  };
  const pickFromRail = (widgetId, itemId) => {
    setWidget(widgetId);
    if (itemId) setSelected(itemId);
  };

  const listing = D.LISTINGS[widget];
  const focus = D.FOCUS[selected] || null;
  const widgetLabel = D.WIDGETS.find(w=>w.id===widget)?.label;

  return (
    <div className={`v9 ${bottomOpen?"with-bottom":""}`} data-screen-label="01 Columns v9">
      <div className="v9-crumb">
        <span className="seg">Tesela</span><span className="sep">›</span>
        <span className="seg">{widgetLabel}</span><span className="sep">›</span>
        <span className="seg">{listing?.groups?.[0]?.label || "All"}</span><span className="sep">›</span>
        <span className="seg curr">{focus?.title || "—"}</span>
        <span className="sp"></span>
        <span className="end"><kbd>⌘K</kbd> jump · <kbd>⌃w</kbd>+<kbd>hjkl</kbd> split · <kbd>b</kbd> bottom</span>
      </div>

      <V9Rail active={widget} expanded={expanded} onToggle={toggleExpand} onPickItem={pickFromRail} selected={selected}/>

      <div className="v9-middle">
        <div className="v9-pane-head">
          <span className="t">{listing?.title}</span>
          <span className="s">{listing?.subtitle}</span>
          {listing?.query && <span className="q">{listing.query}</span>}
        </div>
        <div className="v9-pane-body">
          <V9Listing data={listing} selected={selected} onSelect={setSelected}/>
        </div>
      </div>

      <div className="v9-focus">
        <V9Focus focus={focus}/>
      </div>

      {bottomOpen && <V9Bottom focus={focus} tab={bottomTab} setTab={setBottomTab}/>}

      <div className="v9-status">
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

window.V9 = V9;

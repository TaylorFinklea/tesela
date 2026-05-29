/* GRAPHITE — shared shell + complete stylesheet.
   The Graphite direction, productionized into a reusable shell so every
   screen renders the same top bar / widget rail / status line. All CSS for
   every Graphite screen lives here in ONE place so it's easy to tweak.

   Contrast pass (per feedback): the foreground ramp was lifted — widget
   headers, pinned rows, icons and faint metadata now read clearly against
   the dark surfaces instead of dissolving into them. */

(function () {
  if (document.getElementById('grx-styles')) return;
  const css = `
  .gr-root{
    /* surfaces */
    --bg:#0E1014; --surface:#14171D; --raised:#1A1E26; --raised-2:#20242D; --raised-3:#272C37;
    --line:rgba(255,255,255,.07); --line-2:rgba(255,255,255,.12); --line-3:rgba(255,255,255,.18);
    /* foreground ramp — lifted for contrast */
    --fg:#EDEFF2; --fg2:#CBD0D9; --muted:#AAB0BB; --subtle:#8A909C; --faint:#646B78;
    /* accent + semantics */
    --coral:#FF6B5A; --coral-dim:rgba(255,107,90,.14); --coral-line:rgba(255,107,90,.40);
    --task:#E8697F; --event:#62B8CE; --note:#E4AE66; --project:#7493E8; --person:#AE90E6; --query:#85BC63;
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    position:absolute; inset:0; background:var(--bg); color:var(--fg);
    font-family:var(--sans); font-size:13.5px; line-height:1.5;
    display:grid; grid-template-rows:48px 1fr 30px; overflow:hidden; -webkit-font-smoothing:antialiased;
  }
  .gr-root *{box-sizing:border-box;}
  .gr-mono{font-family:var(--mono);}

  /* ── top bar ── */
  .gr-top{display:grid; grid-template-columns:auto 1fr auto; align-items:center; gap:18px;
    padding:0 16px; border-bottom:1px solid var(--line); background:var(--surface); z-index:5;}
  .gr-brand{display:flex; align-items:center; gap:9px;}
  .gr-brand .nm{font-size:13.5px; font-weight:600; letter-spacing:-.01em; color:var(--fg);}
  .gr-tabs{display:flex; align-items:center; gap:4px; margin-left:6px; min-width:0; overflow:hidden;}
  .gr-tab{display:flex; align-items:center; gap:7px; height:30px; padding:0 11px; border-radius:8px; white-space:nowrap;
    color:var(--subtle); cursor:pointer; font-size:12.5px; border:1px solid transparent; transition:all .14s;}
  .gr-tab:hover{color:var(--fg2); background:var(--raised);}
  .gr-tab.active{color:var(--fg); background:var(--raised); border-color:var(--line-2);}
  .gr-tab .kdot{width:6px; height:6px; border-radius:50%; background:var(--coral);}
  .gr-tab.active .nm{font-weight:550;}
  .gr-cmd{justify-self:center; width:min(440px,100%); display:flex; align-items:center; gap:9px;
    height:32px; padding:0 11px; border-radius:9px; background:var(--bg); border:1px solid var(--line-2);
    color:var(--subtle); cursor:text; transition:border-color .14s;}
  .gr-cmd:hover{border-color:var(--line-3);}
  .gr-cmd .ph{flex:1; font-size:12.5px;}
  .gr-cmd kbd{font-family:var(--mono); font-size:10.5px; color:var(--subtle); background:var(--raised);
    border:1px solid var(--line); border-radius:5px; padding:2px 6px; line-height:1;}
  .gr-icons{display:flex; align-items:center; gap:2px;}
  .gr-ic{width:30px; height:30px; display:grid; place-items:center; border-radius:8px; color:var(--subtle);
    cursor:pointer; transition:all .14s;}
  .gr-ic:hover{color:var(--fg); background:var(--raised);}
  .gr-conn{width:30px; height:30px; display:grid; place-items:center;}
  .gr-conn i{width:7px; height:7px; border-radius:50%; background:var(--query); box-shadow:0 0 0 3px rgba(133,188,99,.16);}

  /* ── body + rail ── */
  .gr-body{display:flex; min-height:0; overflow:hidden; position:relative;}
  .gr-rail{width:256px; flex-shrink:0; background:var(--surface); border-right:1px solid var(--line);
    display:flex; flex-direction:column; min-height:0;}
  .gr-rail-scroll{flex:1; overflow:hidden; padding:12px 10px; display:flex; flex-direction:column; gap:8px;}

  .gr-w{background:var(--raised); border:1px solid var(--line); border-radius:11px; overflow:hidden;}
  .gr-w-head{display:flex; align-items:center; gap:8px; padding:9px 11px 7px;}
  .gr-w-head .ti{flex:1; font-size:11px; font-weight:600; letter-spacing:.04em; text-transform:uppercase; color:var(--fg2);}
  .gr-w-head .ic{color:var(--subtle);}
  .gr-w-head .bd{font-family:var(--mono); font-size:10px; color:var(--subtle); background:var(--bg); white-space:nowrap;
    border:1px solid var(--line); border-radius:5px; padding:1px 6px;}
  .gr-w-head .caret{color:var(--faint); margin-left:2px;}
  .gr-w-body{padding:2px 7px 9px;}

  .gr-capture{display:flex; align-items:center; gap:8px; margin:0 4px 4px; padding:9px 11px; border-radius:8px;
    background:var(--bg); border:1px solid var(--line); color:var(--subtle); font-size:12.5px;}
  .gr-capture .pl{flex:1;}
  .gr-capture .pk{font-family:var(--mono); font-size:10px; color:var(--faint);}

  .gr-row{display:flex; align-items:center; gap:9px; padding:6px 8px; border-radius:7px; cursor:pointer;
    color:var(--fg2); font-size:12.5px; transition:background .12s;}
  .gr-row:hover{background:var(--raised-2);}
  .gr-row.active{background:var(--raised-2); color:var(--fg);}
  .gr-row.active::before{content:""; position:absolute; }
  .gr-row .ic{color:var(--subtle);}
  .gr-row .lb{flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .gr-row .mt{font-family:var(--mono); font-size:10.5px; color:var(--faint); font-variant-numeric:tabular-nums; white-space:nowrap;}
  .gr-row .mt.urg{color:var(--coral);}
  .gr-dot{width:6px; height:6px; border-radius:50%; flex-shrink:0;}
  .gr-dot.event{background:var(--event);} .gr-dot.task{background:var(--task);}
  .gr-dot.note{background:var(--note);} .gr-dot.project{background:var(--project);} .gr-dot.person{background:var(--person);}
  .gr-sub{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase;
    color:var(--faint); padding:7px 8px 3px;}
  .gr-check{width:14px; height:14px; border-radius:4px; border:1.5px solid var(--subtle); flex-shrink:0;}
  .gr-check.task{border-color:color-mix(in srgb,var(--task) 75%,transparent);}
  .gr-check.done{background:var(--task); border-color:var(--task); position:relative;}
  .gr-addw{display:flex; align-items:center; justify-content:center; gap:7px; margin-top:auto; white-space:nowrap;
    padding:9px; border-radius:8px; border:1px dashed var(--line-2); color:var(--subtle); font-size:12px; cursor:pointer;}
  .gr-addw:hover{color:var(--fg2); border-color:var(--line-3);}

  /* ── main area shared ── */
  .gr-main{flex:1; display:flex; min-width:0; min-height:0;}
  .gr-pane{flex:1; min-width:0; display:flex; flex-direction:column; background:var(--bg); min-height:0;}
  .gr-pane.side{flex:1; background:var(--surface); border-left:1px solid var(--line); max-width:420px;}
  .gr-pane.focus{flex:1.7;}
  .gr-pane-head{display:flex; align-items:center; gap:11px; padding:14px 18px 12px; border-bottom:1px solid var(--line); flex-shrink:0;}
  .gr-back{color:var(--subtle); cursor:pointer;}
  .gr-pane-head .ttl{font-size:16px; font-weight:600; letter-spacing:-.01em; color:var(--fg); white-space:nowrap;}
  .gr-pane-head .sub{font-family:var(--mono); font-size:10.5px; color:var(--faint);}
  .gr-pane-head .sp{flex:1;}
  .gr-pane-head .meta{font-family:var(--mono); font-size:10.5px; color:var(--faint); white-space:nowrap;}
  .gr-typetag{display:inline-flex; align-items:center; gap:6px; height:21px; padding:0 9px; border-radius:6px;
    font-family:var(--mono); font-size:10.5px; letter-spacing:.02em; background:var(--raised); border:1px solid var(--line-2); color:var(--project);}
  .gr-typetag .sw{width:6px; height:6px; border-radius:2px; background:var(--project);}
  .gr-typetag.task{color:var(--task);} .gr-typetag.task .sw{background:var(--task);}
  .gr-headbtn{display:inline-flex; align-items:center; gap:6px; height:28px; padding:0 11px; border-radius:8px; white-space:nowrap;
    background:var(--raised); border:1px solid var(--line-2); color:var(--fg2); font-size:12px; cursor:pointer;}
  .gr-headbtn:hover{background:var(--raised-2); color:var(--fg);}
  .gr-headbtn.cta{background:var(--coral); color:#10110f; border-color:transparent; font-weight:600;}

  /* ── outliner (project / daily) ── */
  .gr-outline{flex:1; overflow:hidden; padding:14px 18px;}
  .gr-blk{position:relative; padding:7px 10px 7px 8px; border-radius:9px; border-left:2px solid transparent;}
  .gr-blk.sel{background:var(--raised); border-left-color:var(--coral);}
  .gr-blk-main{display:flex; align-items:flex-start; gap:10px;}
  .gr-bull{width:7px; height:7px; border-radius:50%; background:var(--faint); margin-top:7px; flex-shrink:0;}
  .gr-blk.task > .gr-blk-main > .gr-bull{background:var(--task);}
  .gr-blk.done > .gr-blk-main > .gr-bull{background:var(--query);}
  .gr-blk-body{flex:1 1 0%; min-width:0;}
  .gr-blk-text{font-size:14.5px; color:var(--fg); line-height:1.45; letter-spacing:-.005em;}
  .gr-blk.done .gr-blk-text{color:var(--subtle); text-decoration:line-through; text-decoration-color:var(--faint);}
  .gr-tagchip{display:inline-flex; align-items:center; height:18px; padding:0 7px; margin-left:7px; border-radius:5px;
    font-family:var(--mono); font-size:10.5px; vertical-align:1px; background:var(--coral-dim); color:var(--coral);}
  .gr-tagchip.alt{background:rgba(232,105,127,.15); color:var(--task);}
  .gr-tagchip.proj{background:rgba(116,147,232,.15); color:var(--project);}
  .gr-props{display:flex; flex-wrap:wrap; gap:6px; margin-top:8px;}
  .gr-pchip{display:inline-flex; align-items:center; gap:7px; height:23px; padding:0 9px; border-radius:7px; white-space:nowrap; flex-shrink:0;
    background:var(--surface); border:1px solid var(--line); font-family:var(--mono); font-size:11px;}
  .gr-pchip .k{color:var(--faint);} .gr-pchip .v{color:var(--fg2);}
  .gr-pchip.doing .v{color:var(--coral); font-weight:600;}
  .gr-pchip.high .v{color:var(--task); font-weight:600;}
  .gr-pchip .lk{color:var(--project);}
  .gr-kids{margin:6px 0 2px 18px; padding-left:14px; border-left:1px solid var(--line);}
  .gr-kid{display:flex; align-items:center; gap:9px; padding:5px 6px; border-radius:7px; cursor:pointer;}
  .gr-kid:hover{background:var(--raised);}
  .gr-kid .kb{width:5px; height:5px; border-radius:50%; background:var(--faint); flex-shrink:0;}
  .gr-kid .kt{font-size:13px; color:var(--fg2); flex:1 1 0%; min-width:0;}
  .gr-mention{color:var(--person); background:rgba(174,144,230,.14); padding:0 4px; border-radius:4px;}
  .gr-link{color:var(--project); background:rgba(116,147,232,.14); padding:0 4px; border-radius:4px;}

  /* day divider (daily journal) */
  .gr-dayhdr{display:flex; align-items:center; gap:11px; padding:6px 8px 10px; margin-top:4px;}
  .gr-dayhdr .d{font-size:15px; font-weight:600; color:var(--fg); letter-spacing:-.01em;}
  .gr-dayhdr .dow{font-family:var(--mono); font-size:10.5px; color:var(--faint); text-transform:uppercase; letter-spacing:.08em;}
  .gr-dayhdr .ln{flex:1; height:1px; background:var(--line);}
  .gr-dayhdr.today .d{color:var(--coral);}

  /* ── refs / side pane ── */
  .gr-side-body{flex:1; overflow:hidden; padding:12px 14px; display:flex; flex-direction:column; gap:9px;}
  .gr-refcard{padding:10px 12px; border-radius:10px; background:var(--raised); border:1px solid var(--line);}
  .gr-refcard .src{display:flex; align-items:center; gap:7px; font-family:var(--mono); font-size:10.5px; color:var(--fg2); margin-bottom:5px;}
  .gr-refcard .snip{font-size:12.5px; color:var(--muted); line-height:1.5;}
  .gr-refcard .snip em{font-style:normal; color:var(--fg); background:var(--coral-dim); padding:0 3px; border-radius:3px;}
  .gr-proplist{margin-top:2px;}
  .gr-proplist .ph{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:4px 2px 7px;}
  .gr-prow{display:grid; grid-template-columns:18px 84px 1fr; align-items:center; gap:8px; padding:6px 7px; border-radius:7px;}
  .gr-prow:hover{background:var(--raised);}
  .gr-prow .chord{font-family:var(--mono); font-size:9.5px; text-align:center; color:var(--subtle);
    background:var(--surface); border:1px solid var(--line); border-radius:4px; padding:2px 0;}
  .gr-prow .k{font-family:var(--mono); font-size:11px; color:var(--subtle);}
  .gr-prow .v{font-family:var(--mono); font-size:11px; color:var(--fg2); justify-self:start; white-space:nowrap;}
  .gr-prow .v.doing{color:var(--coral);} .gr-prow .v.high{color:var(--task);}

  /* ── status line ── */
  .gr-status{display:flex; align-items:center; gap:12px; padding:0 14px; background:var(--surface); white-space:nowrap; overflow:hidden;
    border-top:1px solid var(--line); font-family:var(--mono); font-size:11px; color:var(--subtle); z-index:5;}
  .gr-status .mode{color:var(--coral); font-weight:700; letter-spacing:.10em; font-size:10px;}
  .gr-status .sep{color:var(--faint);}
  .gr-status .keys{margin-left:auto; display:flex; gap:14px;}
  .gr-status .keys span{color:var(--faint);} .gr-status .keys kbd{color:var(--fg2); font-family:var(--mono);}
  .gr-status .clk{color:var(--faint); display:flex; align-items:center; gap:5px;}

  /* ════════ INBOX ════════ */
  .gr-chipbar{display:flex; align-items:center; gap:7px; padding:11px 18px; border-bottom:1px solid var(--line); flex-wrap:wrap;}
  .gr-chip{display:inline-flex; align-items:center; gap:6px; height:26px; padding:0 11px; border-radius:8px; cursor:pointer;
    background:var(--raised); border:1px solid var(--line); color:var(--fg2); font-size:12px;}
  .gr-chip:hover{border-color:var(--line-2);}
  .gr-chip.active{background:var(--coral-dim); border-color:var(--coral-line); color:var(--coral);}
  .gr-chip .n{font-family:var(--mono); font-size:10px; color:var(--faint);}
  .gr-chip.active .n{color:var(--coral);}
  .gr-inbox-body{flex:1; overflow:hidden; padding:12px 18px; display:flex; flex-direction:column; gap:8px;}
  .gr-icard{display:flex; align-items:flex-start; gap:12px; padding:13px 14px; border-radius:11px; background:var(--surface); border:1px solid var(--line); transition:border-color .14s;}
  .gr-icard:hover{border-color:var(--line-2);}
  .gr-icard.sel{background:var(--raised); border-color:var(--coral-line);}
  .gr-icard .src{width:30px; height:30px; border-radius:8px; display:grid; place-items:center; background:var(--raised-2); color:var(--subtle); flex-shrink:0;}
  .gr-icard-body{flex:1; min-width:0;}
  .gr-icard .txt{font-size:14px; color:var(--fg); line-height:1.45;}
  .gr-icard .meta{display:flex; align-items:center; gap:10px; margin-top:7px; font-family:var(--mono); font-size:10.5px; color:var(--faint);}
  .gr-icard .meta .pill{display:inline-flex; align-items:center; gap:5px; padding:2px 7px; border-radius:5px; background:var(--raised-2); color:var(--subtle); white-space:nowrap;}
  .gr-icard-acts{display:flex; align-items:center; gap:4px; flex-shrink:0;}
  .gr-iact{width:28px; height:28px; display:grid; place-items:center; border-radius:7px; color:var(--subtle); cursor:pointer; border:1px solid transparent;}
  .gr-iact:hover{background:var(--raised-2); color:var(--fg); border-color:var(--line);}
  .gr-iact.go:hover{color:var(--coral);}

  /* ════════ AGENDA ════════ */
  .gr-agenda{flex:1; overflow:hidden; display:flex; flex-direction:column;}
  .gr-agrid{flex:1; display:grid; grid-template-columns:56px repeat(5,1fr); overflow:hidden;}
  .gr-ag-col{border-right:1px solid var(--line); position:relative; min-width:0;}
  .gr-ag-col:last-child{border-right:none;}
  .gr-ag-times{display:flex; flex-direction:column;}
  .gr-ag-time{height:62px; font-family:var(--mono); font-size:9.5px; color:var(--faint); text-align:right; padding:2px 7px 0 0; border-top:1px solid var(--line);}
  .gr-ag-daycol{display:flex; flex-direction:column;}
  .gr-ag-slot{height:62px; border-top:1px solid var(--line);}
  .gr-ag-colhdr{height:46px; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:1px; border-bottom:1px solid var(--line); border-right:1px solid var(--line);}
  .gr-ag-colhdr .dw{font-family:var(--mono); font-size:9.5px; letter-spacing:.08em; text-transform:uppercase; color:var(--faint);}
  .gr-ag-colhdr .dn{font-size:15px; font-weight:600; color:var(--fg2);}
  .gr-ag-colhdr.today .dn{color:#10110f; background:var(--coral); width:25px; height:25px; border-radius:50%; display:grid; place-items:center; font-size:13px;}
  .gr-ag-corner{border-bottom:1px solid var(--line); border-right:1px solid var(--line);}
  .gr-ev{position:absolute; left:5px; right:5px; border-radius:7px; padding:6px 8px; overflow:hidden; border:1px solid transparent; cursor:pointer;}
  .gr-ev .et{font-size:11.5px; font-weight:550; line-height:1.25; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;}
  .gr-ev .em{font-family:var(--mono); font-size:9.5px; opacity:.8; margin-top:1px;}
  .gr-ev.event{background:rgba(98,184,206,.16); border-color:rgba(98,184,206,.34); color:var(--event);}
  .gr-ev.task{background:rgba(232,105,127,.15); border-color:rgba(232,105,127,.34); color:var(--task);}
  .gr-ev.focus-ev{background:rgba(255,107,90,.16); border-color:var(--coral-line); color:var(--coral);}
  .gr-now{position:absolute; left:0; right:0; height:0; border-top:1.5px solid var(--coral); z-index:3;}
  .gr-now::before{content:""; position:absolute; left:-3px; top:-3.5px; width:7px; height:7px; border-radius:50%; background:var(--coral);}

  /* ════════ TAG PAGE ════════ */
  .gr-tagbody{flex:1; overflow:hidden; padding:16px 22px; display:flex; flex-direction:column; gap:16px;}
  .gr-schema{background:var(--surface); border:1px solid var(--line); border-radius:12px; padding:13px 15px;}
  .gr-schema .h{display:flex; align-items:center; gap:8px; font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); margin-bottom:11px;}
  .gr-schema-grid{display:grid; grid-template-columns:repeat(2,1fr); gap:8px;}
  .gr-pdef{display:flex; align-items:center; gap:10px; padding:8px 10px; border-radius:8px; background:var(--raised); border:1px solid var(--line);}
  .gr-pdef .pk{font-family:var(--mono); font-size:12px; color:var(--fg2); flex:1;}
  .gr-pdef .pt{font-family:var(--mono); font-size:10px; color:var(--subtle); padding:1px 7px; border-radius:5px; background:var(--raised-2);}
  .gr-pdef .pic{color:var(--note);}
  .gr-tbl-wrap{flex:1; overflow:hidden; border:1px solid var(--line); border-radius:12px; background:var(--surface);}
  .gr-tbl{width:100%; border-collapse:collapse;}
  .gr-tbl th{text-align:left; font-family:var(--mono); font-size:9.5px; letter-spacing:.08em; text-transform:uppercase; color:var(--faint);
    font-weight:500; padding:10px 14px; border-bottom:1px solid var(--line); background:var(--raised);}
  .gr-tbl td{padding:11px 14px; border-bottom:1px solid var(--line); font-size:13px; color:var(--fg2); vertical-align:middle;}
  .gr-tbl tr:last-child td{border-bottom:none;}
  .gr-tbl tr:hover td{background:var(--raised);}
  .gr-tbl .c-text{color:var(--fg); display:flex; align-items:center; gap:9px;}
  .gr-tbl .cell-chip{display:inline-flex; align-items:center; height:20px; padding:0 8px; border-radius:6px; font-family:var(--mono); font-size:10.5px; background:var(--raised-2); color:var(--muted);}
  .gr-tbl .cell-chip.doing{background:var(--coral-dim); color:var(--coral);}
  .gr-tbl .cell-chip.todo{color:var(--subtle);}
  .gr-tbl .cell-chip.done{background:rgba(133,188,99,.15); color:var(--query);}
  .gr-tbl .cell-chip.high{background:rgba(232,105,127,.15); color:var(--task);}
  .gr-tbl .due{font-family:var(--mono); font-size:11.5px; color:var(--muted);} .gr-tbl .due.urg{color:var(--coral);}

  /* ════════ SETTINGS ════════ */
  .gr-set{flex:1; display:flex; min-height:0;}
  .gr-set-nav{width:200px; flex-shrink:0; border-right:1px solid var(--line); padding:14px 10px; display:flex; flex-direction:column; gap:2px;}
  .gr-set-nav .navh{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:6px 10px 7px;}
  .gr-set-nav .item{display:flex; align-items:center; gap:10px; padding:8px 11px; border-radius:8px; color:var(--fg2); font-size:13px; cursor:pointer;}
  .gr-set-nav .item:hover{background:var(--raised);}
  .gr-set-nav .item.active{background:var(--raised-2); color:var(--fg);}
  .gr-set-nav .item.active .ic{color:var(--coral);}
  .gr-set-nav .item .ic{color:var(--subtle);}
  .gr-set-main{flex:1; min-width:0; overflow:hidden; padding:24px 30px;}
  .gr-set-main .h1{font-size:19px; font-weight:600; letter-spacing:-.015em; color:var(--fg);}
  .gr-set-main .h1-sub{font-size:13px; color:var(--subtle); margin-top:3px;}
  .gr-set-sec{margin-top:22px;}
  .gr-set-sec .sech{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); margin-bottom:10px;}
  .gr-field{display:flex; align-items:center; gap:14px; padding:13px 0; border-bottom:1px solid var(--line);}
  .gr-field .fl{flex:1;}
  .gr-field .fl .ft{font-size:13.5px; color:var(--fg);}
  .gr-field .fl .fd{font-size:12px; color:var(--subtle); margin-top:2px;}
  .gr-input{height:32px; padding:0 11px; border-radius:8px; background:var(--bg); border:1px solid var(--line-2); color:var(--fg2);
    font-family:var(--mono); font-size:12px; min-width:240px;}
  .gr-toggle{width:38px; height:22px; border-radius:11px; background:var(--raised-3); position:relative; cursor:pointer; flex-shrink:0; transition:background .16s;}
  .gr-toggle::after{content:""; position:absolute; top:3px; left:3px; width:16px; height:16px; border-radius:50%; background:var(--subtle); transition:all .16s;}
  .gr-toggle.on{background:var(--coral);} .gr-toggle.on::after{left:19px; background:#10110f;}
  .gr-devrow{display:flex; align-items:center; gap:12px; padding:12px 13px; border-radius:10px; background:var(--surface); border:1px solid var(--line); margin-bottom:8px;}
  .gr-devrow .di{width:32px; height:32px; border-radius:8px; display:grid; place-items:center; background:var(--raised-2); color:var(--subtle);}
  .gr-devrow .dn{flex:1;} .gr-devrow .dn .nm2{font-size:13.5px; color:var(--fg);} .gr-devrow .dn .ds{font-family:var(--mono); font-size:11px; color:var(--subtle); margin-top:1px;}
  .gr-devrow .badge{font-family:var(--mono); font-size:10px; color:var(--query); display:flex; align-items:center; gap:5px;}
  .gr-devrow .badge i{width:6px; height:6px; border-radius:50%; background:var(--query);}

  /* ════════ OVERLAYS (command / leader / graph) ════════ */
  .gr-scrim{position:absolute; inset:0; z-index:40; background:rgba(8,9,12,.58); backdrop-filter:blur(3px); display:flex; flex-direction:column; align-items:center;}
  .gr-cmdk{width:min(640px,92%); margin-top:72px; background:var(--raised); border:1px solid var(--line-2); border-radius:14px;
    box-shadow:0 28px 90px rgba(0,0,0,.55); overflow:hidden;}
  .gr-cmdk-in{display:flex; align-items:center; gap:12px; padding:16px 18px; border-bottom:1px solid var(--line);}
  .gr-cmdk-in .ph{flex:1; font-size:16px; color:var(--fg);}
  .gr-cmdk-in .cur{width:2px; height:18px; background:var(--coral); animation:gr-blink 1.1s steps(1) infinite;}
  @keyframes gr-blink{50%{opacity:0;}}
  .gr-cmdk-in kbd{font-family:var(--mono); font-size:10.5px; color:var(--subtle); background:var(--surface); border:1px solid var(--line); border-radius:5px; padding:3px 7px;}
  .gr-cmdk-body{padding:8px; max-height:430px; overflow:hidden;}
  .gr-cmdk-grp{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:9px 12px 5px;}
  .gr-cmdk-row{display:grid; grid-template-columns:26px 1fr auto; align-items:center; gap:13px; padding:10px 12px; border-radius:9px; cursor:pointer;}
  .gr-cmdk-row .ic{color:var(--subtle);}
  .gr-cmdk-row .lb{font-size:13.5px; color:var(--fg2); display:flex; align-items:center; gap:9px; white-space:nowrap;}
  .gr-cmdk-row .lb .desc{color:var(--faint); font-size:12px;}
  .gr-cmdk-row .rk{font-family:var(--mono); font-size:10.5px; color:var(--faint); display:flex; gap:4px;}
  .gr-cmdk-row .rk kbd{background:var(--surface); border:1px solid var(--line); border-radius:4px; padding:2px 6px;}
  .gr-cmdk-row.sel{background:var(--raised-3);}
  .gr-cmdk-row.sel .ic{color:var(--coral);} .gr-cmdk-row.sel .lb{color:var(--fg);}
  .gr-cmdk-foot{display:flex; align-items:center; gap:16px; padding:10px 16px; border-top:1px solid var(--line); font-family:var(--mono); font-size:10.5px; color:var(--faint); white-space:nowrap;}
  .gr-cmdk-foot .sp{flex:1;} .gr-cmdk-foot kbd{color:var(--fg2);}

  /* leader chord menu */
  .gr-leader{width:min(460px,92%); margin-top:auto; margin-bottom:46px; background:var(--raised); border:1px solid var(--line-2); border-radius:14px;
    box-shadow:0 28px 90px rgba(0,0,0,.55); overflow:hidden;}
  .gr-leader-head{display:flex; align-items:center; gap:9px; padding:13px 16px; border-bottom:1px solid var(--line); font-family:var(--mono); font-size:11px; color:var(--subtle);}
  .gr-leader-head kbd{color:var(--coral); background:var(--surface); border:1px solid var(--line); border-radius:5px; padding:2px 7px;}
  .gr-leader-head .crumb{color:var(--fg2);}
  .gr-leader-body{padding:8px; display:grid; grid-template-columns:1fr 1fr; gap:2px;}
  .gr-chord{display:flex; align-items:center; gap:11px; padding:9px 11px; border-radius:9px; cursor:pointer;}
  .gr-chord:hover{background:var(--raised-2);}
  .gr-chord .key{width:24px; height:24px; border-radius:6px; display:grid; place-items:center; background:var(--surface); border:1px solid var(--line-2);
    font-family:var(--mono); font-size:12px; color:var(--coral); flex-shrink:0; font-weight:600;}
  .gr-chord .cl{flex:1; font-size:13px; color:var(--fg2);}
  .gr-chord .more{color:var(--faint);}
  .gr-leader-foot{padding:9px 16px; border-top:1px solid var(--line); font-family:var(--mono); font-size:10.5px; color:var(--faint);}

  /* graph */
  .gr-graph{flex:1; position:relative; overflow:hidden; background:
     radial-gradient(900px 540px at 50% 42%, rgba(116,147,232,.06), transparent 70%), var(--bg);}
  .gr-graph svg{position:absolute; inset:0; width:100%; height:100%;}
  .gr-graph-tools{position:absolute; top:14px; left:14px; display:flex; gap:7px; z-index:3;}
  .gr-graph-tools .gt{display:inline-flex; align-items:center; gap:7px; height:30px; padding:0 12px; border-radius:8px; background:var(--raised); border:1px solid var(--line-2); color:var(--fg2); font-size:12px; white-space:nowrap;}
  .gr-graph-legend{position:absolute; bottom:16px; left:14px; display:flex; flex-direction:column; gap:7px; background:var(--surface); border:1px solid var(--line); border-radius:10px; padding:11px 13px; z-index:3;}
  .gr-graph-legend .lg{display:flex; align-items:center; gap:9px; font-size:12px; color:var(--fg2);}
  .gr-graph-legend .lg i{width:9px; height:9px; border-radius:50%;}
  .gr-glabel{font-family:var(--sans); font-size:11px; fill:var(--muted);}
  .gr-glabel.hub{fill:var(--fg); font-size:12.5px; font-weight:600;}
  `;
  const el = document.createElement('style'); el.id = 'grx-styles'; el.textContent = css; document.head.appendChild(el);
})();

// Tabs shown in the top strip. Screens pass which is active; some add their own.
const GR_TABS = [
  { id: 'today', name: 'today' },
  { id: 'ship', name: 'ship the docs refresh' },
  { id: 'inbox', name: 'inbox' },
];

function GrTopBar({ activeTab = 'ship', tabs = GR_TABS }) {
  return (
    <div className="gr-top">
      <div style={{ display: 'flex', alignItems: 'center', minWidth: 0 }}>
        <div className="gr-brand"><MosaicMark size={18} tile="#8693B2" accent="#FF6B5A" /><span className="nm">tesela</span></div>
        <div className="gr-tabs">
          {tabs.map((t) => (
            <div key={t.id} className={'gr-tab' + (t.id === activeTab ? ' active' : '')}>
              {t.id === activeTab && <span className="kdot" />}<span className="nm">{t.name}</span>
            </div>
          ))}
          <div className="gr-ic" style={{ width: 26, height: 26 }}><Icon name="plus" size={15} /></div>
        </div>
      </div>
      <div className="gr-cmd"><Icon name="search" size={15} /><span className="ph">Search or run a command…</span><kbd>⌘K</kbd></div>
      <div className="gr-icons">
        <div className="gr-ic"><Icon name="microphone" size={16} /></div>
        <div className="gr-conn"><i /></div>
        <div className="gr-ic"><Icon name="graph" size={16} /></div>
        <div className="gr-ic"><Icon name="settings" size={16} /></div>
      </div>
    </div>
  );
}

function GrRail({ active }) {
  return (
    <div className="gr-rail">
      <div className="gr-rail-scroll">
        <div className="gr-w">
          <div className="gr-w-head"><Icon name="bolt" size={13} className="ic" /><span className="ti">Quick capture</span></div>
          <div className="gr-w-body"><div className="gr-capture"><span className="pl">Capture a thought…</span><span className="pk">C</span></div></div>
        </div>
        <div className="gr-w">
          <div className="gr-w-head"><Icon name="pin" size={13} className="ic" /><span className="ti">Pinned</span><Icon name="chevronDown" size={13} className="caret" /></div>
          <div className="gr-w-body">
            {WIDGETS.pinned.items.map((it, i) => (
              <div key={i} className={'gr-row' + (active === 'ship' && it.label === 'Tesela v5 launch' ? '' : '')}><Icon name={it.icon} size={14} className="ic" /><span className="lb">{it.label}</span></div>
            ))}
          </div>
        </div>
        <div className="gr-w">
          <div className="gr-w-head"><Icon name="sun" size={13} className="ic" /><span className="ti">Today</span><span className="bd">{WIDGETS.today.badge}</span></div>
          <div className="gr-w-body">
            {WIDGETS.today.items.map((it, i) => (
              <div key={i} className={'gr-row' + (active === 'today' && i === 0 ? ' active' : '')}><span className={'gr-dot ' + it.kind} /><span className="lb">{it.label}</span><span className={'mt' + (it.urgent ? ' urg' : '')}>{it.meta}</span></div>
            ))}
          </div>
        </div>
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
  );
}

function GrStatus({ mode = 'NORMAL', path = 'today / ship-docs-refresh', extra, keys = STATUS.keys, clock = '14:08' }) {
  return (
    <div className="gr-status">
      <span className="mode">{mode}</span>
      <span className="sep">·</span>
      <span>{path}</span>
      {extra && (<><span className="sep">·</span><span>{extra}</span></>)}
      <span className="keys">
        {keys.map((k, i) => (<span key={i}><kbd>{k.k}</kbd> {k.label}</span>))}
        <span className="clk"><Icon name="clock" size={12} color="var(--faint)" />{clock}</span>
      </span>
    </div>
  );
}

// Full shell: top bar + (rail + main) + status, with optional overlay layer.
function GraphiteShell({ activeTab, tabs, railActive, status = {}, overlay, children }) {
  return (
    <div className="gr-root">
      <GrTopBar activeTab={activeTab} tabs={tabs} />
      <div className="gr-body">
        <GrRail active={railActive} />
        <div className="gr-main">{children}</div>
        {overlay}
      </div>
      <GrStatus {...status} />
    </div>
  );
}

Object.assign(window, { GR_TABS, GrTopBar, GrRail, GrStatus, GraphiteShell });

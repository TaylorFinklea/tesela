/* GRAPHITE MOBILE — shared shell: stylesheet, iPhone frame, Apple Watch
   frame, and shared chrome (header, Liquid-Glass tab bar). Aligns 1:1 with
   the desktop Graphite system: same surfaces, lifted-contrast foreground
   ramp, coral accent, Tabler icons, mono metadata / Geist content.

   Real iOS structure (from the Tesela-iOS source): tabs are
   Daily · Agenda · Inbox · Library with Search pinned as a trailing glass
   circle; capture is a sheet from the header; Library reveals a workspace
   widget grid. All reflected here. */

(function () {
  if (document.getElementById('grm-styles')) return;
  const css = `
  .grm-root{
    --bg:#0E1014; --surface:#14171D; --raised:#1A1E26; --raised-2:#20242D; --raised-3:#272C37;
    --line:rgba(255,255,255,.07); --line-2:rgba(255,255,255,.12); --line-3:rgba(255,255,255,.18);
    --fg:#EDEFF2; --fg2:#CBD0D9; --muted:#AAB0BB; --subtle:#8A909C; --faint:#646B78;
    --coral:#FF6B5A; --coral-dim:rgba(255,107,90,.15); --coral-line:rgba(255,107,90,.42);
    --task:#E8697F; --event:#62B8CE; --note:#E4AE66; --project:#7493E8; --person:#AE90E6; --query:#85BC63;
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    position:absolute; inset:0; background:var(--bg); color:var(--fg);
    font-family:var(--sans); -webkit-font-smoothing:antialiased; display:flex; flex-direction:column; overflow:hidden;
  }
  .grm-root *{box-sizing:border-box;}

  /* scroll body */
  .grm-body{flex:1; overflow:hidden; position:relative;}
  .grm-scroll{height:100%; overflow:hidden; padding:0 16px 170px;}

  /* ── large-title header ── */
  .grm-head{padding:6px 18px 12px; border-bottom:1px solid var(--line); position:relative;}
  .grm-head .row{display:flex; align-items:flex-end; gap:10px;}
  .grm-head .tt{flex:1; min-width:0;}
  .grm-head .ttl{font-size:25px; font-weight:650; letter-spacing:-.02em; color:var(--fg); line-height:1.1;}
  .grm-head .sub{font-family:var(--mono); font-size:11px; color:var(--subtle); margin-top:4px;}
  .grm-head .chrome{display:flex; align-items:center; gap:2px; padding-bottom:2px;}
  .grm-cbtn{width:36px; height:36px; display:grid; place-items:center; border-radius:9px; color:var(--subtle);}
  .grm-cbtn:active{background:var(--raised);}
  .grm-syncdot{width:36px; height:36px; display:grid; place-items:center; position:relative;}
  .grm-syncdot i{width:7px; height:7px; border-radius:50%; background:var(--query); box-shadow:0 0 0 3px rgba(133,188,99,.16);}

  /* ── widget card (matches desktop rail widgets) ── */
  .grm-w{background:var(--raised); border:1px solid var(--line); border-radius:14px; overflow:hidden; margin-top:12px;}
  .grm-w-head{display:flex; align-items:center; gap:9px; padding:12px 14px 9px;}
  .grm-w-head .ic{color:var(--subtle);}
  .grm-w-head .ti{flex:1; font-size:11.5px; font-weight:600; letter-spacing:.04em; text-transform:uppercase; color:var(--fg2);}
  .grm-w-head .bd{font-family:var(--mono); font-size:10px; color:var(--subtle); background:var(--bg); border:1px solid var(--line); border-radius:6px; padding:2px 7px;}
  .grm-w-body{padding:0 8px 9px;}

  /* generic rows */
  .grm-row{display:flex; align-items:center; gap:11px; padding:10px 8px; border-radius:9px; min-height:44px;}
  .grm-row:active{background:var(--raised-2);}
  .grm-row + .grm-row{border-top:1px solid var(--line);}
  .grm-row .ic{color:var(--subtle);}
  .grm-row .lb{flex:1; min-width:0; font-size:14.5px; color:var(--fg2); overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .grm-row .mt{font-family:var(--mono); font-size:11px; color:var(--faint); white-space:nowrap;}
  .grm-row .mt.urg{color:var(--coral);}
  .grm-dot{width:7px; height:7px; border-radius:50%; flex-shrink:0;}
  .grm-dot.event{background:var(--event);} .grm-dot.task{background:var(--task);} .grm-dot.note{background:var(--note);} .grm-dot.project{background:var(--project);} .grm-dot.person{background:var(--person);}
  .grm-check{width:20px; height:20px; border-radius:6px; border:1.75px solid var(--subtle); flex-shrink:0;}
  .grm-check.task{border-color:color-mix(in srgb,var(--task) 75%,transparent);}
  .grm-check.done{background:var(--query); border-color:var(--query); position:relative;}

  /* ════ DAILY (journal) ════ */
  .grm-dayhdr{display:flex; align-items:center; gap:11px; padding:18px 6px 8px;}
  .grm-dayhdr .d{font-size:16px; font-weight:600; color:var(--fg); letter-spacing:-.01em;}
  .grm-dayhdr.today .d{color:var(--coral);}
  .grm-dayhdr .dow{font-family:var(--mono); font-size:10.5px; color:var(--faint); text-transform:uppercase; letter-spacing:.06em;}
  .grm-dayhdr .ln{flex:1; height:1px; background:var(--line);}
  .grm-blk{display:flex; align-items:flex-start; gap:11px; padding:9px 8px;}
  .grm-bull{width:7px; height:7px; border-radius:50%; background:var(--faint); margin-top:8px; flex-shrink:0;}
  .grm-blk.task .grm-bull{background:var(--task);} .grm-blk.done .grm-bull{background:var(--query);}
  .grm-blk-body{flex:1; min-width:0;}
  .grm-blk-text{font-size:15.5px; color:var(--fg); line-height:1.45; letter-spacing:-.006em;}
  .grm-blk.done .grm-blk-text{color:var(--subtle); text-decoration:line-through; text-decoration-color:var(--faint);}
  .grm-tagchip{display:inline-flex; align-items:center; height:19px; padding:0 7px; margin-left:6px; border-radius:5px; vertical-align:1px;
    font-family:var(--mono); font-size:10.5px; background:var(--coral-dim); color:var(--coral);}
  .grm-tagchip.proj{background:rgba(116,147,232,.15); color:var(--project);}
  .grm-props{display:flex; flex-wrap:wrap; gap:6px; margin-top:8px;}
  .grm-pchip{display:inline-flex; align-items:center; gap:6px; height:24px; padding:0 9px; border-radius:7px;
    background:var(--surface); border:1px solid var(--line); font-family:var(--mono); font-size:10.5px;}
  .grm-pchip .k{color:var(--faint);} .grm-pchip .v{color:var(--fg2);}
  .grm-pchip.doing .v{color:var(--coral); font-weight:600;} .grm-pchip.high .v{color:var(--task); font-weight:600;}
  .grm-kids{margin:4px 0 2px 18px; padding-left:13px; border-left:1px solid var(--line);}
  .grm-kid{display:flex; align-items:center; gap:9px; padding:5px 6px;}
  .grm-kid .kb{width:5px; height:5px; border-radius:50%; background:var(--faint); flex-shrink:0;}
  .grm-kid .kt{font-size:14px; color:var(--fg2); flex:1; min-width:0;}
  .grm-mention{color:var(--person);} .grm-link{color:var(--project);}

  /* ════ INBOX ════ */
  .grm-chips{display:flex; gap:7px; padding:12px 2px 4px; overflow:hidden;}
  .grm-chip{display:inline-flex; align-items:center; gap:6px; height:30px; padding:0 12px; border-radius:9px; flex-shrink:0;
    background:var(--raised); border:1px solid var(--line); color:var(--fg2); font-size:12.5px;}
  .grm-chip.active{background:var(--coral-dim); border-color:var(--coral-line); color:var(--coral);}
  .grm-chip .n{font-family:var(--mono); font-size:10px; color:var(--faint);} .grm-chip.active .n{color:var(--coral);}
  .grm-icard{display:flex; gap:12px; padding:13px; border-radius:13px; background:var(--surface); border:1px solid var(--line); margin-top:9px;}
  .grm-icard.sel{background:var(--raised); border-color:var(--coral-line);}
  .grm-icard .src{width:32px; height:32px; border-radius:9px; display:grid; place-items:center; background:var(--raised-2); color:var(--subtle); flex-shrink:0;}
  .grm-icard-body{flex:1; min-width:0;}
  .grm-icard .txt{font-size:14.5px; color:var(--fg); line-height:1.42;}
  .grm-icard .meta{display:flex; flex-wrap:wrap; gap:7px; margin-top:9px;}
  .grm-icard .pill{display:inline-flex; align-items:center; gap:5px; padding:3px 8px; border-radius:6px; background:var(--raised-2); color:var(--subtle); font-family:var(--mono); font-size:10px;}
  .grm-icard .acts{display:flex; gap:14px; margin-top:11px; padding-top:11px; border-top:1px solid var(--line);}
  .grm-iact{display:flex; align-items:center; gap:6px; color:var(--subtle); font-size:12px;}
  .grm-iact.go{color:var(--coral); margin-left:auto;}

  /* ════ LIBRARY — workspace widget grid ════ */
  .grm-grid{display:grid; grid-template-columns:1fr 1fr; gap:11px; margin-top:12px;}
  .grm-acard{padding:14px; border-radius:14px; background:var(--surface); border:1px solid var(--line); min-height:118px; display:flex; flex-direction:column;}
  .grm-acard .top{display:flex; align-items:center; justify-content:space-between;}
  .grm-acard .gl{width:38px; height:38px; border-radius:10px; display:grid; place-items:center;}
  .grm-acard .soon{font-family:var(--mono); font-size:9px; color:var(--faint); background:var(--raised-2); padding:2px 6px; border-radius:5px;}
  .grm-acard .at{font-size:15px; font-weight:600; color:var(--fg); margin-top:auto;}
  .grm-acard .ah{font-family:var(--mono); font-size:10.5px; color:var(--faint); margin-top:3px; line-height:1.4;}
  .grm-seg{display:flex; gap:6px; padding:12px 0 2px;}
  .grm-segbtn{flex:1; text-align:center; height:32px; line-height:32px; border-radius:9px; font-size:12.5px; color:var(--subtle); background:var(--raised); border:1px solid var(--line);}
  .grm-segbtn.active{background:var(--raised-2); color:var(--fg); border-color:var(--line-2);}

  /* ════ PAGE / PROJECT (pushed) ════ */
  .grm-pageback{display:flex; align-items:center; gap:2px; padding:8px 8px 8px 6px; color:var(--coral); font-size:15px;}
  .grm-pagehead{padding:6px 18px 14px; border-bottom:1px solid var(--line);}
  .grm-pagehead .ttl{font-size:22px; font-weight:650; letter-spacing:-.02em; color:var(--fg); line-height:1.15;}
  .grm-pagehead .metarow{display:flex; align-items:center; gap:9px; margin-top:9px;}
  .grm-typetag{display:inline-flex; align-items:center; gap:6px; height:22px; padding:0 9px; border-radius:6px; font-family:var(--mono); font-size:10.5px;
    background:var(--raised); border:1px solid var(--line-2); color:var(--project);}
  .grm-typetag .sw{width:6px; height:6px; border-radius:2px; background:var(--project);}
  .grm-pagehead .when{font-family:var(--mono); font-size:10.5px; color:var(--faint);}
  .grm-refhdr{font-family:var(--mono); font-size:9.5px; letter-spacing:.10em; text-transform:uppercase; color:var(--faint); padding:18px 8px 8px;}
  .grm-refcard{padding:12px 13px; border-radius:12px; background:var(--surface); border:1px solid var(--line); margin-top:8px;}
  .grm-refcard .src{display:flex; align-items:center; gap:8px; font-family:var(--mono); font-size:10.5px; color:var(--fg2); margin-bottom:6px;}
  .grm-refcard .snip{font-size:13px; color:var(--muted); line-height:1.5;}
  .grm-refcard .snip em{font-style:normal; color:var(--fg); background:var(--coral-dim); padding:0 3px; border-radius:3px;}

  /* ════ AGENDA (day) ════ */
  .grm-agstrip{display:flex; gap:6px; padding:12px 0 6px;}
  .grm-agday{flex:1; display:flex; flex-direction:column; align-items:center; gap:3px; padding:7px 0; border-radius:11px; background:var(--raised); border:1px solid transparent;}
  .grm-agday .dw{font-family:var(--mono); font-size:9.5px; text-transform:uppercase; color:var(--faint);}
  .grm-agday .dn{font-size:15px; font-weight:600; color:var(--fg2);}
  .grm-agday.today{background:var(--coral); border-color:transparent;} .grm-agday.today .dw{color:rgba(16,17,15,.7);} .grm-agday.today .dn{color:#10110f;}
  .grm-agtl{position:relative; margin-top:8px; padding-left:52px;}
  .grm-agtime{position:absolute; left:0; font-family:var(--mono); font-size:9.5px; color:var(--faint); width:46px; text-align:right;}
  .grm-agrow{position:relative; min-height:58px; border-top:1px solid var(--line); padding:7px 0;}
  .grm-agev{border-radius:9px; padding:8px 10px; margin-bottom:6px; border:1px solid transparent;}
  .grm-agev .et{font-size:13.5px; font-weight:550; line-height:1.25;}
  .grm-agev .em{font-family:var(--mono); font-size:10px; opacity:.85; margin-top:2px;}
  .grm-agev.event{background:rgba(98,184,206,.15); border-color:rgba(98,184,206,.32); color:var(--event);}
  .grm-agev.task{background:rgba(232,105,127,.15); border-color:rgba(232,105,127,.32); color:var(--task);}
  .grm-agev.focus-ev{background:var(--coral-dim); border-color:var(--coral-line); color:var(--coral);}

  /* ── floating bottom chrome: capture bar above Liquid-Glass tab bar ── */
  .grm-botwrap{position:absolute; left:0; right:0; bottom:0; z-index:30; pointer-events:none;
    display:flex; flex-direction:column; gap:10px; padding:0 13px 22px;}
  .grm-glass{position:relative; overflow:hidden; background:rgba(27,31,40,.93); border:1px solid rgba(255,255,255,.11);
    box-shadow:0 10px 34px rgba(0,0,0,.46), inset 0 1px 0 rgba(255,255,255,.09);}
  /* capture bar (the iOS tabViewBottomAccessory) */
  .grm-capbar{pointer-events:auto; height:56px; border-radius:28px; display:flex; align-items:center; gap:11px; padding:0 6px 0 15px;}
  .grm-capbar .cb-plus{color:var(--muted); flex-shrink:0;}
  .grm-capbar .cb-target{width:38px; height:38px; border-radius:12px; display:grid; place-items:center; flex-shrink:0;
    background:rgba(116,147,232,.18); border:1px solid rgba(116,147,232,.26); color:var(--note);}
  .grm-capbar .cb-ph{flex:1; min-width:0; color:var(--subtle); font-size:16px; letter-spacing:-.01em;}
  .grm-capbar .cb-mic{width:44px; height:44px; display:grid; place-items:center; color:var(--muted); flex-shrink:0;}
  /* tab row */
  .grm-botrow{display:flex; gap:10px; align-items:stretch;}
  .grm-tabbar{pointer-events:auto; flex:1; height:60px; border-radius:30px; display:flex; align-items:center; justify-content:space-between; padding:6px;}
  .grm-tab{flex:1; height:100%; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:3px; color:var(--muted); border-radius:22px;}
  .grm-tab.active{color:var(--coral); background:rgba(255,255,255,.07);}
  .grm-tab .tl{font-size:10px; font-weight:600; letter-spacing:-.01em;}
  .grm-searchbtn{pointer-events:auto; width:60px; height:60px; border-radius:30px; display:grid; place-items:center; color:var(--fg2); flex-shrink:0;}

  /* ── capture sheet ── */
  .grm-sheetdim{position:absolute; inset:0; z-index:40; background:rgba(8,9,12,.5); backdrop-filter:blur(2px);}
  .grm-sheet{position:absolute; left:0; right:0; bottom:0; z-index:41; background:var(--surface); border-top:1px solid var(--line-2);
    border-radius:26px 26px 0 0; box-shadow:0 -16px 50px rgba(0,0,0,.5); padding:10px 18px 0; display:flex; flex-direction:column;}
  .grm-grab{width:38px; height:5px; border-radius:3px; background:var(--line-3); margin:0 auto 12px;}
  .grm-sheet-head{display:flex; align-items:center; gap:10px; margin-bottom:14px;}
  .grm-sheet-head .h{font-size:16px; font-weight:600; color:var(--fg);}
  .grm-sheet-head .to{font-family:var(--mono); font-size:11px; color:var(--subtle); margin-left:auto; display:flex; align-items:center; gap:6px;}
  .grm-compose{font-size:16px; color:var(--fg); line-height:1.5; min-height:54px;}
  .grm-compose .ph{color:var(--faint);}
  .grm-compose .parsed-tag{color:var(--coral);} .grm-compose .parsed-date{color:var(--event);} .grm-compose .parsed-at{color:var(--person);}
  .grm-cur{display:inline-block; width:2px; height:18px; background:var(--coral); vertical-align:-3px; animation:grm-blink 1.1s steps(1) infinite;}
  @keyframes grm-blink{50%{opacity:0;}}
  .grm-wave{display:flex; align-items:center; gap:3px; height:40px; margin:10px 0 6px;}
  .grm-wave i{flex:1; background:var(--coral); border-radius:2px; opacity:.85;}
  .grm-parsed{display:flex; flex-wrap:wrap; gap:7px; padding:12px 0;}
  .grm-sheet-foot{display:flex; align-items:center; gap:10px; padding:12px 0 16px; border-top:1px solid var(--line); margin-top:6px;}
  .grm-recbtn{width:50px; height:50px; border-radius:25px; display:grid; place-items:center; background:var(--coral); color:#10110f; flex-shrink:0; box-shadow:0 0 0 6px var(--coral-dim);}
  .grm-recmeta{flex:1; min-width:0;} .grm-recmeta .l1{font-size:13.5px; color:var(--fg);} .grm-recmeta .l2{font-family:var(--mono); font-size:11px; color:var(--subtle); margin-top:2px;}
  .grm-sendbtn{height:40px; padding:0 18px; border-radius:12px; background:var(--raised-2); border:1px solid var(--line-2); color:var(--fg); font-size:14px; font-weight:600; display:flex; align-items:center; gap:7px;}

  /* ════════════ APPLE WATCH ════════════ */
  .grw-root{
    --bg:#000; --surface:#15181E; --raised:#1C2029; --raised-2:#252A35;
    --line:rgba(255,255,255,.09); --line-2:rgba(255,255,255,.15);
    --fg:#F4F5F7; --fg2:#C7CCD5; --muted:#9298A4; --faint:#5E646F;
    --coral:#FF6B5A; --coral-dim:rgba(255,107,90,.18);
    --task:#F0758C; --event:#62B8CE; --note:#E4AE66; --project:#7493E8; --query:#8ECB6A;
    --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;
    position:absolute; inset:0; background:#000; color:var(--fg); font-family:var(--sans); -webkit-font-smoothing:antialiased;
    display:flex; flex-direction:column; overflow:hidden;
  }
  .grw-root *{box-sizing:border-box;}
  .grw-time{display:flex; align-items:center; justify-content:space-between; padding:9px 16px 5px; flex-shrink:0;}
  .grw-time .t{font-size:15px; font-weight:600; color:var(--fg); font-variant-numeric:tabular-nums;}
  .grw-time .l{font-size:13px; font-weight:600; color:var(--coral); display:flex; align-items:center; gap:5px;}
  .grw-scroll{flex:1; overflow:hidden; padding:0 12px 16px;}
  .grw-title{font-size:13px; font-weight:600; color:var(--fg2); padding:4px 4px 8px; letter-spacing:-.01em;}
  .grw-card{background:var(--surface); border-radius:16px; padding:12px 13px; margin-bottom:8px;}
  .grw-card.coral{background:linear-gradient(150deg, rgba(255,107,90,.22), rgba(255,107,90,.10)); border:1px solid var(--coral-line);}
  .grw-eyebrow{display:flex; align-items:center; gap:6px; font-family:var(--mono); font-size:9.5px; letter-spacing:.06em; text-transform:uppercase; color:var(--faint); margin-bottom:7px;}
  .grw-eyebrow.coral{color:var(--coral);}
  .grw-evt{font-size:15px; font-weight:600; color:var(--fg); letter-spacing:-.01em; line-height:1.2;}
  .grw-evm{font-family:var(--mono); font-size:11px; color:var(--muted); margin-top:4px;}
  .grw-trow{display:flex; align-items:center; gap:10px; padding:10px 4px;}
  .grw-trow + .grw-trow{border-top:1px solid var(--line);}
  .grw-tcheck{width:22px; height:22px; border-radius:7px; border:2px solid var(--task); flex-shrink:0;}
  .grw-tcheck.done{background:var(--query); border-color:var(--query);}
  .grw-tlb{flex:1; min-width:0; font-size:14px; color:var(--fg); overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .grw-tlb.done{color:var(--faint); text-decoration:line-through;}
  .grw-bigbtn{display:flex; align-items:center; justify-content:center; gap:9px; height:50px; border-radius:25px; background:var(--coral); color:#000; font-size:15px; font-weight:650;}
  .grw-ring{position:relative; width:54px; height:54px; flex-shrink:0;}
  .grw-ringlbl{position:absolute; inset:0; display:flex; flex-direction:column; align-items:center; justify-content:center;}
  .grw-ringlbl .n{font-size:17px; font-weight:700; color:var(--fg); line-height:1;}
  .grw-ringlbl .u{font-family:var(--mono); font-size:7.5px; color:var(--faint); text-transform:uppercase;}
  .grw-listrow{display:flex; align-items:center; gap:10px; padding:11px 12px; background:var(--surface); border-radius:13px; margin-bottom:7px;}
  .grw-listrow .dot{width:8px; height:8px; border-radius:50%; flex-shrink:0;}
  .grw-listrow .lb{flex:1; min-width:0; font-size:14px; color:var(--fg); overflow:hidden; text-overflow:ellipsis; white-space:nowrap;}
  .grw-listrow .mt{font-family:var(--mono); font-size:10px; color:var(--muted);}
  .grw-wave{display:flex; align-items:center; justify-content:center; gap:3px; height:46px; margin:8px 0;}
  .grw-wave i{width:3.5px; background:var(--coral); border-radius:2px;}
  .grw-cap-status{text-align:center; font-size:15px; color:var(--fg); font-weight:600; margin-top:4px;}
  .grw-cap-hint{text-align:center; font-family:var(--mono); font-size:10.5px; color:var(--muted); margin-top:4px;}
  /* capture field + native input chooser */
  .grw-capfield{background:var(--surface); border:1px solid var(--line-2); border-radius:14px; padding:11px 13px; min-height:54px; display:flex; align-items:center;}
  .grw-capfield .ph{font-size:14px; color:var(--faint); flex:1;}
  .grw-capfield .cur{width:2px; height:17px; background:var(--coral); border-radius:1px; animation:grm-blink 1.1s steps(1) infinite;}
  .grw-inputs{display:flex; gap:8px; margin-top:10px;}
  .grw-inbtn{flex:1; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:5px; height:62px; border-radius:16px;
    background:var(--surface); color:var(--fg2);}
  .grw-inbtn .il{font-size:10px; font-weight:600;}
  .grw-inbtn.primary{background:var(--coral); color:#000;}
  .grw-inbtn.primary .il{color:#000;}
  .grw-pager{display:flex; gap:5px; justify-content:center; padding:6px 0 2px;}
  .grw-pager i{width:6px; height:6px; border-radius:50%; background:var(--faint);} .grw-pager i.on{background:var(--fg);}

  /* watch complication faces */
  .grw-face{border-radius:22px; overflow:hidden; position:relative; background:radial-gradient(120% 120% at 50% 0%, #0c0e12, #000 70%);}
  `;
  const el = document.createElement('style'); el.id = 'grm-styles'; el.textContent = css; document.head.appendChild(el);
})();

// ── iPhone frame: bezel + dynamic island + dark status bar + home indicator ──
function GrmPhone({ children, time = '9:41', w = 390, h = 844 }) {
  return (
    <div style={{
      width: w, height: h, borderRadius: 52, position: 'relative', background: '#000',
      boxShadow: '0 40px 90px rgba(0,0,0,.45), 0 0 0 12px #1b1c1f, 0 0 0 13px #2c2d30',
      overflow: 'hidden', fontFamily: "'Geist', system-ui, sans-serif",
    }}>
      {/* dynamic island */}
      <div style={{ position: 'absolute', top: 12, left: '50%', transform: 'translateX(-50%)', width: 118, height: 35, borderRadius: 22, background: '#000', zIndex: 60 }} />
      {/* status bar */}
      <div style={{ position: 'absolute', top: 0, left: 0, right: 0, zIndex: 55, display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '17px 32px 0' }}>
        <span style={{ fontWeight: 600, fontSize: 15, color: '#fff', letterSpacing: '.02em' }}>{time}</span>
        <span style={{ display: 'flex', gap: 7, alignItems: 'center' }}>
          <svg width="18" height="11" viewBox="0 0 18 11"><rect x="0" y="7" width="3" height="4" rx=".6" fill="#fff"/><rect x="4.5" y="4.5" width="3" height="6.5" rx=".6" fill="#fff"/><rect x="9" y="2" width="3" height="9" rx=".6" fill="#fff"/><rect x="13.5" y="0" width="3" height="11" rx=".6" fill="#fff"/></svg>
          <svg width="16" height="11" viewBox="0 0 16 11"><path d="M8 3C10 3 11.8 3.8 13.2 5.1L14.2 4C12.5 2.4 10.4 1.4 8 1.4C5.6 1.4 3.5 2.4 1.8 4L2.8 5.1C4.2 3.8 6 3 8 3Z" fill="#fff"/><path d="M8 6.2C9.2 6.2 10.3 6.7 11.1 7.5L12 6.6C10.9 5.5 9.5 4.8 8 4.8C6.5 4.8 5.1 5.5 4 6.6L4.9 7.5C5.7 6.7 6.8 6.2 8 6.2Z" fill="#fff"/><circle cx="8" cy="9.6" r="1.3" fill="#fff"/></svg>
          <svg width="25" height="12" viewBox="0 0 25 12"><rect x="0.5" y="0.5" width="21" height="11" rx="3" stroke="#fff" strokeOpacity=".4" fill="none"/><rect x="2" y="2" width="18" height="8" rx="1.8" fill="#fff"/><path d="M23 4v4c.7-.3 1.2-1 1.2-2S23.7 4.3 23 4Z" fill="#fff" fillOpacity=".5"/></svg>
        </span>
      </div>
      {/* screen */}
      <div className="grm-root" style={{ paddingTop: 44 }}>{children}</div>
      {/* home indicator */}
      <div style={{ position: 'absolute', bottom: 8, left: '50%', transform: 'translateX(-50%)', width: 134, height: 5, borderRadius: 3, background: 'rgba(255,255,255,.5)', zIndex: 62 }} />
    </div>
  );
}

// ── Apple Watch frame: cushion bezel + digital crown + side button ──
function GrmWatch({ children, w = 198, h = 242 }) {
  const bezel = 14;
  return (
    <div style={{ position: 'relative', width: w + bezel * 2 + 8, height: h + bezel * 2, fontFamily: "'Geist', system-ui, sans-serif" }}>
      {/* digital crown */}
      <div style={{ position: 'absolute', right: 0, top: h * 0.30, width: 11, height: 30, borderRadius: 4, background: 'linear-gradient(90deg,#3a3b3e,#202124)', boxShadow: 'inset 0 0 2px rgba(255,255,255,.2)' }} />
      <div style={{ position: 'absolute', right: 2, top: h * 0.30 + 4, width: 7, height: 22, borderRadius: 3, background: 'repeating-linear-gradient(0deg,#4a4b4e,#4a4b4e 1px,#2a2b2e 1px,#2a2b2e 2.5px)' }} />
      {/* side button */}
      <div style={{ position: 'absolute', right: 1, top: h * 0.30 + 44, width: 8, height: 38, borderRadius: 4, background: 'linear-gradient(90deg,#3a3b3e,#202124)' }} />
      {/* case */}
      <div style={{
        position: 'absolute', left: 0, top: 0, width: w + bezel * 2, height: h + bezel * 2,
        borderRadius: 56, background: 'linear-gradient(155deg,#2a2b2f,#16171a)',
        boxShadow: '0 30px 70px rgba(0,0,0,.5), inset 0 1px 1px rgba(255,255,255,.14)', padding: bezel,
      }}>
        {/* screen */}
        <div className="grw-root grw-face" style={{ width: w, height: h, borderRadius: 44, position: 'relative' }}>
          {children}
        </div>
      </div>
    </div>
  );
}

// ── shared phone chrome ──
function GrmHeader({ title, sub, calendar }) {
  return (
    <div className="grm-head">
      <div className="row">
        <div className="tt"><div className="ttl">{title}</div>{sub && <div className="sub">{sub}</div>}</div>
        <div className="chrome">
          {calendar && <div className="grm-cbtn"><Icon name="calendar" size={20} /></div>}
          <div className="grm-cbtn"><Icon name="settings" size={20} /></div>
          <div className="grm-syncdot"><i /></div>
        </div>
      </div>
    </div>
  );
}

const GRM_TABS = [
  { id: 'daily', name: 'Daily', icon: 'calendar' },
  { id: 'agenda', name: 'Agenda', icon: 'list' },
  { id: 'inbox', name: 'Inbox', icon: 'inbox' },
  { id: 'library', name: 'Library', icon: 'fileText' },
];

// Bottom chrome: the iOS capture accessory bar floating above the
// Liquid-Glass tab bar (Daily · Agenda · Inbox · Library) + Search circle.
// Tapping the bar / mic opens the capture sheet.
function GrmBottomBar({ active }) {
  return (
    <div className="grm-botwrap">
      <div className="grm-glass grm-capbar">
        <Icon name="plus" size={22} className="cb-plus" />
        <div className="cb-target"><Icon name="calendar" size={19} /></div>
        <span className="cb-ph">Capture…</span>
        <div className="cb-mic"><Icon name="microphone" size={21} /></div>
      </div>
      <div className="grm-botrow">
        <div className="grm-glass grm-tabbar">
          {GRM_TABS.map((t) => (
            <div key={t.id} className={'grm-tab' + (t.id === active ? ' active' : '')}>
              <Icon name={t.icon} size={22} stroke={t.id === active ? 2 : 1.75} />
              <span className="tl">{t.name}</span>
            </div>
          ))}
        </div>
        <div className="grm-glass grm-searchbtn"><Icon name="search" size={22} /></div>
      </div>
    </div>
  );
}

Object.assign(window, { GrmPhone, GrmWatch, GrmHeader, GrmBottomBar, GRM_TABS });

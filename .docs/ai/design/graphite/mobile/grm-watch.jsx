/* GRAPHITE MOBILE — Apple Watch screens. Net-new (no watch target exists
   yet), designed to watchOS conventions + the Graphite system. The watch's
   job: glance + capture. Capture-to-Inbox via voice is the signature flow,
   mirroring the phone's on-device Parakeet transcription. */

// ── Today glance ────────────────────────────────────────────────────────
function WatchToday() {
  return (
    <GrmWatch>
      <div className="grw-time"><span className="t">9:41</span><span className="l"><MosaicMark size={13} tile="#8693B2" accent="#FF6B5A" />tesela</span></div>
      <div className="grw-scroll">
        <div className="grw-card coral" style={{ padding: '11px 12px', marginBottom: 7 }}>
          <div className="grw-eyebrow coral"><Icon name="calendar" size={11} color="var(--coral)" />Next · 9:30</div>
          <div className="grw-evt">Standup</div>
          <div className="grw-evm">in 12 min · 30 min</div>
        </div>
        <div className="grw-card" style={{ display: 'flex', alignItems: 'center', gap: 11, padding: '9px 12px', marginBottom: 9 }}>
          <div className="grw-ring" style={{ width: 40, height: 40 }}>
            <svg width="40" height="40" viewBox="0 0 40 40">
              <circle cx="20" cy="20" r="16" fill="none" stroke="rgba(255,255,255,.10)" strokeWidth="5" />
              <circle cx="20" cy="20" r="16" fill="none" stroke="var(--task)" strokeWidth="5" strokeLinecap="round"
                strokeDasharray={2 * Math.PI * 16} strokeDashoffset={2 * Math.PI * 16 * (1 - 0.375)} transform="rotate(-90 20 20)" />
            </svg>
            <div className="grw-ringlbl"><span className="n" style={{ fontSize: 13 }}>3/8</span></div>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="grw-evt" style={{ fontSize: 14 }}>5 tasks left</div>
            <div className="grw-evm">2 high priority</div>
          </div>
        </div>
        <div className="grw-bigbtn"><Icon name="microphone" size={20} />Capture</div>
      </div>
    </GrmWatch>
  );
}

// ── Voice capture (signature) — defers to native watchOS input ──────────
// Parakeet (600M params, needs ~2GB RAM + GPU) can't run on the Watch, so
// capture here is a simple field that hands off to the system's own
// Dictation / Scribble / keyboard; the text then syncs to the mosaic.
function WatchCapture() {
  return (
    <GrmWatch>
      <div className="grw-time"><span className="t">9:41</span><span className="l"><Icon name="inbox" size={13} color="var(--coral)" />Inbox</span></div>
      <div className="grw-scroll" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'center' }}>
        <div className="grw-eyebrow coral"><Icon name="bolt" size={11} color="var(--coral)" />New capture</div>
        <div className="grw-capfield"><span className="ph">What's on your mind?</span><span className="cur" /></div>
        <div className="grw-inputs">
          <div className="grw-inbtn primary"><Icon name="microphone" size={21} color="#000" /><span className="il">Dictate</span></div>
          <div className="grw-inbtn"><Icon name="pencil" size={21} /><span className="il">Scribble</span></div>
          <div className="grw-inbtn"><Icon name="keyboard" size={21} /><span className="il">Keyboard</span></div>
        </div>
        <div className="grw-cap-hint">Transcribed by Apple Watch · synced to your mosaic</div>
      </div>
    </GrmWatch>
  );
}

// ── Tasks list ──────────────────────────────────────────────────────────
const GRW_TASKS = [
  { label: 'Ship the docs refresh', done: false, urg: true },
  { label: 'Write getting-started guide', done: false },
  { label: 'Fix the login bug', done: false },
  { label: 'Call Priya re: launch copy', done: false },
  { label: 'Review empty states', done: true },
];

function WatchTasks() {
  return (
    <GrmWatch>
      <div className="grw-time"><span className="t">9:41</span><span className="l" style={{ color: 'var(--task)', fontSize: 12 }}>5 left</span></div>
      <div className="grw-scroll">
        <div className="grw-title">Tasks · Today</div>
        {GRW_TASKS.map((t, i) => (
          <div key={i} className="grw-trow">
            <span className={'grw-tcheck' + (t.done ? ' done' : '')}>{t.done && <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#000" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12l4 4l10 -10" /></svg>}</span>
            <span className={'grw-tlb' + (t.done ? ' done' : '')}>{t.label}</span>
            {t.urg && <Icon name="flame" size={15} color="var(--task)" />}
          </div>
        ))}
      </div>
    </GrmWatch>
  );
}

// ── Agenda (up next list) ───────────────────────────────────────────────
const GRW_AGENDA = [
  { time: '9:30', label: 'Standup', kind: 'event', tint: 'var(--event)' },
  { time: '14:00', label: 'Docs review w/ Mara', kind: 'event', tint: 'var(--event)' },
  { time: '16:00', label: 'Ship docs · due', kind: 'task', tint: 'var(--task)' },
  { time: 'Fri', label: 'v5 Retro', kind: 'event', tint: 'var(--event)' },
];

function WatchAgenda() {
  return (
    <GrmWatch>
      <div className="grw-time"><span className="t">9:41</span><span className="l" style={{ fontSize: 12, color: 'var(--event)' }}>Thu 10</span></div>
      <div className="grw-scroll">
        <div className="grw-title">Up next</div>
        {GRW_AGENDA.map((e, i) => (
          <div key={i} className="grw-listrow">
            <span className="dot" style={{ background: e.tint }} />
            <span className="lb">{e.label}</span>
            <span className="mt">{e.time}</span>
          </div>
        ))}
      </div>
    </GrmWatch>
  );
}

// ── Complications row (watch-face surfaces) ─────────────────────────────
function WatchComplications() {
  // Corner, circular, and rectangular complication treatments on a face.
  return (
    <div style={{ display: 'flex', gap: 26, alignItems: 'center' }}>
      {/* Circular (modular) */}
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 }}>
        <div style={{ width: 88, height: 88, borderRadius: 24, background: 'radial-gradient(120% 120% at 50% 0%, #0c0e12, #000 70%)', display: 'grid', placeItems: 'center', position: 'relative' }}>
          <svg width="78" height="78" viewBox="0 0 78 78" style={{ position: 'absolute' }}>
            <circle cx="39" cy="39" r="33" fill="none" stroke="rgba(255,255,255,.10)" strokeWidth="5" />
            <circle cx="39" cy="39" r="33" fill="none" stroke="#FF6B5A" strokeWidth="5" strokeLinecap="round" strokeDasharray={2 * Math.PI * 33} strokeDashoffset={2 * Math.PI * 33 * (1 - 0.375)} transform="rotate(-90 39 39)" />
          </svg>
          <div style={{ textAlign: 'center', fontFamily: "'Geist',sans-serif" }}>
            <MosaicMark size={15} tile="#8693B2" accent="#FF6B5A" />
            <div style={{ fontSize: 15, fontWeight: 700, color: '#F4F5F7', marginTop: 2 }}>3/8</div>
          </div>
        </div>
        <span style={{ fontFamily: "'JetBrains Mono',monospace", fontSize: 9.5, color: '#646B78' }}>Circular</span>
      </div>

      {/* Rectangular (modular large) */}
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 }}>
        <div style={{ width: 188, height: 88, borderRadius: 22, background: 'radial-gradient(120% 140% at 0% 0%, #0c0e12, #000 70%)', padding: '13px 15px', fontFamily: "'Geist',sans-serif", display: 'flex', flexDirection: 'column', justifyContent: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontFamily: "'JetBrains Mono',monospace", fontSize: 9.5, letterSpacing: '.06em', textTransform: 'uppercase', color: '#FF6B5A', marginBottom: 6 }}>
            <MosaicMark size={11} tile="#8693B2" accent="#FF6B5A" />Tesela · Next
          </div>
          <div style={{ fontSize: 15, fontWeight: 600, color: '#F4F5F7', letterSpacing: '-.01em' }}>Standup · 9:30</div>
          <div style={{ fontFamily: "'JetBrains Mono',monospace", fontSize: 10.5, color: '#9298A4', marginTop: 3 }}>then 5 tasks left today</div>
        </div>
        <span style={{ fontFamily: "'JetBrains Mono',monospace", fontSize: 9.5, color: '#646B78' }}>Rectangular</span>
      </div>

      {/* Corner */}
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 }}>
        <div style={{ width: 88, height: 88, borderRadius: 24, background: 'radial-gradient(120% 120% at 100% 100%, #0c0e12, #000 70%)', position: 'relative' }}>
          <div style={{ position: 'absolute', left: 11, top: 11, display: 'flex', alignItems: 'center', gap: 5, fontFamily: "'JetBrains Mono',monospace", fontSize: 9, color: '#FF6B5A' }}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#FF6B5A" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M4 4m0 2a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2z" /><path d="M4 13h3l3 3h4l3 -3h3" /></svg>
            12
          </div>
          <div style={{ position: 'absolute', right: 11, bottom: 10, fontFamily: "'Geist',sans-serif", textAlign: 'right' }}>
            <div style={{ fontSize: 13, fontWeight: 700, color: '#F4F5F7' }}>Inbox</div>
          </div>
          <svg width="88" height="88" viewBox="0 0 88 88" style={{ position: 'absolute', inset: 0 }}>
            <path d="M 80 78 A 70 70 0 0 0 78 62" fill="none" stroke="#FF6B5A" strokeWidth="4" strokeLinecap="round" />
          </svg>
        </div>
        <span style={{ fontFamily: "'JetBrains Mono',monospace", fontSize: 9.5, color: '#646B78' }}>Corner</span>
      </div>
    </div>
  );
}

Object.assign(window, { WatchToday, WatchCapture, WatchTasks, WatchAgenda, WatchComplications });

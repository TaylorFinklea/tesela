/* Tabler-style stroke icons (24×24, stroke currentColor, no fill).
   Taylor always uses Tabler — these are drawn to match that grammar:
   2px stroke, round caps/joins, 24 grid. Exported as <Icon name size stroke/>. */

const TABLER_PATHS = {
  search: 'M10 10m-7 0a7 7 0 1 0 14 0a7 7 0 1 0 -14 0 M21 21l-6 -6',
  command: 'M7 9a2 2 0 1 1 2 -2v10a2 2 0 1 1 -2 -2h10a2 2 0 1 1 -2 2v-10a2 2 0 1 1 2 2h-10',
  microphone: 'M9 2m0 3a3 3 0 0 1 3 -3h0a3 3 0 0 1 3 3v5a3 3 0 0 1 -3 3h0a3 3 0 0 1 -3 -3z M5 10a7 7 0 0 0 14 0 M8 21l8 0 M12 17l0 4',
  settings: 'M10.325 4.317c.426 -1.756 2.924 -1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543 -.94 3.31 .826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756 .426 1.756 2.924 0 3.35a1.724 1.724 0 0 0 -1.066 2.573c.94 1.543 -.826 3.31 -2.37 2.37a1.724 1.724 0 0 0 -2.572 1.065c-.426 1.756 -2.924 1.756 -3.35 0a1.724 1.724 0 0 0 -2.573 -1.066c-1.543 .94 -3.31 -.826 -2.37 -2.37a1.724 1.724 0 0 0 -1.065 -2.572c-1.756 -.426 -1.756 -2.924 0 -3.35a1.724 1.724 0 0 0 1.066 -2.573c-.94 -1.543 .826 -3.31 2.37 -2.37c1 .608 2.296 .07 2.572 -1.065z M9 12a3 3 0 1 0 6 0a3 3 0 0 0 -6 0',
  adjustments: 'M4 8l4 0 M6 4l0 4 M6 12l0 8 M10 14l4 0 M12 10l0 4 M12 18l0 2 M16 16l4 0 M18 12l0 4 M18 20l0 0 M18 4l0 8',
  help: 'M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0 M12 16v.01 M12 13a2 2 0 0 0 .914 -3.782a1.98 1.98 0 0 0 -2.414 .483',
  plus: 'M12 5l0 14 M5 12l14 0',
  pin: 'M9 4v6l-2 4v2h10v-2l-2 -4v-6 M12 16l0 5 M8 4l8 0',
  calendar: 'M4 7a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2v-12z M16 3v4 M8 3v4 M4 11h16 M11 15h1 M12 15v3',
  square: 'M3 5a2 2 0 0 1 2 -2h14a2 2 0 0 1 2 2v14a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-14z',
  squareCheck: 'M3 5a2 2 0 0 1 2 -2h14a2 2 0 0 1 2 2v14a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-14z M9 12l2 2l4 -4',
  circle: 'M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0',
  circleDot: 'M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0 M12 12m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0',
  hash: 'M5 9l14 0 M5 15l14 0 M11 4l-4 16 M17 4l-4 16',
  link: 'M9 15l6 -6 M11 6l.463 -.536a5 5 0 0 1 7.071 7.072l-.534 .464 M13 18l-.397 .534a5.068 5.068 0 0 1 -7.127 0a4.972 4.972 0 0 1 0 -7.071l.524 -.463',
  user: 'M8 7a4 4 0 1 0 8 0a4 4 0 0 0 -8 0 M6 21v-2a4 4 0 0 1 4 -4h4a4 4 0 0 1 4 4v2',
  folder: 'M5 4h4l3 3h7a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-11a2 2 0 0 1 2 -2',
  clock: 'M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0 M12 7v5l3 3',
  inbox: 'M4 4m0 2a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2z M4 13h3l3 3h4l3 -3h3',
  bolt: 'M13 3l0 7l6 0l-8 11l0 -7l-6 0l8 -11',
  flame: 'M12 12c2 -2.96 0 -7 -1 -8c0 3.038 -1.773 4.741 -3 6c-1.226 1.26 -2 3.24 -2 5a6 6 0 1 0 12 0c0 -1.532 -1.056 -3.94 -2 -5c-1.786 3 -2.791 3 -4 2z',
  chevronRight: 'M9 6l6 6l-6 6',
  chevronDown: 'M6 9l6 6l6 -6',
  layoutSidebar: 'M4 4m0 2a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2z M9 4v16',
  x: 'M18 6l-12 12 M6 6l12 12',
  list: 'M9 6l11 0 M9 12l11 0 M9 18l11 0 M5 6l0 .01 M5 12l0 .01 M5 18l0 .01',
  focus: 'M12 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M12 12m-5 0a5 5 0 1 0 10 0a5 5 0 1 0 -10 0 M12 3l0 2 M3 12l2 0 M12 19l0 2 M19 12l2 0',
  fileText: 'M14 3v4a1 1 0 0 0 1 1h4 M17 21h-10a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2h7l5 5v11a2 2 0 0 1 -2 2z M9 9l1 0 M9 13l6 0 M9 17l6 0',
  graph: 'M3 17a2 2 0 1 0 4 0a2 2 0 0 0 -4 0 M17 5a2 2 0 1 0 4 0a2 2 0 0 0 -4 0 M17 17a2 2 0 1 0 4 0a2 2 0 0 0 -4 0 M5 15v-6a3 3 0 0 1 3 -3h7 M14 8l3 -3l-3 -3 M19 15v-3',
  dotsVertical: 'M12 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M12 5m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M12 19m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0',
  dots: 'M5 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M12 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M19 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0',
  arrowLeft: 'M5 12l14 0 M5 12l6 6 M5 12l6 -6',
  arrowRight: 'M5 12l14 0 M13 18l6 -6 M13 6l6 6',
  cornerDownRight: 'M6 6v6a3 3 0 0 0 3 3h10 M15 11l4 4l-4 4',
  sparkles: 'M16 18a2 2 0 0 1 2 2a2 2 0 0 1 2 -2a2 2 0 0 1 -2 -2a2 2 0 0 1 -2 2zm0 -12a2 2 0 0 1 2 2a2 2 0 0 1 2 -2a2 2 0 0 1 -2 -2a2 2 0 0 1 -2 2zm-7 12a6 6 0 0 1 6 -6a6 6 0 0 1 -6 -6a6 6 0 0 1 -6 6a6 6 0 0 1 6 6z',
  flag: 'M5 5a5 5 0 0 1 7 0a5 5 0 0 0 7 0v9a5 5 0 0 1 -7 0a5 5 0 0 0 -7 0v-9z M5 21v-7',
  star: 'M12 17.75l-6.172 3.245l1.179 -6.873l-5 -4.867l6.9 -1l3.086 -6.253l3.086 6.253l6.9 1l-5 4.867l1.179 6.873z',
  layoutGrid: 'M4 4m0 1a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v4a1 1 0 0 1 -1 1h-4a1 1 0 0 1 -1 -1z M14 4m0 1a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v4a1 1 0 0 1 -1 1h-4a1 1 0 0 1 -1 -1z M4 14m0 1a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v4a1 1 0 0 1 -1 1h-4a1 1 0 0 1 -1 -1z M14 14m0 1a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v4a1 1 0 0 1 -1 1h-4a1 1 0 0 1 -1 -1z',
  checkbox: 'M9 11l3 3l8 -8 M20 12v6a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2v-12a2 2 0 0 1 2 -2h9',
  chevronsRight: 'M7 7l5 5l-5 5 M13 7l5 5l-5 5',
  grip: 'M9 5m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M9 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M9 19m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M15 5m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M15 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M15 19m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0',
  calendarEvent: 'M4 5m0 2a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2v-12z M16 3v4 M8 3v4 M4 11h16 M8 15h2v2h-2z',
  sun: 'M12 12m-4 0a4 4 0 1 0 8 0a4 4 0 1 0 -8 0 M3 12h1 M12 3v1 M20 12h1 M12 20v1 M5.6 5.6l.7 .7 M18.4 5.6l-.7 .7 M17.7 17.7l.7 .7 M6.3 17.7l-.7 .7',
  pencil: 'M4 20h4l10.5 -10.5a2.828 2.828 0 1 0 -4 -4l-10.5 10.5v4 M13.5 6.5l4 4',
  keyboard: 'M2 6m0 2a2 2 0 0 1 2 -2h16a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-16a2 2 0 0 1 -2 -2z M6 10l0 .01 M10 10l0 .01 M14 10l0 .01 M18 10l0 .01 M6 14l0 .01 M18 14l0 .01 M10 14l4 0',
};

function Icon({ name, size = 16, stroke = 1.75, color = 'currentColor', style, className }) {
  const d = TABLER_PATHS[name];
  if (!d) return null;
  const segs = d.split(' M').map((s, i) => (i === 0 ? s : 'M' + s));
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth={stroke}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ flexShrink: 0, display: 'block', ...style }}
      aria-hidden="true"
    >
      {segs.map((seg, i) => <path key={i} d={seg} />)}
    </svg>
  );
}

// The mosaic "T" mark, rebuilt as inline tesserae so we can recolor per
// direction (tile color + a single coral accent tile).
function MosaicMark({ size = 18, tile = '#93C5FD', accent = '#F13408', gap = 1.4 }) {
  // 7-tile T: 3 across the crossbar, stem of 4. Square 24 grid.
  const u = (24 - gap * 3) / 4; // tile unit
  const cells = [
    // crossbar (top row) — 3 tiles
    { x: 2, y: 2, key: 'a' },
    { x: 2 + u + gap, y: 2, key: 'b', accent: true },
    { x: 2 + (u + gap) * 2, y: 2, key: 'c' },
    // stem — center column going down
    { x: 2 + u + gap, y: 2 + u + gap, key: 'd' },
    { x: 2 + u + gap, y: 2 + (u + gap) * 2, key: 'e' },
  ];
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" style={{ display: 'block', flexShrink: 0 }} aria-hidden="true">
      {cells.map((c) => (
        <rect key={c.key} x={c.x} y={c.y} width={u} height={u} rx={1.5}
          fill={c.accent ? accent : tile} />
      ))}
    </svg>
  );
}

Object.assign(window, { Icon, MosaicMark, TABLER_PATHS });

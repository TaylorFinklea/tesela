// Minimal stroke icons; size 16 default
const Ico = ({ d, size = 16, stroke = 1.6, fill = "none", className = "ico", style }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill={fill}
    stroke="currentColor"
    strokeWidth={stroke}
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
    style={style}
    aria-hidden="true"
  >
    <g dangerouslySetInnerHTML={{ __html: d }} />
  </svg>
);

const Icons = {
  Sun: (p) => <Ico {...p} d='<circle cx="12" cy="12" r="4"/><path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4"/>' />,
  Moon: (p) => <Ico {...p} d='<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>' />,
  Calendar: (p) => <Ico {...p} d='<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M8 3v4M16 3v4M3 9h18"/>' />,
  Graph: (p) => <Ico {...p} d='<circle cx="6" cy="6" r="2.5"/><circle cx="18" cy="7" r="2.5"/><circle cx="12" cy="17" r="2.5"/><path d="M8 7l8 .5M7 8l4 7M17 9l-4 6"/>' />,
  Search: (p) => <Ico {...p} d='<circle cx="11" cy="11" r="6"/><path d="m20 20-3.5-3.5"/>' />,
  Settings: (p) => <Ico {...p} d='<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>' />,
  ChevLeft: (p) => <Ico {...p} d='<path d="m15 6-6 6 6 6"/>' />,
  ChevRight: (p) => <Ico {...p} d='<path d="m9 6 6 6-6 6"/>' />,
  ChevDown: (p) => <Ico {...p} d='<path d="m6 9 6 6 6-6"/>' />,
  Star: (p) => <Ico {...p} d='<path d="M12 3l2.6 5.5 6 .9-4.3 4.2 1 6L12 17l-5.3 2.6 1-6L3.4 9.4l6-.9L12 3z"/>' />,
  Clock: (p) => <Ico {...p} d='<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>' />,
  File: (p) => <Ico {...p} d='<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>' />,
  Tag: (p) => <Ico {...p} d='<path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9-9-9z"/><circle cx="7.5" cy="7.5" r="1.5"/>' />,
  Hash: (p) => <Ico {...p} d='<path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18"/>' />,
  Inbox: (p) => <Ico {...p} d='<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.5 5h13l3.5 7v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6z"/>' />,
  Plus: (p) => <Ico {...p} d='<path d="M12 5v14M5 12h14"/>' />,
  Grip: (p) => <Ico {...p} d='<circle cx="9" cy="6" r="1"/><circle cx="9" cy="12" r="1"/><circle cx="9" cy="18" r="1"/><circle cx="15" cy="6" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="18" r="1"/>' fill="currentColor" stroke="none" />,
  Link: (p) => <Ico {...p} d='<path d="M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"/><path d="M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"/>' />,
  Properties: (p) => <Ico {...p} d='<path d="M4 6h7M4 12h7M4 18h7"/><path d="M14 6h6M14 12h6M14 18h6"/>' />,
  Outline: (p) => <Ico {...p} d='<path d="M4 6h12M8 12h12M12 18h8"/>' />,
  Filter: (p) => <Ico {...p} d='<path d="M3 5h18l-7 9v6l-4-2v-4z"/>' />,
  Check: (p) => <Ico {...p} d='<path d="m5 12 4 4 10-10"/>' />,
  Sparkle: (p) => <Ico {...p} d='<path d="M12 3v4M12 17v4M3 12h4M17 12h4M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M5.6 18.4l2.8-2.8M15.6 8.4l2.8-2.8"/>' />,
  Layers: (p) => <Ico {...p} d='<path d="M12 3 3 8l9 5 9-5z"/><path d="m3 14 9 5 9-5"/>' />,
  Note: (p) => <Ico {...p} d='<path d="M5 4h11l3 3v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"/><path d="M16 4v3h3"/><path d="M8 11h8M8 15h6"/>' />,
  Trash: (p) => <Ico {...p} d='<path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2"/><path d="M5 6v14a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V6"/>' />,
  Cmd: (p) => <Ico {...p} d='<path d="M9 9h6v6H9z"/><path d="M9 6V4a2 2 0 0 0-2 2v3M9 18v2a2 2 0 0 1-2-2v-3M15 6V4a2 2 0 0 1 2 2v3M15 18v2a2 2 0 0 0 2-2v-3"/>' />,
  Dot: (p) => <Ico {...p} d='<circle cx="12" cy="12" r="3"/>' fill="currentColor" stroke="none" />,
};

window.Icons = Icons;

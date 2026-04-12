/**
 * CodeMirror 6 decorations for Tesela block content:
 * - #tags as styled pill
 * - [[wiki-links]] as styled link
 * - key:: value as styled property
 */
import { EditorView, Decoration, type DecorationSet, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";

const tagMark = Decoration.mark({ class: "cm-tesela-tag" });
const wikiLinkMark = Decoration.mark({ class: "cm-tesela-wikilink" });
const wikiLinkBracketMark = Decoration.mark({ class: "cm-tesela-wikilink-bracket" });
const propertyKeyMark = Decoration.mark({ class: "cm-tesela-prop-key" });
const propertyValueMark = Decoration.mark({ class: "cm-tesela-prop-value" });

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;
const PROPERTY_RE = /^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/gm;

function buildDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc.toString();

  const decos: Array<{ from: number; to: number; decoration: Decoration }> = [];

  // Tags
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + m[0].length, decoration: tagMark });
  }

  // Wiki-links
  WIKI_LINK_RE.lastIndex = 0;
  while ((m = WIKI_LINK_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + 2, decoration: wikiLinkBracketMark });
    decos.push({ from: m.index + 2, to: m.index + m[0].length - 2, decoration: wikiLinkMark });
    decos.push({ from: m.index + m[0].length - 2, to: m.index + m[0].length, decoration: wikiLinkBracketMark });
  }

  // Properties
  PROPERTY_RE.lastIndex = 0;
  while ((m = PROPERTY_RE.exec(doc)) !== null) {
    const keyEnd = m.index + m[1].length + 2;
    decos.push({ from: m.index, to: keyEnd, decoration: propertyKeyMark });
    decos.push({ from: keyEnd + 1, to: m.index + m[0].length, decoration: propertyValueMark });
  }

  decos.sort((a, b) => a.from - b.from || a.to - b.to);
  for (const d of decos) {
    builder.add(d.from, d.to, d.decoration);
  }
  return builder.finish();
}

export const teselaDecorations = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

export const teselaDecorationTheme = EditorView.theme({
  ".cm-tesela-tag": {
    color: "oklch(0.70 0.15 220)",
    backgroundColor: "oklch(0.70 0.15 220 / 8%)",
    borderRadius: "4px",
    padding: "1px 5px",
    fontSize: "0.88em",
    fontWeight: "500",
  },
  ".cm-tesela-wikilink": {
    color: "oklch(0.78 0.14 75)",
    textDecoration: "underline",
    textDecorationColor: "oklch(0.78 0.14 75 / 30%)",
    textUnderlineOffset: "2px",
  },
  ".cm-tesela-wikilink-bracket": {
    color: "oklch(0.78 0.14 75 / 25%)",
    fontSize: "0.85em",
  },
  ".cm-tesela-prop-key": {
    color: "oklch(0.45 0 0)",
    fontSize: "0.9em",
  },
  ".cm-tesela-prop-value": {
    color: "oklch(0.68 0.10 160)",
    fontSize: "0.9em",
  },
});

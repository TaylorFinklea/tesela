/**
 * CodeMirror 6 decorations for Tesela block content:
 * - #tags as styled pill
 * - [[wiki-links]] as styled link
 * - key:: value as styled property
 */
import { EditorView, Decoration, WidgetType, type DecorationSet, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";

class EmptyWidget extends WidgetType {
  toDOM() { return document.createElement("span"); }
  eq() { return true; }
}

const tagMark = Decoration.mark({ class: "cm-tesela-tag" });
const tagHide = Decoration.replace({ widget: new EmptyWidget() });
const wikiLinkMark = Decoration.mark({ class: "cm-tesela-wikilink" });
const wikiLinkBracketMark = Decoration.mark({ class: "cm-tesela-wikilink-bracket" });
const propertyKeyMark = Decoration.mark({ class: "cm-tesela-prop-key" });
const propertyValueMark = Decoration.mark({ class: "cm-tesela-prop-value" });

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;
const PROPERTY_RE = /^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/gm;

function buildDecorations(view: EditorView, focused: boolean): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc.toString();

  const decos: Array<{ from: number; to: number; decoration: Decoration }> = [];

  // Tags: hide when unfocused (pills on right are canonical display), show inline when editing
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + m[0].length, decoration: focused ? tagMark : tagHide });
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
      this.decorations = buildDecorations(view, view.hasFocus);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || update.focusChanged) {
        this.decorations = buildDecorations(update.view, update.view.hasFocus);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

export const teselaDecorationTheme = EditorView.theme({
  ".cm-tesela-tag": {
    color: "var(--primary)",
    backgroundColor: "color-mix(in srgb, var(--primary) 10%, transparent)",
    borderRadius: "4px",
    padding: "1px 6px",
    fontSize: "0.88em",
    fontWeight: "500",
  },
  ".cm-tesela-wikilink": {
    color: "var(--primary)",
    textDecoration: "underline",
    textDecorationColor: "color-mix(in srgb, var(--primary) 30%, transparent)",
    textUnderlineOffset: "3px",
    textDecorationThickness: "1px",
  },
  ".cm-tesela-wikilink-bracket": {
    color: "var(--muted-foreground)",
    opacity: "0.4",
    fontSize: "0.85em",
  },
  ".cm-tesela-prop-key": {
    color: "var(--muted-foreground)",
    fontSize: "0.9em",
  },
  ".cm-tesela-prop-value": {
    color: "color-mix(in srgb, var(--primary) 50%, var(--foreground))",
    fontSize: "0.9em",
  },
});

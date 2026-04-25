/**
 * CodeMirror 6 decorations for Tesela block content:
 * - #tags hidden as atomic empty widgets (legacy inline tags from old notes;
 *   new tags live in the `tags::` block property and never appear inline)
 * - [[wiki-links]] as styled link
 * - key:: value as styled property
 */
import { EditorView, Decoration, WidgetType, type DecorationSet, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { RangeSet, RangeSetBuilder } from "@codemirror/state";

class EmptyWidget extends WidgetType {
  toDOM() { return document.createElement("span"); }
  eq() { return true; }
}

const tagHide = Decoration.replace({ widget: new EmptyWidget() });
const wikiLinkMark = Decoration.mark({ class: "cm-tesela-wikilink" });
const wikiLinkBracketMark = Decoration.mark({ class: "cm-tesela-wikilink-bracket" });
const propertyKeyMark = Decoration.mark({ class: "cm-tesela-prop-key" });
const propertyValueMark = Decoration.mark({ class: "cm-tesela-prop-value" });

const tagsLineHide = Decoration.line({ attributes: { class: "cm-tesela-tags-line" } });
const propLineDeco = Decoration.line({ attributes: { class: "cm-tesela-prop-line" } });

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;
const PROPERTY_RE = /^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/gm;
// `tags:: ...` lines are managed via pills and the /tag command — hidden from
// the editor via display:none on the line. CM6 doesn't allow ViewPlugin
// decorations to span line breaks, so we use a per-line CSS decoration.
const TAGS_LINE_RE = /^tags:: .+$/gm;

type Built = { decorations: DecorationSet; atomicTags: RangeSet<Decoration> };

function buildDecorations(view: EditorView): Built {
  const builder = new RangeSetBuilder<Decoration>();
  const atomicBuilder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc.toString();

  const decos: Array<{ from: number; to: number; decoration: Decoration; atomic?: boolean }> = [];

  // Tags: always hidden + atomic so cursor jumps over the token as one unit
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + m[0].length, decoration: tagHide, atomic: true });
  }

  // tags:: property lines: hide the whole line (canonical display is the pill UI)
  TAGS_LINE_RE.lastIndex = 0;
  while ((m = TAGS_LINE_RE.exec(doc)) !== null) {
    // Line decorations need a zero-width range at line start
    decos.push({ from: m.index, to: m.index, decoration: tagsLineHide });
  }

  // Wiki-links
  WIKI_LINK_RE.lastIndex = 0;
  while ((m = WIKI_LINK_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + 2, decoration: wikiLinkBracketMark });
    decos.push({ from: m.index + 2, to: m.index + m[0].length - 2, decoration: wikiLinkMark });
    decos.push({ from: m.index + m[0].length - 2, to: m.index + m[0].length, decoration: wikiLinkBracketMark });
  }

  // Properties: tag the whole line (so it can be hidden by default and toggled
  // expanded by an outer .show-props class), plus inline mark decorations for
  // key/value styling when expanded.
  PROPERTY_RE.lastIndex = 0;
  while ((m = PROPERTY_RE.exec(doc)) !== null) {
    const key = m[1].toLowerCase();
    // tags:: is handled separately via tagsLineHide; skip here so we don't
    // double-decorate (and so the always-hidden style takes precedence).
    if (key === "tags") continue;
    decos.push({ from: m.index, to: m.index, decoration: propLineDeco });
    const keyEnd = m.index + m[1].length + 2;
    decos.push({ from: m.index, to: keyEnd, decoration: propertyKeyMark });
    decos.push({ from: keyEnd + 1, to: m.index + m[0].length, decoration: propertyValueMark });
  }

  decos.sort((a, b) => a.from - b.from || a.to - b.to);
  for (const d of decos) {
    builder.add(d.from, d.to, d.decoration);
    if (d.atomic) atomicBuilder.add(d.from, d.to, d.decoration);
  }
  return { decorations: builder.finish(), atomicTags: atomicBuilder.finish() };
}

export const teselaDecorations = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    atomicTags: RangeSet<Decoration>;
    constructor(view: EditorView) {
      const built = buildDecorations(view);
      this.decorations = built.decorations;
      this.atomicTags = built.atomicTags;
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        const built = buildDecorations(update.view);
        this.decorations = built.decorations;
        this.atomicTags = built.atomicTags;
      }
    }
  },
  {
    decorations: (v) => v.decorations,
    provide: (plugin) => EditorView.atomicRanges.of((view) => view.plugin(plugin)?.atomicTags ?? RangeSet.empty),
  },
);

export const teselaDecorationTheme = EditorView.theme({
  ".cm-tesela-tags-line": {
    display: "none",
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

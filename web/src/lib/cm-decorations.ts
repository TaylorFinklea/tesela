/**
 * CodeMirror 6 decorations for Tesela block content:
 * - #tags hidden as atomic empty widgets (legacy inline tags from old notes;
 *   new tags live in the `tags::` block property and never appear inline)
 * - [[wiki-links]] as styled link
 * - key:: value as styled property; specific keys (configured per-block via
 *   the hiddenPropertyKeysFacet) get a hide-class that the parent
 *   `.show-props` ancestor can override
 */
import { EditorView, Decoration, WidgetType, type DecorationSet, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { Facet, RangeSet, RangeSetBuilder } from "@codemirror/state";

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
const hiddenPropLineDeco = Decoration.line({ attributes: { class: "cm-tesela-hidden-prop-line" } });

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;
// Allow empty values so we can decorate auto-filled `key:: ` (no value yet)
// lines too. Use `[ \t]?` (not `\s?`) for the separator — `\s` would match a
// newline and let `(.*)` greedily eat the next line into the value group.
const PROPERTY_RE = /^([A-Za-z_][A-Za-z0-9_]*)::[ \t]?(.*)$/gm;
// `tags:: ...` lines are managed via pills and the /tag command — hidden from
// the editor via display:none on the line.
const TAGS_LINE_RE = /^tags:: .+$/gm;

/**
 * Per-block configuration for which property keys to hide in the editor.
 * Population: the parent (BlockOutliner) computes this set from the block's
 * inherited tag-property defs and writes it via the compartment-wrapped facet.
 */
export type HiddenKeysConfig = {
  /** Property keys (lowercase) that are unconditionally hidden by default. */
  hide: ReadonlySet<string>;
  /** Property keys (lowercase) hidden only when their value is empty. */
  hideEmpty: ReadonlySet<string>;
};

const EMPTY_HIDDEN_KEYS: HiddenKeysConfig = { hide: new Set(), hideEmpty: new Set() };

export const hiddenPropertyKeysFacet = Facet.define<HiddenKeysConfig, HiddenKeysConfig>({
  combine: (values) => values[0] ?? EMPTY_HIDDEN_KEYS,
});

type Built = { decorations: DecorationSet; atomicTags: RangeSet<Decoration> };

function buildDecorations(view: EditorView): Built {
  const builder = new RangeSetBuilder<Decoration>();
  const atomicBuilder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc.toString();
  const config = view.state.facet(hiddenPropertyKeysFacet);

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
    decos.push({ from: m.index, to: m.index, decoration: tagsLineHide });
  }

  // Wiki-links
  WIKI_LINK_RE.lastIndex = 0;
  while ((m = WIKI_LINK_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + 2, decoration: wikiLinkBracketMark });
    decos.push({ from: m.index + 2, to: m.index + m[0].length - 2, decoration: wikiLinkMark });
    decos.push({ from: m.index + m[0].length - 2, to: m.index + m[0].length, decoration: wikiLinkBracketMark });
  }

  // Properties: emit key/value styling, plus a hide-class line decoration if
  // configured to hide via the facet (either always-hide or empty-hide+empty).
  PROPERTY_RE.lastIndex = 0;
  while ((m = PROPERTY_RE.exec(doc)) !== null) {
    const key = m[1].toLowerCase();
    if (key === "tags") continue; // tags:: handled above
    const value = m[2] ?? "";
    const isEmpty = value.trim() === "";

    const shouldHide = config.hide.has(key) || (isEmpty && config.hideEmpty.has(key));
    if (shouldHide) {
      decos.push({ from: m.index, to: m.index, decoration: hiddenPropLineDeco });
    }

    const keyEnd = m.index + m[1].length + 2; // `key::`
    decos.push({ from: m.index, to: keyEnd, decoration: propertyKeyMark });
    // Skip value mark when value range is empty (zero-width marks are invalid).
    const valueStart = keyEnd + (m[0].length > m[1].length + 2 ? 1 : 0); // after `:: `
    const valueEnd = m.index + m[0].length;
    if (valueEnd > valueStart) {
      decos.push({ from: valueStart, to: valueEnd, decoration: propertyValueMark });
    }
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
      const facetChanged =
        update.startState.facet(hiddenPropertyKeysFacet) !==
        update.state.facet(hiddenPropertyKeysFacet);
      if (update.docChanged || update.viewportChanged || facetChanged) {
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

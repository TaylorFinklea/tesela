/**
 * CodeMirror remote-cursor decorations (Phase 2 desktop presence).
 *
 * Renders OTHER peers' carets inside a block's editor. Each block's editor gets
 * `remoteCursorExtension(slug, bid)`, which:
 *  - holds a `StateField<DecorationSet>` of remote carets. The field `.map`s its
 *    decorations through every local change, so an idle peer's caret SHIFTS
 *    correctly when you type before it (no op-anchored cursor needed on the web).
 *  - rebuilds from fresh offsets whenever the remote-cursors store changes (a
 *    `ViewPlugin` subscribes and dispatches `setRemoteCursors`).
 *
 * The store is fed by the WS client (`onPresence`). Carets are filtered to THIS
 * block's `bid` so each editor only draws the ones that belong in it.
 */
import {
  EditorView,
  Decoration,
  WidgetType,
  ViewPlugin,
  type DecorationSet,
} from "@codemirror/view";
import { StateEffect, StateField, RangeSetBuilder } from "@codemirror/state";

import {
  remoteCursorsForBlock,
  subscribeRemoteCursors,
  type RemoteCursor,
} from "./remote-cursors.ts";

const setRemoteCursors = StateEffect.define<RemoteCursor[]>();

class RemoteCursorWidget extends WidgetType {
  constructor(
    readonly color: string,
    readonly name: string,
  ) {
    super();
  }

  eq(other: RemoteCursorWidget): boolean {
    return other.color === this.color && other.name === this.name;
  }

  toDOM(): HTMLElement {
    const bar = document.createElement("span");
    bar.className = "cm-remote-cursor";
    bar.style.borderLeftColor = this.color;
    bar.setAttribute("aria-hidden", "true");
    if (this.name) {
      const flag = document.createElement("span");
      flag.className = "cm-remote-cursor-flag";
      flag.style.backgroundColor = this.color;
      flag.textContent = this.name;
      bar.appendChild(flag);
    }
    return bar;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

function buildDecorations(docLength: number, cursors: RemoteCursor[]): DecorationSet {
  const placed = cursors
    .map((c) => ({ c, pos: Math.max(0, Math.min(c.offset, docLength)) }))
    .sort((a, b) => a.pos - b.pos || a.c.peer.localeCompare(b.c.peer));
  const builder = new RangeSetBuilder<Decoration>();
  for (const { c, pos } of placed) {
    builder.add(
      pos,
      pos,
      Decoration.widget({
        widget: new RemoteCursorWidget(c.color, c.name ?? ""),
        side: 1,
      }),
    );
  }
  return builder.finish();
}

const remoteCursorTheme = EditorView.baseTheme({
  ".cm-remote-cursor": {
    position: "relative",
    display: "inline-block",
    width: 0,
    borderLeft: "2px solid transparent",
    marginLeft: "-1px",
    height: "1.1em",
    verticalAlign: "text-bottom",
    pointerEvents: "none",
  },
  ".cm-remote-cursor-flag": {
    position: "absolute",
    top: "-1.05em",
    left: "-1px",
    fontSize: "0.62em",
    lineHeight: "1.25",
    padding: "0 3px",
    borderRadius: "3px 3px 3px 0",
    color: "white",
    whiteSpace: "nowrap",
    pointerEvents: "none",
    userSelect: "none",
    opacity: "0.9",
  },
});

/** Editor extension that renders the remote carets for note `slug`, block
 * `bid`, kept in sync with the remote-cursors store. */
export function remoteCursorExtension(slug: string, bid: string) {
  const field = StateField.define<DecorationSet>({
    create() {
      return Decoration.none;
    },
    update(deco, tr) {
      // Auto-shift existing carets through local edits…
      deco = deco.map(tr.changes);
      // …and rebuild from fresh offsets when presence changes.
      for (const e of tr.effects) {
        if (e.is(setRemoteCursors)) {
          deco = buildDecorations(tr.state.doc.length, e.value);
        }
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  const syncer = ViewPlugin.fromClass(
    class {
      private unsub: () => void;
      private dead = false;
      private view: EditorView;

      constructor(view: EditorView) {
        this.view = view;
        this.unsub = subscribeRemoteCursors(() => this.push());
        this.push();
      }

      private push(): void {
        const cursors = remoteCursorsForBlock(slug, bid);
        // Dispatch out of the update cycle (CM forbids dispatch-within-update).
        void Promise.resolve().then(() => {
          if (this.dead) return;
          this.view.dispatch({ effects: setRemoteCursors.of(cursors) });
        });
      }

      destroy(): void {
        this.dead = true;
        this.unsub();
      }
    },
  );

  return [field, syncer, remoteCursorTheme];
}

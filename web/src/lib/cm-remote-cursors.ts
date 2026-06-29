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
  type ViewUpdate,
} from "@codemirror/view";
import { StateEffect, StateField, RangeSetBuilder } from "@codemirror/state";

import {
  remoteCursorsForBlock,
  subscribeRemoteCursors,
  type RemoteCursor,
} from "./remote-cursors.ts";

const setRemoteCursors = StateEffect.define<RemoteCursor[]>();

const FLAG_BELOW_CLASS = "cm-remote-cursor-flag-below";

/**
 * Highest viewport-Y the name flag can occupy before an ancestor clips it: the
 * largest top edge among the bar's clipping ancestors (computed overflow not
 * `visible`). When the daily is scrolled, inner content wrappers translate up
 * (their top goes negative) so the real scroll-port top — which stays put —
 * wins the `max`. Defaults to the viewport top (0).
 */
function clipTopFor(bar: HTMLElement): number {
  let top = 0;
  let node: HTMLElement | null = bar.parentElement;
  while (node && node !== document.body && node !== document.documentElement) {
    const cs = getComputedStyle(node);
    if (cs.overflowY !== "visible" || cs.overflowX !== "visible") {
      top = Math.max(top, node.getBoundingClientRect().top);
    }
    node = node.parentElement;
  }
  return top;
}

/**
 * Keep the full device-name flag visible regardless of caret position: flip it
 * BELOW the caret when there isn't room above (caret near the top of the scroll
 * viewport), otherwise leave it above. Pure presentation — never shifts text or
 * steals pointer events.
 */
function positionFlag(flag: HTMLElement): void {
  const bar = flag.parentElement;
  if (!bar || !bar.isConnected) return;
  const barTop = bar.getBoundingClientRect().top;
  const flagH = flag.offsetHeight || 16;
  flag.classList.toggle(FLAG_BELOW_CLASS, barTop - clipTopFor(bar) < flagH + 2);
}

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
      // toDOM runs before the node is laid out; decide above/below next frame.
      requestAnimationFrame(() => positionFlag(flag));
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

// Scoped (EditorView.theme, NOT baseTheme): CM prefixes these selectors with a
// generated class added only to editors that include THIS extension — i.e. the
// per-block daily editors — so the global document editor's scroller is left
// alone. Each block is its own single-line, auto-height editor (lineWrapping
// on), so its `.cm-scroller` never scrolls — yet CM's default `overflow-x: auto`
// coerces `overflow-y` to clip too, which cut off the name flag that sits above
// the caret. Let the flag overflow the per-block scroller.
const blockScrollerOverflow = EditorView.theme({
  ".cm-scroller": {
    overflow: "visible",
  },
});

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
    // Paint above adjacent blocks' text when the flag overflows into them.
    zIndex: "2",
  },
  // Flipped below the caret when there isn't room above (caret near the scroll
  // viewport's top). Square corner points up at the caret instead of down.
  ".cm-remote-cursor-flag-below": {
    top: "1.15em",
    borderRadius: "0 3px 3px 3px",
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
      private scrollParent: HTMLElement | null = null;
      private readonly onScroll = () => this.repositionFlags();

      constructor(view: EditorView) {
        this.view = view;
        this.unsub = subscribeRemoteCursors(() => this.push());
        this.push();
        // Outer-pane scroll doesn't dispatch CM updates, so re-evaluate the
        // above/below flip on it directly. Deferred until the view is mounted.
        requestAnimationFrame(() => this.attachScroll());
      }

      update(u: ViewUpdate): void {
        // Caret moves / edits / viewport changes can change room-above.
        if (u.geometryChanged || u.docChanged || u.viewportChanged) {
          this.view.requestMeasure({ read: () => this.repositionFlags() });
        }
      }

      private attachScroll(): void {
        if (this.dead) return;
        let node: HTMLElement | null = this.view.dom.parentElement;
        while (node && node !== document.body && node !== document.documentElement) {
          const cs = getComputedStyle(node);
          if (
            (cs.overflowY === "auto" || cs.overflowY === "scroll") &&
            node.scrollHeight > node.clientHeight
          ) {
            this.scrollParent = node;
            break;
          }
          node = node.parentElement;
        }
        (this.scrollParent ?? window).addEventListener("scroll", this.onScroll, {
          passive: true,
        });
        this.repositionFlags();
      }

      private repositionFlags(): void {
        if (this.dead) return;
        const flags = this.view.dom.getElementsByClassName("cm-remote-cursor-flag");
        for (let i = 0; i < flags.length; i++) positionFlag(flags[i] as HTMLElement);
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
        (this.scrollParent ?? window).removeEventListener("scroll", this.onScroll);
      }
    },
  );

  return [field, syncer, remoteCursorTheme, blockScrollerOverflow];
}

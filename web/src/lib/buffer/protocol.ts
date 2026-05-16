/**
 * Prism v5 — renderer protocol.
 *
 * Three renderer flavors (page, derived, ambient) share a host-agnostic
 * shape: each gets a typed `props` shape, declares a `cascade` of mode
 * components ordered descending by `minSize`, and the host picks which
 * cascade member to instantiate based on the leaf's current size.
 *
 * Renderers OWN their data fetching (TanStack Query keyed on their input);
 * the host doesn't negotiate loading/error states. The host's sole
 * responsibilities: pass the input (Page / Reference / nothing), pass the
 * current size, accept NavigationIntent events.
 */

import type { Component } from "svelte";
import type { Reference, ReferenceKind } from "./types.ts";

export type Size = { cols: number; rows: number };

export type NavigationIntent =
  | {
      kind: "open-page";
      path: string;
      how: "replace" | "split-right" | "split-down" | "new-tab";
    }
  | { kind: "open-tag"; value: string }
  | { kind: "open-query"; dsl: string };

export interface DerivedRendererProps<R extends Reference> {
  reference: R;
  size: Size;
  onNavigate: (i: NavigationIntent) => void;
}

export interface PageRendererProps {
  pageId: string;
  size: Size;
  onNavigate: (i: NavigationIntent) => void;
}

export interface AmbientRendererProps {
  size: Size;
  onNavigate: (i: NavigationIntent) => void;
}

export interface RendererCascade<P extends Record<string, any>> {
  /** Bare fallback mode, used when no `modes` entry's `minSize` is satisfied. */
  default: Component<P>;
  /** Higher-fidelity modes, descending by `minSize`. Pick the first that fits. */
  modes: ReadonlyArray<{
    minSize: Size;
    component: Component<P>;
    /** Optional debug label, surfaced in the status line. */
    label?: string;
  }>;
}

/** Pick the most-featured cascade member that fits within `size`. */
export function pickCascadeMember<P extends Record<string, any>>(
  cascade: RendererCascade<P>,
  size: Size,
): Component<P> {
  for (const m of cascade.modes) {
    if (size.cols >= m.minSize.cols && size.rows >= m.minSize.rows) {
      return m.component;
    }
  }
  return cascade.default;
}

// ── renderer-module interfaces ─────────────────────────────────────────────

export interface PageRenderer {
  /** Page-type frontmatter value this renderer handles (e.g. "note", "daily"). */
  acceptsType: string;
  cascade: RendererCascade<PageRendererProps>;
}

export interface DerivedRenderer<K extends ReferenceKind = ReferenceKind> {
  accepts: K;
  cascade: RendererCascade<
    DerivedRendererProps<Extract<Reference, { kind: K }>>
  >;
}

export interface AmbientRenderer {
  cascade: RendererCascade<AmbientRendererProps>;
}

// ── error type ─────────────────────────────────────────────────────────────

export class RendererReferenceMismatch extends Error {
  readonly rendererName: string;
  readonly expected: ReferenceKind;
  readonly got: ReferenceKind;

  constructor(
    rendererName: string,
    expected: ReferenceKind,
    got: ReferenceKind,
  ) {
    super(
      `Renderer "${rendererName}" expects a ${expected} reference, got ${got}.`,
    );
    this.name = "RendererReferenceMismatch";
    this.rendererName = rendererName;
    this.expected = expected;
    this.got = got;
  }
}

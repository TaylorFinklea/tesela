/**
 * Prism v4 — `:` ex-mode popup state.
 *
 * Tiny shared store so the layout can open the ex-command line (on `:`
 * keypress outside a cm-editor) and the line can flip itself closed
 * after Enter/Esc.
 */

let open = $state(false);
let priorPaneId = $state<string | undefined>(undefined);

export function isColonModeOpen(): boolean {
  return open;
}

export function getColonPriorPaneId(): string | undefined {
  return priorPaneId;
}

export function openColonMode(opts?: { priorPaneId?: string }) {
  priorPaneId = opts?.priorPaneId;
  open = true;
}

export function closeColonMode() {
  open = false;
}

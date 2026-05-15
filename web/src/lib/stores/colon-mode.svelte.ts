/**
 * Prism v4 — `:` ex-mode popup state.
 *
 * Tiny shared store so the layout can open the ex-command line (on `:`
 * keypress outside a cm-editor) and the line can flip itself closed
 * after Enter/Esc.
 */

let open = $state(false);

export function isColonModeOpen(): boolean {
  return open;
}

export function openColonMode() {
  open = true;
}

export function closeColonMode() {
  open = false;
}

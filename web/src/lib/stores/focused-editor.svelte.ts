/**
 * Focused-editor presence.
 *
 * Set by the focused BlockEditor on cm focus, cleared on blur/unmount. The
 * shell reads `isEditorFocused()` into `commandCtx.editorFocused` so the
 * leader's `i` (insert) and `p` (properties) buckets populate with editor
 * commands ONLY when a block is focused. Those commands then dispatch
 * `tesela:run-editor-command`, which the focused BlockEditor handles by
 * supplying the real `SlashContext` (the shell can't build one).
 *
 * Tracks the focused block id (not a bare boolean) so a late `blur` from the
 * previously-focused editor can't clobber a fresh `focus` on another — the
 * clear is id-guarded.
 */
let focusedId = $state<string | null>(null);

export function setFocusedEditor(id: string): void {
  focusedId = id;
}

export function clearFocusedEditor(id: string): void {
  if (focusedId === id) focusedId = null;
}

export function isEditorFocused(): boolean {
  return focusedId !== null;
}

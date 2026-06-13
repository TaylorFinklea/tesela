import type { PropertyDefinition } from "$lib/property-registry";

export type SlashContext = {
  /** Focused block identity. `bid` present ⇒ may be Loro-bound; the parent's structured funnel resolves the address. */
  block: { id: string; bid: string | null; properties: Record<string, string> };
  /** Whole-block text BEFORE the `/` trigger: doc.slice(0, slashStartPos). Frozen at dispatch time. */
  before: string;
  /** Block text from the caret onward: doc.slice(cursorPos). Frozen at dispatch time. */
  after: string;
  /** Property defs in scope — drives `/p` leaves and status-verb hoisting. */
  propertyDefs: PropertyDefinition[];
  /** Status enum for this block (statusChoices ?? ["todo","doing","done"]). */
  statusChoices: string[];
  /** Tag name → its default property-key names, for tag-add auto-fill (PROP6). */
  autoFillNames: (tagName: string) => string[];

  /** Replace the trigger region (whole-doc replace) under the guard, then onChange. `caretFromEnd` chars before doc end; omit ⇒ caret at end (never collapses to 0). */
  replaceTrigger: (insert: string, caretFromEnd?: number) => void;
  /** Emit ONE structured container op (never a `key:: value` line); empty value clears. Wraps onSetProperty → setBlockPropertyStructured. Re-resolves address LIVE at call time. */
  setProperty: (key: string, value: string) => void;
  /** Add a tag AND fire onTagAdded so the parent emits tag property-default ops. Text-only no-op if already present. */
  addTag: (tagName: string) => void;
  /** Hand a template note-id to the parent (onInsertTemplate) to expand into child blocks. */
  insertTemplate: (noteId: string) => void;

  /** Strip the trigger, then open the date picker; on pick sets `propertyKey` (default prefs.bareDateField) to the ISO via setProperty. */
  openDatePicker: (propertyKey?: string) => void;
  /** Strip the trigger, then open the tag-manager autocomplete popover (multi-toggle; commit re-enters addTag). */
  openTagPicker: () => void;
  /** Strip the trigger, then open the template-pick popover (commit re-enters insertTemplate). */
  openTemplatePicker: () => void;
  /** Open a typed-property value picker/input (select/checkbox/text) for `def`; commit re-enters setProperty. */
  openPropertyValue: (def: PropertyDefinition) => void;

  /** Selection-only caret move within the focused block — never persisted, never splices. */
  moveCursor: (anchor: number, head?: number) => void;
  /** Shared tail: close slash menu, reset slashStartPos=-1, fire onSlashCommand(verb), refocus view. */
  finish: (verb: string) => void;
};

# Type-aware property editing — tesela-11v

## Goal

- make property chips actionable on web and iOS
- checkbox: one-action boolean toggle
- multi-select: add/remove checklist backed by per-item CRDT list operations
- URL/email/phone: safe native links while retaining an edit affordance
- iOS: tap-to-edit sheet at least matching the web popup

## Write contract

- scalar writes keep the existing typed `set-property` seam
- multi-select edits send a delta (`add[]`, `remove[]`), never a replacement comma string
- server and on-device engine author `AddToList` / `RemoveFromList` per item
- empty/duplicate list deltas are idempotent; stable block bids remain the preferred address
- optimistic clients reconcile from the canonical materialized note after mutation

## Web contract

- native button for editable checkbox and multi-select chips
- native anchor for URL/email/phone chip values with normalized `https:`, `mailto:`, or `tel:` targets
- multi-select popup has keyboard-safe checklist, Save, and Cancel
- query-table editing uses the same editor and list-delta path

## iOS contract

- `PropertyChip` uses native `Button` / `Link`, not tap gestures
- item-driven edit sheet owns draft state, Save, Cancel, and dismissal
- checkbox toggles immediately; multi-select commits list deltas; typed text/date/number/link values use the scalar write seam
- relay mode calls the local engine directly, then pushes and refreshes like existing property mutations

## Out of scope

- new property types or registry schema
- general property authoring/configuration UI
- number steppers, object/node pickers, or datetime picker expansion
- changing Markdown materialization format

## Verify

- focused Rust route + FFI tests for typed/list operations
- `pnpm --dir web test:unit`
- `pnpm --dir web check`
- focused iOS XCTest plus simulator build
- iPhone Simulator product flow: checkbox, multi-select, links, save/cancel, relaunch persistence

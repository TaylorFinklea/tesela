# Inbox → Views Rename Spec

## Goal
- Product copy says **Views**, not **Inbox**, across web/desktop/iOS user-visible surfaces.
- Aggressive rename where safe; compatibility preserved for existing data.

## Rename scope
- Rename visible labels: commands, palette entries, tab/nav labels, headers, empty states, editor/reset/delete copy, built-in view display name.
- Rename bundled command manifests so iOS/offline command palette says Views.
- Rename code symbols/files only when low-risk and local to presentation; avoid large route/storage churn unless covered by aliases.

## Compatibility boundary
- Keep stable synced/storage ids unless a migration/alias exists:
  - `builtin-inbox` view id remains accepted.
  - ambient buffer name `inbox` remains accepted as an alias.
  - existing localStorage keys remain readable; new keys may use `views`.
  - `#inbox` capture tag semantics remain unchanged unless a separate migration is designed.
- Built-in view display name becomes `Views`; older synced records named `Inbox` should render as `Views` for the built-in id and preserve user-created names.

## Tests
- Web unit test: command manifest/builtin command label exposes Views, not Open Inbox.
- iOS unit test: bundled command manifest/tab label and fallback built-in view display as Views.
- Existing saved-view tests updated for compatibility expectations.

## Verify
- `pnpm --dir web test:unit`
- `pnpm --dir web check`
- iOS focused test(s) for saved views/commands; full iOS if practical.

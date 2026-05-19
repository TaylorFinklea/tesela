# Import + backup workflow

This is the trust path: how to bring an existing knowledge base into Tesela
and how to back it up so you can take real notes here.

Verified by `cargo test -p tesela-cli --test integration
logseq_import_backup_restore_byte_exact_round_trip` — that test runs the
full Logseq → mosaic → backup → restore → byte-exact-diff loop. If it's
green on `main`, the trust criterion is met.

## Importing a Logseq graph

```bash
# 1. Create a fresh mosaic for the import (don't mix into an existing one).
tesela init ~/teselas/from-logseq

# 2. Dry-run first to see what would happen. Counts new imports, conflicts,
#    and hard-skips (whiteboards, drawings — see "Known lossy" below).
tesela --mosaic ~/teselas/from-logseq import-logseq \
    --source ~/logseq --dry-run

# 3. Run for real.
tesela --mosaic ~/teselas/from-logseq import-logseq --source ~/logseq
```

Re-running the same import is idempotent — files already-imported and
unchanged upstream are no-ops. Files where you've edited the Tesela copy
**and** the upstream changed surface as conflicts; resolve them via the web
UI (Settings → Data → Logseq plan) which presents per-file
Skip / Overwrite / Rename choices.

### What the importer preserves

| Logseq construct | Tesela representation |
|---|---|
| Journal `journals/2026_05_19.md` | Page `notes/2026-05-19.md` (ISO date) |
| Namespace `pages/Parent___Child.md` | Page `notes/parent-child.md` |
| Page properties (`title::`, `tags::`, etc.) | Frontmatter or block properties |
| Tasks (TODO / DOING / DONE / LATER / NOW / WAITING / CANCELED) | `status:: todo/doing/done/backlog/canceled` block property |
| Priority `[#A] / [#B] / [#C]` | `priority:: high/medium/low` |
| `SCHEDULED: <2026-05-20 Wed>` | `scheduled:: 2026-05-20` |
| `DEADLINE: <2026-05-21 Thu>` | `deadline:: 2026-05-21` |
| Wikilinks `[[Page]]` | Pass through; resolved by Tesela's link table |
| Hashtags `#tag` | Pass through |
| External links `[label](url)` | Pass through |
| Asset references `![](../assets/foo.png)` | Rewritten to `../attachments/foo.png` (asset file is copied) |
| Block refs `((uuid))` | Preserved literally — Tesela's link resolver can grow to handle them later |
| Queries `#+BEGIN_QUERY ... #+END_QUERY` | Wrapped in a ` ```query ` fenced code block — content stays visible so you can re-create as a Tesela query |
| Triple-backtick code blocks | Untouched — task / block-ref conversions skip over them |

### Known lossy

These constructs are **not** converted; the importer doesn't drop your
data, but you'll see them as hard-skips or unchanged source files:

- **Whiteboards** (`whiteboards/*.edn`) — Logseq's tldraw format. Not
  rendered in Tesela. Files stay in the source vault.
- **Excalidraw drawings** (`draws/*.excalidraw`) — same.
- **Custom Logseq plugins / commands** — anything Logseq-specific outside
  the formats above isn't interpreted.
- **Logseq-only block properties** (`collapsed::`, `id::`, `file::`,
  `file-path::`) — stripped as noise.

## Backing up

Two destinations supported: a local path (default) and a remote git
repository.

### Local backups

```bash
# Goes to <mosaic>/.tesela/backups/backup-<ISO-timestamp>/
tesela --mosaic ~/teselas/main backup
```

By default this is **not** encrypted. Pass `--encrypt` to force encryption
on local backups, or set `--output <path>` to a directory outside the
mosaic and encryption turns on automatically (since the bytes leave the
mosaic's trust boundary).

```bash
# Generate the keypair first (one-time, per mosaic):
tesela --mosaic ~/teselas/main backup-keygen

# Encrypted local copy:
tesela --mosaic ~/teselas/main backup --encrypt
```

### Off-machine backups (recommended)

Push to any git remote you control. Encryption is always on:

```bash
tesela --mosaic ~/teselas/main backup \
    --git-remote git@github.com:taylor/tesela-backups.git \
    --git-branch main
```

Tesela maintains a local git mirror at
`<mosaic>/.tesela/backups/.git-mirror/` and pushes each backup as a commit
to the remote. The private age identity stays in the macOS Keychain — the
remote only ever sees ciphertext.

### Verifying a backup

```bash
# Re-runs the manifest validation on an existing backup directory.
tesela backup-verify ~/teselas/main/.tesela/backups/backup-20260519-093000

# Lists backups under a destination.
tesela --mosaic ~/teselas/main backup-list
```

## Restore drill

You should do this once before you trust real data:

```bash
# 1. Take a backup.
tesela --mosaic ~/teselas/main backup

# 2. Restore to a sibling directory (default — won't touch the original).
tesela --mosaic ~/teselas/main restore \
    ~/teselas/main/.tesela/backups/backup-<timestamp>

# That creates ~/teselas/main-restored/. Diff against the original:
diff -r ~/teselas/main/notes ~/teselas/main-restored/notes
diff -r ~/teselas/main/attachments ~/teselas/main-restored/attachments
```

No output from `diff -r` means the round trip is byte-exact. (The CI test
runs the same shape continuously.)

For an in-place restore (overwrites the current mosaic, but first renames
the existing dir to `<mosaic>.before-restore-<timestamp>` so nothing is
silently destroyed):

```bash
tesela --mosaic ~/teselas/main restore \
    ~/teselas/main/.tesela/backups/backup-<timestamp> --in-place
```

## Trust criteria summary

You can trust real notes here when:

1. `cargo test -p tesela-cli --test integration
   logseq_import_backup_restore_byte_exact_round_trip` passes. (CI gate.)
2. You've done one manual restore drill against your own mosaic and the
   `diff -r` is clean.
3. You've pointed `--git-remote` at an off-machine repo so a hardware loss
   on this Mac is recoverable.

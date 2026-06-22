//! SQLite schema definitions and migrations for Tesela

pub const SCHEMA_VERSION: i64 = 6;

pub const CREATE_MIGRATIONS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
)"#;

/// Each migration is a name and a list of SQL statements to execute in order.
pub const MIGRATIONS: &[(&str, &[&str])] = &[(
    "001_initial",
    &[
        r#"CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    content TEXT NOT NULL,
    path TEXT NOT NULL,
    checksum TEXT NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]'
)"#,
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    id UNINDEXED,
    title,
    body,
    tags,
    content=notes,
    content_rowid=rowid
)"#,
        r#"CREATE TABLE IF NOT EXISTS links (
    source_id TEXT NOT NULL,
    target TEXT NOT NULL,
    link_text TEXT NOT NULL,
    position INTEGER NOT NULL,
    link_type TEXT NOT NULL DEFAULT 'internal',
    FOREIGN KEY (source_id) REFERENCES notes(id) ON DELETE CASCADE
)"#,
        "CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_id)",
        "CREATE INDEX IF NOT EXISTS idx_links_target ON links(target)",
        r#"CREATE TRIGGER IF NOT EXISTS notes_fts_insert AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, id, title, body, tags) VALUES (new.rowid, new.id, new.title, new.body, new.tags);
END"#,
        r#"CREATE TRIGGER IF NOT EXISTS notes_fts_delete AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, id, title, body, tags) VALUES('delete', old.rowid, old.id, old.title, old.body, old.tags);
END"#,
        r#"CREATE TRIGGER IF NOT EXISTS notes_fts_update AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, id, title, body, tags) VALUES('delete', old.rowid, old.id, old.title, old.body, old.tags);
    INSERT INTO notes_fts(rowid, id, title, body, tags) VALUES (new.rowid, new.id, new.title, new.body, new.tags);
END"#,
    ],
), (
    "002_type_system",
    &[
        // Tag definitions — cached from Tag pages
        r#"CREATE TABLE IF NOT EXISTS tag_defs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    extends TEXT,
    icon TEXT DEFAULT '📄',
    color TEXT DEFAULT '#808080',
    properties_json TEXT NOT NULL DEFAULT '[]',
    note_id TEXT,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE SET NULL
)"#,
        // Property definitions — cached from Property pages
        r#"CREATE TABLE IF NOT EXISTS property_defs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    value_type TEXT NOT NULL DEFAULT 'text',
    choices_json TEXT,
    default_value TEXT,
    multiple_values BOOLEAN NOT NULL DEFAULT 0,
    hide_empty BOOLEAN NOT NULL DEFAULT 0,
    description TEXT,
    note_id TEXT,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE SET NULL
)"#,
        // Block-level property values — for cross-page queries
        r#"CREATE TABLE IF NOT EXISTS block_properties (
    block_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    property_id TEXT NOT NULL,
    property_name TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (block_id, property_id),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
)"#,
        "CREATE INDEX IF NOT EXISTS idx_block_props_note ON block_properties(note_id)",
        "CREATE INDEX IF NOT EXISTS idx_block_props_property ON block_properties(property_name)",
        "CREATE INDEX IF NOT EXISTS idx_block_props_value ON block_properties(property_name, value)",
        // Add note_type column to notes table
        "ALTER TABLE notes ADD COLUMN note_type TEXT",
    ],
), (
    "003_note_versions",
    &[
        // Per-note edit history. Every PUT writes a row; capped at 200/note.
        r#"CREATE TABLE IF NOT EXISTS note_versions (
    id INTEGER PRIMARY KEY,
    note_id TEXT NOT NULL,
    version_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    prev_content TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
    UNIQUE(note_id, version_number)
)"#,
        "CREATE INDEX IF NOT EXISTS idx_note_versions_note ON note_versions(note_id, version_number DESC)",
    ],
), (
    // Sync substrate. Owned by the `tesela-sync` crate (see
    // crates/tesela-sync/src/schema.rs for the canonical DDL). Duplicated
    // here so the mosaic database has these tables even when tesela-sync
    // is not driving migrations. Idempotent via IF NOT EXISTS.
    "004_sync_substrate",
    &[
        r#"CREATE TABLE IF NOT EXISTS oplog (
    hlc_ntp         INTEGER NOT NULL,
    device_id       BLOB    NOT NULL,
    schema_version  INTEGER NOT NULL,
    payload         BLOB    NOT NULL,
    content_hash    BLOB    NOT NULL,
    txn_id          BLOB,
    PRIMARY KEY (hlc_ntp, device_id)
) WITHOUT ROWID"#,
        "CREATE INDEX IF NOT EXISTS idx_oplog_device_hlc ON oplog(device_id, hlc_ntp)",
        "CREATE INDEX IF NOT EXISTS idx_oplog_content_hash ON oplog(content_hash)",
        r#"CREATE TABLE IF NOT EXISTS peer_cursors (
    peer_device_id          BLOB    PRIMARY KEY,
    last_seen_hlc_ntp       INTEGER NOT NULL,
    last_ack_at_wall_clock  INTEGER NOT NULL
)"#,
        r#"CREATE TABLE IF NOT EXISTS parked_ops (
    op_hlc_ntp      INTEGER NOT NULL,
    op_device_id    BLOB    NOT NULL,
    schema_version  INTEGER NOT NULL,
    payload         BLOB    NOT NULL,
    parked_at       INTEGER NOT NULL,
    park_reason     TEXT    NOT NULL,
    PRIMARY KEY (op_hlc_ntp, op_device_id)
)"#,
        r#"CREATE TABLE IF NOT EXISTS device_self (
    rowid           INTEGER PRIMARY KEY CHECK (rowid = 1),
    device_id       BLOB    NOT NULL,
    ed25519_pubkey  BLOB    NOT NULL,
    ed25519_privkey BLOB    NOT NULL,
    display_name    TEXT    NOT NULL
)"#,
        r#"CREATE TABLE IF NOT EXISTS group_members (
    group_id        BLOB    NOT NULL,
    device_id       BLOB    NOT NULL,
    ed25519_pubkey  BLOB    NOT NULL,
    display_name    TEXT,
    added_at        INTEGER NOT NULL,
    PRIMARY KEY (group_id, device_id)
)"#,
        r#"CREATE TABLE IF NOT EXISTS group_keys (
    group_id        BLOB    PRIMARY KEY,
    group_sym_key   BLOB    NOT NULL
)"#,
    ],
), (
    // Type-definition FKs: switch `tag_defs.note_id` and
    // `property_defs.note_id` from `ON DELETE SET NULL` to
    // `ON DELETE CASCADE` so cached Tag/Property pages don't outlive
    // their source note. The application-level fix in
    // `SqliteIndex::remove_note` and `index_type_info` is the primary
    // cleanup path; this migration hardens the schema as a defense in
    // depth. SQLite can't ALTER a FK, so we drop and recreate the two
    // tables. Cached rows are caches of note content — they get
    // repopulated on the next reindex.
    "005_type_defs_cascade_fks",
    &[
        r#"DROP TABLE IF EXISTS tag_defs"#,
        r#"CREATE TABLE tag_defs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    extends TEXT,
    icon TEXT DEFAULT '📄',
    color TEXT DEFAULT '#808080',
    properties_json TEXT NOT NULL DEFAULT '[]',
    note_id TEXT,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
)"#,
        r#"DROP TABLE IF EXISTS property_defs"#,
        r#"CREATE TABLE property_defs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    value_type TEXT NOT NULL DEFAULT 'text',
    choices_json TEXT,
    default_value TEXT,
    multiple_values BOOLEAN NOT NULL DEFAULT 0,
    hide_empty BOOLEAN NOT NULL DEFAULT 0,
    description TEXT,
    note_id TEXT,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
)"#,
    ],
), (
    // Per-type property configuration (Phase 1 of the per-type
    // property/type spec, 2026-06-22). `tag_defs` and `property_defs`
    // are owned by migration 005, so we ALTER ADD COLUMN here rather
    // than drop/recreate:
    //   - tag_defs.property_overrides_json — per-type override map
    //     (keyed by property name, case-insensitive): choices / show /
    //     default / hide_choices.
    //   - tag_defs.plural — plural display name (falls back to name).
    //   - property_defs.hide_by_default — so the Rust resolver can
    //     derive the 3-state `show` (parity with the TS registry, which
    //     already reads this from frontmatter).
    "006_per_type_property_config",
    &[
        "ALTER TABLE tag_defs ADD COLUMN property_overrides_json TEXT NOT NULL DEFAULT '{}'",
        "ALTER TABLE tag_defs ADD COLUMN plural TEXT",
        "ALTER TABLE property_defs ADD COLUMN hide_by_default BOOLEAN NOT NULL DEFAULT 0",
    ],
)];

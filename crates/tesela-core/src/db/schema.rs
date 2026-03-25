//! SQLite schema definitions and migrations for Tesela

pub const SCHEMA_VERSION: i64 = 2;

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
)];

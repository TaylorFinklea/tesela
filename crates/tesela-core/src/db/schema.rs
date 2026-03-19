//! SQLite schema definitions and migrations for Tesela

pub const SCHEMA_VERSION: i64 = 1;

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
)];

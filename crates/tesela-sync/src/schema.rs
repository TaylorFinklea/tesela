//! SQLite DDL for the sync substrate.
//!
//! These statements are wired into `tesela-core`'s `MIGRATIONS` constant
//! as migration `004_sync_substrate`. Keeping them here means the sync
//! crate owns its schema while reusing tesela-core's existing migration
//! runner.

/// The full set of DDL statements for migration `004_sync_substrate`.
///
/// Order matters: tables before indexes, and the `oplog` index after the
/// table that it indexes.
pub const SYNC_SUBSTRATE_DDL: &[&str] = &[
    // The append-only oplog. Every locally-authored mutation appends a
    // row; every successfully-applied remote op appends a row. The
    // primary key is the HLC plus device id, which is globally unique
    // by construction. `hlc_ntp` stores the full uhlc NTP64 as a signed
    // i64 (all reasonable values fit in i64 because seconds since epoch
    // is well under 2^31).
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
    // Per-peer cursors: where each peer has acknowledged ops up to.
    r#"CREATE TABLE IF NOT EXISTS peer_cursors (
        peer_device_id          BLOB    PRIMARY KEY,
        last_seen_hlc_ntp       INTEGER NOT NULL,
        last_ack_at_wall_clock  INTEGER NOT NULL
    )"#,
    // Ops we received but our local schema cannot apply (newer than us
    // or no translator chain available).
    r#"CREATE TABLE IF NOT EXISTS parked_ops (
        op_hlc_ntp      INTEGER NOT NULL,
        op_device_id    BLOB    NOT NULL,
        schema_version  INTEGER NOT NULL,
        payload         BLOB    NOT NULL,
        parked_at       INTEGER NOT NULL,
        park_reason     TEXT    NOT NULL,
        PRIMARY KEY (op_hlc_ntp, op_device_id)
    )"#,
    // This device's own identity. Singleton row (rowid=1).
    r#"CREATE TABLE IF NOT EXISTS device_self (
        rowid           INTEGER PRIMARY KEY CHECK (rowid = 1),
        device_id       BLOB    NOT NULL,
        ed25519_pubkey  BLOB    NOT NULL,
        ed25519_privkey BLOB    NOT NULL,
        display_name    TEXT    NOT NULL
    )"#,
    // Other paired devices in our group(s).
    r#"CREATE TABLE IF NOT EXISTS group_members (
        group_id        BLOB    NOT NULL,
        device_id       BLOB    NOT NULL,
        ed25519_pubkey  BLOB    NOT NULL,
        display_name    TEXT,
        added_at        INTEGER NOT NULL,
        PRIMARY KEY (group_id, device_id)
    )"#,
    // Symmetric keys for each group we belong to.
    r#"CREATE TABLE IF NOT EXISTS group_keys (
        group_id        BLOB    PRIMARY KEY,
        group_sym_key   BLOB    NOT NULL
    )"#,
];

/// Apply the sync substrate DDL to a freshly-connected SQLite handle.
///
/// Idempotent (all `CREATE` statements use `IF NOT EXISTS`). Useful for
/// tests and standalone `tesela-sync` integration; in production this
/// runs via the tesela-core migration mechanism.
pub async fn apply_ddl(pool: &sqlx::SqlitePool) -> crate::SyncResult<()> {
    for stmt in SYNC_SUBSTRATE_DDL {
        sqlx::query(stmt).execute(pool).await?;
    }
    Ok(())
}

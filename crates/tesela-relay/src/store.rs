//! SQLite-backed store for relay state. Two tables — registrations
//! and ops — both per-group-id. Schema mirrors the spec's storage
//! section.
//!
//! Stage 2a wires the connection + migrations only; reads/writes
//! arrive with the endpoint handlers in stages 3a-3d.

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Per-spec storage. Two tables, both keyed by `group_id`.
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .with_context(|| format!("invalid sqlite path: {}", path.display()))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .context("opening sqlite pool")?;
        migrate(&pool).await?;
        Ok(Self { pool })
    }

    #[allow(dead_code)] // wired in stages 3a-3d
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

async fn migrate(pool: &SqlitePool) -> Result<()> {
    // Inline migration for now — small + stable enough that we don't
    // need sqlx::migrate! file plumbing yet. Add proper migration
    // files once we cross v1 of the wire format and need backward
    // compat across relay versions.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS relay_registrations (
            group_id      BLOB NOT NULL PRIMARY KEY,
            auth_key      BLOB NOT NULL,
            registered_at INTEGER NOT NULL,
            intent        BLOB NOT NULL
        );

        CREATE TABLE IF NOT EXISTS relay_ops (
            group_id    BLOB NOT NULL,
            seq         INTEGER NOT NULL,
            from_device BLOB NOT NULL,
            ts          REAL NOT NULL,
            payload     BLOB NOT NULL,
            -- JSON array of hex-encoded device ids that have acked.
            acks        TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (group_id, seq),
            FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_relay_ops_group_seq ON relay_ops(group_id, seq);
        "#,
    )
    .execute(pool)
    .await
    .context("running relay migrations")?;
    Ok(())
}

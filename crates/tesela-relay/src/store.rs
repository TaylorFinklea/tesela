//! SQLite-backed store for relay state. Three tables — registrations,
//! ops, and device-seen — all per-group-id. Schema mirrors the spec's
//! storage section.
//!
//! All concurrency is handled by SQLite's single-writer model: every
//! method that mutates wraps its read-then-write in a transaction so
//! concurrent PUTs/ACKs can't race on the per-group `seq` counter or
//! the `acks` set.

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

/// Per-spec storage. Cloneable wrapper around the connection pool;
/// every method takes `&self` so handlers share without contention.
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

/// Outcome of a `POST /register` write. Mirrors the spec's response
/// shape (200 vs 409) one-to-one so the handler is a thin translation.
pub enum RegisterOutcome {
    /// First-write: stored the registration.
    Inserted,
    /// Re-register with byte-identical tuple — no-op, idempotent.
    Idempotent,
    /// A different registration already exists for this group_id.
    /// Conflict carries the stored record so the handler can echo it
    /// in the 409 body (spec requirement).
    Conflict(Registration),
}

/// One row from `relay_registrations`. Serialised verbatim on
/// `GET /registration`.
#[derive(Clone)]
pub struct Registration {
    pub auth_key: Vec<u8>,
    pub registered_at: i64,
    pub intent: Vec<u8>,
}

/// One row from `relay_ops`. Returned by `list_ops_since`; serialised
/// to JSON envelope shape by the handler.
#[derive(Clone)]
pub struct RelayOp {
    pub seq: i64,
    pub from_device: Vec<u8>,
    pub ts: f64,
    pub payload: Vec<u8>,
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

    // ── Registrations ─────────────────────────────────────────────

    /// First-write registration. Idempotent if the tuple matches byte-for-byte;
    /// returns `Conflict(existing)` otherwise so the handler can 409 with the
    /// stored record echoed back.
    pub async fn register_group(
        &self,
        group_id: &[u8; 16],
        auth_key: &[u8; 32],
        registered_at: i64,
        intent: &[u8],
    ) -> Result<RegisterOutcome> {
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query(
            "SELECT auth_key, registered_at, intent FROM relay_registrations WHERE group_id = ?",
        )
        .bind(&group_id[..])
        .fetch_optional(&mut *tx)
        .await
        .context("read existing registration")?;

        if let Some(row) = existing {
            let stored = Registration {
                auth_key: row.get::<Vec<u8>, _>("auth_key"),
                registered_at: row.get::<i64, _>("registered_at"),
                intent: row.get::<Vec<u8>, _>("intent"),
            };
            tx.commit().await?;
            if stored.auth_key == auth_key
                && stored.registered_at == registered_at
                && stored.intent == intent
            {
                return Ok(RegisterOutcome::Idempotent);
            }
            return Ok(RegisterOutcome::Conflict(stored));
        }

        sqlx::query(
            "INSERT INTO relay_registrations(group_id, auth_key, registered_at, intent) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&group_id[..])
        .bind(&auth_key[..])
        .bind(registered_at)
        .bind(intent)
        .execute(&mut *tx)
        .await
        .context("insert registration")?;
        tx.commit().await?;
        Ok(RegisterOutcome::Inserted)
    }

    /// Fetch the stored registration record. `None` if the group isn't
    /// registered (handler returns 404).
    pub async fn get_registration(&self, group_id: &[u8; 16]) -> Result<Option<Registration>> {
        let row = sqlx::query(
            "SELECT auth_key, registered_at, intent FROM relay_registrations WHERE group_id = ?",
        )
        .bind(&group_id[..])
        .fetch_optional(&self.pool)
        .await
        .context("read registration")?;
        Ok(row.map(|r| Registration {
            auth_key: r.get::<Vec<u8>, _>("auth_key"),
            registered_at: r.get::<i64, _>("registered_at"),
            intent: r.get::<Vec<u8>, _>("intent"),
        }))
    }

    /// Admin recovery — wipe a registration. Cascades to ops + device-seen
    /// via the FK. Returns `true` if a row was deleted.
    pub async fn delete_registration(&self, group_id: &[u8; 16]) -> Result<bool> {
        let res = sqlx::query("DELETE FROM relay_registrations WHERE group_id = ?")
            .bind(&group_id[..])
            .execute(&self.pool)
            .await
            .context("delete registration")?;
        Ok(res.rows_affected() > 0)
    }

    // ── Ops ────────────────────────────────────────────────────────

    /// Append one op to a group's FIFO. Assigns a monotonic seq
    /// (per-group `MAX(seq) + 1`) inside a transaction so concurrent
    /// PUTs from different HTTP threads can't collide.
    /// Returns `(seq, ts)` the relay assigned.
    pub async fn insert_op(
        &self,
        group_id: &[u8; 16],
        from_device: &[u8; 32],
        ts: f64,
        payload: &[u8],
    ) -> Result<(i64, f64)> {
        let mut tx = self.pool.begin().await?;
        let next_seq: i64 = sqlx::query(
            "SELECT COALESCE(MAX(seq), 0) + 1 AS next FROM relay_ops WHERE group_id = ?",
        )
        .bind(&group_id[..])
        .fetch_one(&mut *tx)
        .await
        .context("next seq")?
        .get("next");

        sqlx::query(
            "INSERT INTO relay_ops(group_id, seq, from_device, ts, payload) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&group_id[..])
        .bind(next_seq)
        .bind(&from_device[..])
        .bind(ts)
        .bind(payload)
        .execute(&mut *tx)
        .await
        .context("insert op")?;
        tx.commit().await?;
        Ok((next_seq, ts))
    }

    /// Return ops in this group with `seq > since`, ordered ascending.
    /// Empty list when the requester is already caught up.
    pub async fn list_ops_since(
        &self,
        group_id: &[u8; 16],
        since: i64,
    ) -> Result<Vec<RelayOp>> {
        let rows = sqlx::query(
            "SELECT seq, from_device, ts, payload FROM relay_ops \
             WHERE group_id = ? AND seq > ? ORDER BY seq ASC",
        )
        .bind(&group_id[..])
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .context("list ops")?;
        Ok(rows
            .into_iter()
            .map(|r| RelayOp {
                seq: r.get::<i64, _>("seq"),
                from_device: r.get::<Vec<u8>, _>("from_device"),
                ts: r.get::<f64, _>("ts"),
                payload: r.get::<Vec<u8>, _>("payload"),
            })
            .collect())
    }

    // ── Acks + GC ──────────────────────────────────────────────────

    /// Record that `device_id` has applied every op up to and including
    /// `applied_seq` in this group. Implementation appends the hex
    /// device_id to the `acks` JSON array of every op with `seq <= applied_seq`
    /// in this group that doesn't already contain it. Returns the number
    /// of op rows touched.
    ///
    /// Uses SQLite's JSON functions (json_each / json_insert) so the
    /// set semantics are SQL-native; falls back to Rust-side dedupe via
    /// `json_group_array(DISTINCT ...)` to keep the array minimal.
    pub async fn ack_ops(
        &self,
        group_id: &[u8; 16],
        device_id: &[u8; 32],
        applied_seq: i64,
    ) -> Result<u64> {
        let device_hex = hex::encode(device_id);
        let mut tx = self.pool.begin().await?;
        // Fetch each candidate op + current acks, update only when the
        // device isn't already present. Cheaper than json_each for the
        // typical case (few devices per group, few un-acked ops).
        let rows = sqlx::query(
            "SELECT seq, acks FROM relay_ops WHERE group_id = ? AND seq <= ?",
        )
        .bind(&group_id[..])
        .bind(applied_seq)
        .fetch_all(&mut *tx)
        .await
        .context("fetch ops for ack")?;

        let mut touched = 0u64;
        for row in rows {
            let seq: i64 = row.get("seq");
            let acks_json: String = row.get("acks");
            let mut acks: Vec<String> = serde_json::from_str(&acks_json).unwrap_or_default();
            if acks.iter().any(|d| d == &device_hex) {
                continue;
            }
            acks.push(device_hex.clone());
            let new_json = serde_json::to_string(&acks)?;
            sqlx::query("UPDATE relay_ops SET acks = ? WHERE group_id = ? AND seq = ?")
                .bind(new_json)
                .bind(&group_id[..])
                .bind(seq)
                .execute(&mut *tx)
                .await
                .context("update acks")?;
            touched += 1;
        }
        tx.commit().await?;
        Ok(touched)
    }

    /// Delete ops where every known member has acked. Pass the current
    /// known-member set so the test is `len(intersect(known, acks)) ==
    /// len(known)` rather than `len(acks) >= total_devices` (which
    /// would never fire if a device departed without un-ack).
    /// Returns the number of rows GC'd.
    pub async fn gc_fully_acked_ops(
        &self,
        group_id: &[u8; 16],
        known_members_hex: &[String],
    ) -> Result<u64> {
        if known_members_hex.is_empty() {
            // No known members → can't safely GC anything (someone
            // might still join).
            return Ok(0);
        }
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query("SELECT seq, acks FROM relay_ops WHERE group_id = ?")
            .bind(&group_id[..])
            .fetch_all(&mut *tx)
            .await
            .context("fetch ops for gc")?;
        let mut to_delete: Vec<i64> = Vec::new();
        for row in rows {
            let seq: i64 = row.get("seq");
            let acks_json: String = row.get("acks");
            let acks: Vec<String> = serde_json::from_str(&acks_json).unwrap_or_default();
            if known_members_hex.iter().all(|m| acks.iter().any(|a| a == m)) {
                to_delete.push(seq);
            }
        }
        for seq in &to_delete {
            sqlx::query("DELETE FROM relay_ops WHERE group_id = ? AND seq = ?")
                .bind(&group_id[..])
                .bind(*seq)
                .execute(&mut *tx)
                .await
                .context("delete acked op")?;
        }
        tx.commit().await?;
        Ok(to_delete.len() as u64)
    }

    // ── Device-seen tracking (for GC's known-member set) ──────────

    /// Upsert (group_id, device_id) → now. Called on every successful
    /// authenticated request so the membership set captures consumers
    /// (devices that only ack, never PUT) too.
    pub async fn touch_device(
        &self,
        group_id: &[u8; 16],
        device_id: &[u8; 32],
        seen_ts: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO relay_device_seen(group_id, device_id, last_seen_ts) \
             VALUES (?, ?, ?) \
             ON CONFLICT(group_id, device_id) DO UPDATE SET last_seen_ts = excluded.last_seen_ts",
        )
        .bind(&group_id[..])
        .bind(&device_id[..])
        .bind(seen_ts)
        .execute(&self.pool)
        .await
        .context("touch device-seen")?;
        Ok(())
    }

    /// Known members of a group, as hex device_id strings — the set
    /// `gc_fully_acked_ops` intersects against. Membership window is
    /// `now - ttl_secs`; devices that haven't checked in inside the
    /// window are forgotten (their backlog is GC-eligible).
    pub async fn known_members_hex(
        &self,
        group_id: &[u8; 16],
        now: i64,
        ttl_secs: i64,
    ) -> Result<Vec<String>> {
        let cutoff = now - ttl_secs;
        let rows = sqlx::query(
            "SELECT device_id FROM relay_device_seen \
             WHERE group_id = ? AND last_seen_ts > ?",
        )
        .bind(&group_id[..])
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .context("known members")?;
        Ok(rows
            .into_iter()
            .map(|r| hex::encode(r.get::<Vec<u8>, _>("device_id")))
            .collect())
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

        -- Tracks every device that has touched a group (PUT, GET, or
        -- ACK) within the retention window. GC's known-member set is
        -- derived from this — capturing consumers that only fetch +
        -- ack, never PUT (which `ops.from_device` would miss).
        CREATE TABLE IF NOT EXISTS relay_device_seen (
            group_id     BLOB NOT NULL,
            device_id    BLOB NOT NULL,
            last_seen_ts INTEGER NOT NULL,
            PRIMARY KEY (group_id, device_id),
            FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_relay_device_seen_group_ts
            ON relay_device_seen(group_id, last_seen_ts);
        "#,
    )
    .execute(pool)
    .await
    .context("running relay migrations")?;
    Ok(())
}

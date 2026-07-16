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

/// One row from `relay_snapshots`. Returned by `list_snapshots`;
/// serialised to JSON by the handler. `stream_id` + `payload` are
/// OPAQUE ciphertext/keys — the relay never interprets them.
#[derive(Clone)]
pub struct SnapshotRow {
    pub stream_id: Vec<u8>,
    pub snapshot_seq: i64,
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

    /// Upsert the `disc -> group_id` discovery index (ra7 P0 step 2).
    /// Idempotent by construction: `disc` is a one-way PRF of the
    /// group key, so it always maps to the same `group_id` for a
    /// given group — re-registration just overwrites with the same
    /// value.
    pub async fn upsert_discovery_index(&self, disc: &[u8; 32], group_id: &[u8; 16]) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO relay_discovery_index(disc, group_id) VALUES (?, ?)",
        )
        .bind(&disc[..])
        .bind(&group_id[..])
        .execute(&self.pool)
        .await
        .context("upsert discovery index")?;
        Ok(())
    }

    /// Resolve a discovery handle to its `group_id`. `None` if no
    /// group has published this `disc` (handler returns 404).
    pub async fn lookup_discovery(&self, disc: &[u8; 32]) -> Result<Option<[u8; 16]>> {
        let row = sqlx::query("SELECT group_id FROM relay_discovery_index WHERE disc = ?")
            .bind(&disc[..])
            .fetch_optional(&self.pool)
            .await
            .context("read discovery index")?;
        Ok(row
            .map(|r| r.get::<Vec<u8>, _>("group_id"))
            .map(|v| v.try_into().expect("group_id column is always 16 bytes")))
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
    /// (per-group `MAX(MAX(seq), compaction_seq) + 1`) inside a
    /// transaction so concurrent PUTs from different HTTP threads
    /// can't collide. The compaction watermark from `relay_group_meta`
    /// must participate: after a full compaction `relay_ops` is empty,
    /// and allocating from the table alone would restart at 1 — below
    /// every caught-up consumer's cursor, making the op permanently
    /// undeliverable (the #195 black hole). Mirrors the CF Worker's
    /// AUTOINCREMENT, which never reuses seqs.
    /// Returns `(seq, ts)` the relay assigned.
    pub async fn insert_op(
        &self,
        group_id: &[u8; 16],
        from_device: &[u8; 16],
        ts: f64,
        payload: &[u8],
    ) -> Result<(i64, f64)> {
        // BEGIN IMMEDIATE (not the default deferred BEGIN) takes the
        // write lock up front, so the SELECT MAX(seq)+1 below and the
        // subsequent INSERT are atomic w.r.t. every other writer on
        // this connection pool. A deferred BEGIN only acquires the
        // write lock at the first actual write, so two concurrent
        // same-group PUTs could both read the same next_seq before
        // either writes — a real TOCTOU that surfaces as a lock
        // conflict or a duplicate-seq PRIMARY KEY violation on the
        // loser. Concurrent callers now serialize on the IMMEDIATE
        // lock (waiting up to the pool's 5s busy_timeout) instead of
        // racing.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let next_seq: i64 = sqlx::query(
            "SELECT MAX( \
               COALESCE((SELECT MAX(seq) FROM relay_ops WHERE group_id = ?), 0), \
               COALESCE((SELECT compaction_seq FROM relay_group_meta WHERE group_id = ?), 0) \
             ) + 1 AS next",
        )
        .bind(&group_id[..])
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
    pub async fn list_ops_since(&self, group_id: &[u8; 16], since: i64) -> Result<Vec<RelayOp>> {
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
        device_id: &[u8; 16],
        applied_seq: i64,
    ) -> Result<u64> {
        let device_hex = hex::encode(device_id);
        let mut tx = self.pool.begin().await?;
        // Fetch each candidate op + current acks, update only when the
        // device isn't already present. Cheaper than json_each for the
        // typical case (few devices per group, few un-acked ops).
        let rows = sqlx::query("SELECT seq, acks FROM relay_ops WHERE group_id = ? AND seq <= ?")
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
            if known_members_hex
                .iter()
                .all(|m| acks.iter().any(|a| a == m))
            {
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
        device_id: &[u8; 16],
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

    // ── APNs device-token registry (sync durability P3b/P3c) ──────

    /// Upsert (group_id, device_id) → APNs push token. Idempotent;
    /// last-write-wins on the token + timestamp. Mirrors the CF Worker's
    /// `upsertDeviceToken` (+ the per-group `group_id` the multi-group
    /// store needs). The token is stored verbatim (the handler lowercases
    /// it for wire parity with the CF Worker).
    pub async fn upsert_device_token(
        &self,
        group_id: &[u8; 16],
        device_id: &[u8; 16],
        apns_token: &str,
        updated_at: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO relay_device_tokens(group_id, device_id, apns_token, updated_at) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(group_id, device_id) DO UPDATE SET \
               apns_token = excluded.apns_token, updated_at = excluded.updated_at",
        )
        .bind(&group_id[..])
        .bind(&device_id[..])
        .bind(apns_token)
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .context("upsert device token")?;
        Ok(())
    }

    /// APNs tokens of every device in `group_id` EXCEPT `exclude_device`
    /// (the depositor — it already has the op). The push fans out to
    /// these so the group's OTHER devices wake. Mirrors the CF Worker's
    /// `listOtherApnsTokens`.
    pub async fn list_other_apns_tokens(
        &self,
        group_id: &[u8; 16],
        exclude_device: &[u8; 16],
    ) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT apns_token FROM relay_device_tokens \
             WHERE group_id = ? AND device_id != ?",
        )
        .bind(&group_id[..])
        .bind(&exclude_device[..])
        .fetch_all(&self.pool)
        .await
        .context("list other apns tokens")?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("apns_token"))
            .collect())
    }

    /// Prune a permanently-dead APNs token (APNs reported HTTP 410
    /// Unregistered or reason BadDeviceToken) so a stale token left by a
    /// reinstalled device — which keeps a NEW row under a new device_id —
    /// isn't pushed (and logged as a failure) on every future deposit.
    pub async fn delete_device_token(&self, group_id: &[u8; 16], apns_token: &str) -> Result<()> {
        sqlx::query("DELETE FROM relay_device_tokens WHERE group_id = ? AND apns_token = ?")
            .bind(&group_id[..])
            .bind(apns_token)
            .execute(&self.pool)
            .await
            .context("delete device token")?;
        Ok(())
    }

    // ── Snapshots + snapshot-gated compaction ─────────────────────

    /// Deposit a full snapshot batch covering relay-seq `covers_seq`
    /// in ONE transaction: upsert every per-stream snapshot, advance
    /// the compaction watermark, then GC superseded ops. Returns the
    /// number of `relay_ops` rows deleted. Consistency matters here —
    /// a half-applied deposit could GC ops without a snapshot to
    /// restore them, so all three steps share a transaction.
    pub async fn deposit_snapshot_batch(
        &self,
        group_id: &[u8; 16],
        covers_seq: i64,
        snapshots: &[(Vec<u8>, i64, Vec<u8>)],
        now: i64,
    ) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        for (stream_id, snapshot_seq, payload) in snapshots {
            sqlx::query(
                "INSERT INTO relay_snapshots(group_id, stream_id, snapshot_seq, payload, created_at) \
                 VALUES (?, ?, ?, ?, ?) \
                 ON CONFLICT(group_id, stream_id) DO UPDATE SET \
                   snapshot_seq = excluded.snapshot_seq, \
                   payload = excluded.payload, \
                   created_at = excluded.created_at \
                 WHERE excluded.snapshot_seq >= relay_snapshots.snapshot_seq",
            )
            .bind(&group_id[..])
            .bind(&stream_id[..])
            .bind(snapshot_seq)
            .bind(&payload[..])
            .bind(now)
            .execute(&mut *tx)
            .await
            .context("upsert snapshot")?;
        }

        // Advance the watermark (only forward — never regress).
        sqlx::query(
            "INSERT INTO relay_group_meta(group_id, compaction_seq) \
             VALUES (?, ?) \
             ON CONFLICT(group_id) DO UPDATE SET \
               compaction_seq = MAX(relay_group_meta.compaction_seq, excluded.compaction_seq)",
        )
        .bind(&group_id[..])
        .bind(covers_seq)
        .execute(&mut *tx)
        .await
        .context("set compaction seq")?;

        // Snapshot-gated compaction: drop ops the snapshot supersedes.
        let gc = sqlx::query("DELETE FROM relay_ops WHERE group_id = ? AND seq <= ?")
            .bind(&group_id[..])
            .bind(covers_seq)
            .execute(&mut *tx)
            .await
            .context("gc superseded ops")?
            .rows_affected();

        tx.commit().await?;
        Ok(gc)
    }

    /// Latest snapshot per opaque stream for a group. Empty when the
    /// group has never deposited a snapshot.
    pub async fn list_snapshots(&self, group_id: &[u8; 16]) -> Result<Vec<SnapshotRow>> {
        let rows = sqlx::query(
            "SELECT stream_id, snapshot_seq, payload FROM relay_snapshots \
             WHERE group_id = ? ORDER BY stream_id ASC",
        )
        .bind(&group_id[..])
        .fetch_all(&self.pool)
        .await
        .context("list snapshots")?;
        Ok(rows
            .into_iter()
            .map(|r| SnapshotRow {
                stream_id: r.get::<Vec<u8>, _>("stream_id"),
                snapshot_seq: r.get::<i64, _>("snapshot_seq"),
                payload: r.get::<Vec<u8>, _>("payload"),
            })
            .collect())
    }

    /// The group's compaction watermark (0 if no snapshot deposited).
    pub async fn get_compaction_seq(&self, group_id: &[u8; 16]) -> Result<i64> {
        let row = sqlx::query("SELECT compaction_seq FROM relay_group_meta WHERE group_id = ?")
            .bind(&group_id[..])
            .fetch_optional(&self.pool)
            .await
            .context("get compaction seq")?;
        Ok(row.map(|r| r.get::<i64, _>("compaction_seq")).unwrap_or(0))
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

        -- disc -> group_id index (recovery-phrase discovery, ra7 P0
        -- step 2). `disc` is the one-way HKDF handle a phrase-only
        -- device derives from its GroupKey (see
        -- `tesela_sync::crypto::recovery::derive_discovery_handle`);
        -- it has the key but not the random `group_id` this relay
        -- indexes everything by. A `disc` maps to exactly one
        -- `group_id` (it's a PRF of that group's key), so upserts are
        -- idempotent overwrites, not conflicts.
        CREATE TABLE IF NOT EXISTS relay_discovery_index (
            disc     BLOB NOT NULL PRIMARY KEY,
            group_id BLOB NOT NULL,
            FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE
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

        -- Latest encrypted snapshot per (group, opaque stream). The
        -- `stream_id` is the client's per-note key — OPAQUE to the
        -- relay (it never interprets it). `payload` is AEAD ciphertext.
        -- A snapshot batch covering relay-seq N is the compaction
        -- gate: once deposited, ops with seq <= N can be GC'd from
        -- relay_ops (snapshot-gated compaction, spine Phase 1b-i).
        CREATE TABLE IF NOT EXISTS relay_snapshots (
            group_id      BLOB NOT NULL,
            stream_id     BLOB NOT NULL,
            snapshot_seq  INTEGER NOT NULL,
            payload       BLOB NOT NULL,
            created_at    INTEGER NOT NULL,
            PRIMARY KEY (group_id, stream_id),
            FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE
        );

        -- Per-group compaction watermark: the highest relay-seq that a
        -- deposited snapshot batch has covered. Ops at or below it are
        -- GC-eligible because the snapshot supersedes them.
        CREATE TABLE IF NOT EXISTS relay_group_meta (
            group_id       BLOB NOT NULL PRIMARY KEY,
            compaction_seq INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE
        );

        -- APNs device-token registry (sync durability P3b/P3c). One row
        -- per (group, device) → that device's APNs push token (hex). A
        -- token is a ROUTING identifier, not note content — the relay
        -- stays zero-knowledge. On a PUT /ops the relay sends a
        -- content-available silent push to the group's OTHER tokens so a
        -- suspended device catches up instantly. (CF Worker parity: its
        -- per-DO `device_tokens` table is single-group, so it has no
        -- group_id column; the multi-group Rust store needs it.)
        CREATE TABLE IF NOT EXISTS relay_device_tokens (
            group_id   BLOB NOT NULL,
            device_id  BLOB NOT NULL,
            apns_token TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (group_id, device_id),
            FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await
    .context("running relay migrations")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("relay.sqlite")).await.unwrap();
        (store, dir)
    }

    /// APNs device-token registry: upsert is per-(group,device) LWW,
    /// list excludes the depositor, and prune removes a dead token so it
    /// isn't pushed again (the sync-durability P3c stale-token fix).
    #[tokio::test]
    async fn device_token_upsert_list_and_prune() {
        let (store, _dir) = temp_store().await;
        let group = [7u8; 16];
        let dev_a = [0xaau8; 16];
        let dev_b = [0xbbu8; 16];
        store
            .register_group(&group, &[1u8; 32], 0, &[2u8; 32])
            .await
            .unwrap();

        store
            .upsert_device_token(&group, &dev_a, "aatoken", 1)
            .await
            .unwrap();
        store
            .upsert_device_token(&group, &dev_b, "bbtoken", 2)
            .await
            .unwrap();

        // A deposit from dev_a wakes only the OTHER device (dev_b).
        assert_eq!(
            store.list_other_apns_tokens(&group, &dev_a).await.unwrap(),
            vec!["bbtoken".to_string()]
        );

        // Upsert is last-write-wins on (group, device): re-registering dev_b
        // replaces its token, not adds a row.
        store
            .upsert_device_token(&group, &dev_b, "bbtoken2", 3)
            .await
            .unwrap();
        assert_eq!(
            store.list_other_apns_tokens(&group, &dev_a).await.unwrap(),
            vec!["bbtoken2".to_string()]
        );

        // Pruning a permanently-dead token removes it so it isn't pushed
        // (and logged as a failure) on every future deposit.
        store
            .delete_device_token(&group, "bbtoken2")
            .await
            .unwrap();
        assert!(store
            .list_other_apns_tokens(&group, &dev_a)
            .await
            .unwrap()
            .is_empty());

        // dev_a's own token is untouched — a deposit from dev_b still wakes it.
        assert_eq!(
            store.list_other_apns_tokens(&group, &dev_b).await.unwrap(),
            vec!["aatoken".to_string()]
        );
    }
}

//! Engine-direct writes for MCP tool calls that create/mutate notes.
//!
//! Mirrors `tesela-cli`'s `open_locked_engine` pattern
//! (`crates/tesela-cli/src/mosaic_notes.rs`, tesela-ows.2): a raw
//! `FsNoteStore` write never syncs and gets reverted by the engine's next
//! materialize (engine-only-writes rule, 2026-06-09). MCP tool calls are
//! worse than the CLI here — an agent invokes them invisibly, so a silently
//! reverted note is much harder for a human to notice. `tesela-mcp` is a
//! separate `[[bin]]` from `tesela-cli` (no shared lib target), so the small
//! mosaic-locking + hydrate helpers are duplicated here rather than adding a
//! lib target to `tesela-cli` for one shared module; keep this in sync with
//! `tesela-cli/src/mosaic_notes.rs` and `backfill_task.rs` if either changes.
//!
//! Engine-direct was chosen over routing through a running `tesela-server`'s
//! HTTP API because the MCP server, like the CLI, is meant to work standalone
//! against a mosaic with no server required — requiring a running server
//! would make `create_note` fail whenever the desktop/web app isn't open,
//! which is a common case for an agent working headlessly.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tesela_core::stable_uuid_from_slug;
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

/// Read the mosaic's existing device id (no write); falls back to a random
/// id if absent/malformed. Mirrors `tesela-cli::backfill_task::load_device_id`.
fn load_device_id(mosaic: &Path) -> DeviceId {
    let path = mosaic.join(".tesela").join("device_id.hex");
    if let Ok(bytes) = std::fs::read(&path) {
        let s = String::from_utf8_lossy(&bytes);
        let s = s.trim();
        if s.len() == 32 {
            let mut arr = [0u8; 16];
            let mut ok = true;
            for i in 0..16 {
                match u8::from_str_radix(&s[i * 2..i * 2 + 2], 16) {
                    Ok(b) => arr[i] = b,
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return DeviceId::from_bytes(arr);
            }
        }
    }
    DeviceId::new_random()
}

/// Exclusive, non-blocking flock on `<mosaic>/.tesela/server.lock` — the
/// SAME lock the server holds. Mirrors
/// `tesela-cli::backfill_task::acquire_mosaic_lock`.
fn acquire_mosaic_lock(mosaic: &Path) -> Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let tesela_dir = mosaic.join(".tesela");
    std::fs::create_dir_all(&tesela_dir)?;
    let lock_path = tesela_dir.join("server.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        anyhow::bail!("lock held (EWOULDBLOCK)");
    }
    Ok(file)
}

/// `title:` from a YAML frontmatter block; `None` if absent. Mirrors
/// `tesela-cli::mosaic_notes::frontmatter_title`.
fn frontmatter_title(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim_end_matches('\r') != "---" {
        return None;
    }
    for line in lines {
        if line.trim_end_matches('\r') == "---" {
            return None;
        }
        if let Some(v) = line.strip_prefix("title:") {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Turn a plain-text body into the bullet form the block-model round trip
/// preserves — mirrors `tesela-cli::mosaic_notes::ensure_bulleted_body`
/// (a heading/bare-prose body is silently dropped by `parse_note`
/// otherwise).
pub(crate) fn ensure_bulleted_body(body: &str) -> String {
    if body.is_empty() || body.lines().any(|l| l.trim_start().starts_with("- ")) {
        return if body.is_empty() {
            "- \n".to_string()
        } else {
            body.to_string()
        };
    }
    let mut out = String::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push_str("- ");
        out.push_str(line);
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("- \n");
    }
    out
}

/// Parse `content`, stamp persistent block ids onto any unstamped bullets,
/// and return the canonical serialized form. Mirrors
/// `tesela-cli::mosaic_notes::stamp_block_ids`.
pub(crate) fn stamp_block_ids(content: &str) -> String {
    let tree = tesela_core::note_tree::parse_note(content);
    if !tree.stamped_any {
        return content.to_string();
    }
    tesela_core::note_tree::serialize_note(&tree)
}

/// Hydrate a note into the engine: a `NoteUpsert` of its content — the
/// per-bid-reconcile op every editor save records — synchronously
/// persisting the snapshot and materializing `<slug>.md` to disk. Mirrors
/// `tesela-cli::mosaic_notes::hydrate_note`.
async fn hydrate_note(
    engine: &LoroEngine,
    note_id: [u8; 16],
    slug: &str,
    content: &str,
) -> Result<()> {
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(slug.to_string()),
            title: frontmatter_title(content).unwrap_or_else(|| slug.to_string()),
            content: content.to_string(),
            created_at_millis: 0,
        })
        .await
        .map_err(|e| anyhow::anyhow!("hydrate note {slug}: {e}"))?;
    Ok(())
}

/// Lock the mosaic (refuse while `tesela-server`/the desktop holds it) and
/// open the Loro engine over its snapshots + materialized notes dir.
/// Mirrors `tesela-cli::mosaic_notes::open_locked_engine`. The returned
/// `File` guard must stay alive for the duration of the write; dropping it
/// releases the flock.
pub(crate) async fn open_locked_engine(mosaic: &Path) -> Result<(std::fs::File, LoroEngine)> {
    let lock = acquire_mosaic_lock(mosaic).context(
        "could not lock the mosaic — is tesela-server (or the desktop app) running on it? \
         Stop it before writing via MCP, or make the change through the running server/app \
         instead (single-writer; tesela-mcp refuses to bypass the lock).",
    )?;
    let device = load_device_id(mosaic);
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let hlc = Arc::new(Hlc::new(device));
    let engine = LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir))
        .await
        .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;
    Ok((lock, engine))
}

/// Create a note through the engine: lock the mosaic, hydrate a `NoteUpsert`
/// with stamped block ids, and return the slug it was written under.
/// Mirrors `tesela-cli::cmd_new`'s write path.
pub(crate) async fn create_note_via_engine(
    mosaic: &Path,
    title: &str,
    tags: &[&str],
    body: &str,
) -> Result<String> {
    use tesela_core::storage::markdown::{generate_frontmatter, sanitize_filename};

    let (_lock, engine) = open_locked_engine(mosaic).await?;

    let slug = sanitize_filename(title);
    let path = mosaic.join("notes").join(format!("{slug}.md"));
    if path.exists() {
        anyhow::bail!("Note '{}' already exists", title);
    }

    let now = chrono::Utc::now();
    let full_content = if body.trim_start().starts_with("---") {
        body.to_string()
    } else {
        let frontmatter = generate_frontmatter(title, tags, now, &Default::default());
        format!("{}\n{}", frontmatter, ensure_bulleted_body(body))
    };
    let stamped = stamp_block_ids(&full_content);

    hydrate_note(&engine, stable_uuid_from_slug(&slug), &slug, &stamped)
        .await
        .context("Failed to create note")?;
    drop(engine);

    Ok(slug)
}

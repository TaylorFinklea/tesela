//! Phase 12.3 — periodic deadline/scheduled scanner that emits
//! `DeadlineApproaching` / `ScheduledFires` events on the WS bus.
//!
//! The web client subscribes and decides whether to render a desktop
//! `Notification`. Server-side firing means the user gets reminders even
//! when the page hasn't been refreshed (the WS reconnect resends nothing,
//! but the next scan will pick up anything still in-window).
//!
//! Dedupe: every fire is keyed by `(block_id, kind, deadline_iso)` and
//! recorded in an in-memory set so the same window doesn't re-trigger
//! every minute. Restarting the server resets the set; that's fine —
//! a duplicate notification right after restart is acceptable, but
//! flooding the user every minute for 60 minutes isn't.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, warn};

use tesela_core::{
    block::parse_blocks, storage::markdown::parse_frontmatter, traits::note_store::NoteStore,
};

use crate::state::WsEvent;

/// In-memory dedupe set. Each entry is `block_id|kind|deadline_iso`.
pub struct Notifier {
    fired: Mutex<HashSet<String>>,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            fired: Mutex::new(HashSet::new()),
        }
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Default lead time in minutes for `deadline::` notifications.
const DEADLINE_LEAD_MIN: i64 = 60;

/// Spawn the periodic scanner. Walks notes every 60 seconds, computes
/// fire times for any open Task block with a `deadline::` or
/// `scheduled::` value, and emits WS events for any that crossed
/// their threshold this tick.
pub fn start(
    notifier: Arc<Notifier>,
    store: Arc<dyn NoteStore>,
    ws_tx: broadcast::Sender<WsEvent>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Skip the first immediate tick so startup doesn't immediately fire
        // for tasks that should have notified during the previous run.
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(e) = scan_once(&notifier, &store, &ws_tx).await {
                warn!("notification scan failed: {}", e);
            }
        }
    });
}

/// One pass over all notes. Public so tests can drive it deterministically.
pub async fn scan_once(
    notifier: &Notifier,
    store: &Arc<dyn NoteStore>,
    ws_tx: &broadcast::Sender<WsEvent>,
) -> anyhow::Result<()> {
    let now = Utc::now();
    let notes = store.list(None, usize::MAX, 0).await?;
    let mut fired = notifier.fired.lock().await;
    for note in notes {
        let body = match parse_frontmatter(&note.content) {
            Ok((_, body)) => body,
            Err(_) => continue,
        };
        let blocks = parse_blocks(note.id.as_str(), &body);
        for block in blocks {
            // Skip blocks that aren't tasks (no status property → not a task).
            // Done/canceled tasks don't fire either.
            let status = block.properties.get("status").map(String::as_str);
            if matches!(status, None | Some("done") | Some("canceled")) {
                continue;
            }
            let title = task_title(&block.text);

            if let Some(raw) = block.properties.get("deadline") {
                if let Some(deadline_dt) = parse_deadline_local(raw) {
                    let lead = chrono::Duration::minutes(DEADLINE_LEAD_MIN);
                    let fire_at = deadline_dt - lead;
                    let key = format!("{}|deadline|{}", block.id, deadline_dt.to_rfc3339());
                    // Fire when the notification window opens AND the deadline
                    // hasn't passed yet (a deadline 2h in the past with a 1h
                    // lead is past — surfacing it now is just noise).
                    if now >= fire_at && now < deadline_dt && !fired.contains(&key) {
                        fired.insert(key);
                        let _ = ws_tx.send(WsEvent::DeadlineApproaching {
                            block_id: block.id.clone(),
                            title: title.clone(),
                            note_id: note.id.as_str().to_string(),
                            deadline_iso: deadline_dt.to_rfc3339(),
                            lead_minutes: DEADLINE_LEAD_MIN,
                        });
                        debug!("notify: deadline approaching {}", block.id);
                    }
                }
            }

            if let Some(raw) = block.properties.get("scheduled") {
                if let Some(scheduled_dt) = parse_deadline_local(raw) {
                    let key = format!("{}|scheduled|{}", block.id, scheduled_dt.to_rfc3339());
                    // Scheduled fires at the exact time (no lead). One-minute
                    // scan granularity means the fire window is "any tick
                    // after the scheduled moment but within ~10 minutes."
                    let window_end = scheduled_dt + chrono::Duration::minutes(10);
                    if now >= scheduled_dt && now < window_end && !fired.contains(&key) {
                        fired.insert(key);
                        let _ = ws_tx.send(WsEvent::ScheduledFires {
                            block_id: block.id.clone(),
                            title: title.clone(),
                            note_id: note.id.as_str().to_string(),
                            scheduled_iso: scheduled_dt.to_rfc3339(),
                        });
                        debug!("notify: scheduled fires {}", block.id);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Parse `deadline::` / `scheduled::` value into a UTC datetime. Bare dates
/// are interpreted as "end of day in the user's local timezone" so a
/// deadline of `[[2026-05-09]]` notifies at 8 AM the same day (with the
/// 1h lead, that means 7 AM that morning). Adjust if user feedback wants
/// the lead computed from start-of-day instead.
fn parse_deadline_local(raw: &str) -> Option<DateTime<Utc>> {
    let trimmed = raw.trim();
    let (date_part, time_part) = match trimmed.find(' ') {
        Some(idx) => (
            trimmed[..idx].trim(),
            Some(trimmed[idx..].trim().to_string()),
        ),
        None => (trimmed, None),
    };
    let bare = date_part
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(date_part);
    let mut parts = bare.split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(y, m, d)?;

    let time = match time_part.as_deref() {
        Some(t) => parse_hhmm(t)?,
        // Bare-date deadline → 9 AM local. Common convention; matches how
        // most users mentally read "due today."
        None => NaiveTime::from_hms_opt(9, 0, 0)?,
    };
    let local_naive = NaiveDateTime::new(date, time);
    Local
        .from_local_datetime(&local_naive)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    NaiveTime::from_hms_opt(h, m, 0)
}

/// Strip the `#tag` tokens from the block's leading text — those are
/// noise in a notification title.
fn task_title(text: &str) -> String {
    text.split_whitespace()
        .filter(|tok| !tok.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

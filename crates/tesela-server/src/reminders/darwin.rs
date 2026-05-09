//! macOS EventKit bridge for Apple Reminders sync.
//!
//! v2 scope: push (Tesela → Reminders), pull (Reminders → Tesela), and
//! a combined `sync_all` that does pull-then-push so external edits
//! aren't clobbered by an immediate push.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{
    EKCalendar, EKEntityType, EKEventStore, EKReminder, EKSourceType,
};
use objc2_foundation::{NSArray, NSCalendar, NSCalendarUnit, NSDate, NSDateComponents, NSString};

use tesela_core::block::{parse_blocks, ParsedBlock};
use tesela_core::storage::markdown::parse_frontmatter;
use tesela_core::traits::note_store::NoteStore;

use super::{PullError, PullOutcome, PushError, PushOutcome, SyncOutcome};

/// Entry point — called from the `POST /api/sync/reminders/push` route.
///
/// v1 push: walk every Task block with a `deadline::` property, mirror
/// it to a reminder in the "Tesela" Reminders list, and write the
/// resulting `EKCalendarItem.calendarItemIdentifier` back to the block
/// as `apple_reminder_id::` so future syncs can find the same item.
///
/// Conflict resolution: out of scope. Last writer wins per field — i.e.
/// editing the title in Reminders.app will be clobbered on the next
/// push. Pull (slice 2) is what closes that loop.
pub async fn push_all(store: Arc<dyn NoteStore>) -> Result<PushOutcome> {
    request_access().await?;

    // Collect candidate blocks first (async, off the main runtime),
    // then push them on a blocking thread so EventKit's mainloop
    // doesn't fight with tokio.
    let candidates = collect_candidates(&store).await?;
    if candidates.is_empty() {
        return Ok(PushOutcome::default());
    }

    let outcome = tokio::task::spawn_blocking(move || -> Result<PushPlan> {
        let event_store = unsafe { EKEventStore::new() };
        let calendar = unsafe { find_or_create_tesela_calendar(&event_store)? };
        let mut plan = PushPlan::default();
        for cand in candidates {
            match unsafe { push_one(&event_store, &calendar, &cand) } {
                Ok(effect) => {
                    let synced_at = Utc::now().to_rfc3339();
                    let (reminder_id, was_created) = match effect {
                        SyncEffect::Created { reminder_id } => (reminder_id, true),
                        SyncEffect::Updated { reminder_id } => (reminder_id, false),
                    };
                    plan.writebacks.push(Writeback {
                        note_id: cand.note_id.clone(),
                        block_id: cand.block_id.clone(),
                        reminder_id,
                        synced_at,
                    });
                    if was_created {
                        plan.outcome.created.push(cand.block_id.clone());
                    } else {
                        plan.outcome.updated.push(cand.block_id.clone());
                    }
                    plan.outcome.synced.push(cand.block_id);
                }
                Err(e) => plan.outcome.errors.push(PushError {
                    block_id: cand.block_id,
                    message: e.to_string(),
                }),
            }
        }
        Ok(plan)
    })
    .await
    .map_err(|e| anyhow!("blocking task join failure: {e}"))??;

    // Apply writebacks one note at a time. Each note re-parses its body
    // after each prior insertion so block ids stay valid as line numbers
    // shift. New `apple_reminder_id::` lines get appended to the block's
    // continuation region.
    apply_writebacks(&store, &outcome.writebacks).await?;
    Ok(outcome.outcome)
}

#[derive(Default)]
struct PushPlan {
    outcome: PushOutcome,
    writebacks: Vec<Writeback>,
}

struct Writeback {
    note_id: String,
    block_id: String,
    reminder_id: String,
    synced_at: String,
}

enum SyncEffect {
    Created { reminder_id: String },
    Updated { reminder_id: String },
}

struct Candidate {
    note_id: String,
    block_id: String,
    title: String,
    deadline: Deadline,
    priority: u8,
    completed: bool,
    existing_reminder_id: Option<String>,
}

/// A `deadline::` value is a date with an optional time component.
/// Slice 3.1 round-trips both — push writes `dueDateComponents.hour`/
/// `minute` when present and pull reads them back into the same shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Deadline {
    date: NaiveDate,
    time: Option<NaiveTime>,
}

impl Deadline {
    /// Format for round-trip into the `deadline::` property. Mirrors
    /// what the user types: `[[YYYY-MM-DD]]` or `[[YYYY-MM-DD]] HH:MM`.
    fn format_property(&self) -> String {
        match self.time {
            Some(t) => format!("[[{}]] {}", self.date.format("%Y-%m-%d"), t.format("%H:%M")),
            None => format!("[[{}]]", self.date.format("%Y-%m-%d")),
        }
    }
}

/// Parse a `deadline::` value into a date + optional time. Accepts:
///   - `[[YYYY-MM-DD]]` / `YYYY-MM-DD`
///   - `[[YYYY-MM-DD]] HH:MM` / `YYYY-MM-DD HH:MM`
///   - `[[YYYY-MM-DD]] H:MM AM/PM` (12-hour form, case-insensitive)
fn parse_deadline(s: &str) -> Option<Deadline> {
    let trimmed = s.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let date_part = parts.next()?;
    let time_part = parts.next().map(str::trim);

    let date_str = date_part
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(date_part);
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;

    let time = time_part.and_then(parse_time_component);
    Some(Deadline { date, time })
}

fn parse_time_component(t: &str) -> Option<NaiveTime> {
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    NaiveTime::parse_from_str(t, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(t, "%I:%M %p"))
        .or_else(|_| NaiveTime::parse_from_str(&t.to_uppercase(), "%I:%M %p"))
        .ok()
}

async fn collect_candidates(store: &Arc<dyn NoteStore>) -> Result<Vec<Candidate>> {
    let notes = store
        .list(None, usize::MAX, 0)
        .await
        .map_err(|e| anyhow!("list notes: {e}"))?;
    let mut out = Vec::new();
    for note in notes {
        let body = extract_body(&note.content);
        let blocks = parse_blocks(note.id.as_str(), &body);
        for block in blocks {
            if !is_task(&block) {
                continue;
            }
            let Some(deadline_raw) = block.properties.get("deadline") else {
                continue;
            };
            let Some(deadline) = parse_deadline(deadline_raw) else {
                continue;
            };
            out.push(Candidate {
                note_id: note.id.to_string(),
                block_id: block.id.clone(),
                title: block.text.clone(),
                deadline,
                priority: priority_for(block.properties.get("priority").map(String::as_str)),
                completed: block
                    .properties
                    .get("status")
                    .map(|s| s.as_str() == "done")
                    .unwrap_or(false),
                existing_reminder_id: block
                    .properties
                    .get("apple_reminder_id")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            });
        }
    }
    Ok(out)
}

fn extract_body(content: &str) -> String {
    if !content.starts_with("---") {
        return content.to_string();
    }
    match parse_frontmatter(content) {
        Ok((_, body)) => body,
        Err(_) => content.to_string(),
    }
}

fn is_task(block: &ParsedBlock) -> bool {
    block
        .tags
        .iter()
        .chain(block.inherited_tags.iter())
        .any(|t| t.eq_ignore_ascii_case("task"))
}

fn priority_for(s: Option<&str>) -> u8 {
    match s.map(str::to_lowercase).as_deref() {
        Some("critical") | Some("high") => 1,
        Some("medium") => 5,
        Some("low") => 9,
        _ => 0,
    }
}

async fn apply_writebacks(store: &Arc<dyn NoteStore>, items: &[Writeback]) -> Result<()> {
    let mut by_note: HashMap<&str, Vec<&Writeback>> = HashMap::new();
    for wb in items {
        by_note.entry(wb.note_id.as_str()).or_default().push(wb);
    }
    for (note_id, writebacks) in by_note {
        let id = tesela_core::note::NoteId::new(note_id);
        let Some(mut note) = store
            .get(&id)
            .await
            .map_err(|e| anyhow!("get {note_id}: {e}"))?
        else {
            continue;
        };
        for wb in writebacks {
            note.content = upsert_block_property(
                &note.content,
                note_id,
                &wb.block_id,
                "apple_reminder_id",
                &wb.reminder_id,
            );
            note.content = upsert_block_property(
                &note.content,
                note_id,
                &wb.block_id,
                "apple_reminder_synced_at",
                &wb.synced_at,
            );
        }
        store
            .update(&note)
            .await
            .map_err(|e| anyhow!("update {note_id}: {e}"))?;
    }
    Ok(())
}

/// Inserts (or replaces) a `key:: value` continuation line on the block
/// matching `block_id`. Re-parses each call so line-number shifts from
/// prior insertions are honored.
fn upsert_block_property(
    content: &str,
    note_id: &str,
    block_id: &str,
    key: &str,
    value: &str,
) -> String {
    let Some((fm, body)) = split_frontmatter(content) else {
        return content.to_string();
    };
    let blocks = parse_blocks(note_id, body);
    let Some(target) = blocks.iter().find(|b| b.id == block_id) else {
        return content.to_string();
    };

    // Block id is `{note_id}:{0-indexed-line-num-in-body}`. The block
    // spans from its start line to (next block's line - 1), or EOF if
    // it's the last block.
    let block_start_idx: usize = target
        .id
        .rsplit_once(':')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(0);
    let next_block_line = blocks
        .iter()
        .filter_map(|b| b.id.rsplit_once(':').and_then(|(_, n)| n.parse::<usize>().ok()))
        .filter(|n| *n > block_start_idx)
        .min();

    let mut lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
    let block_end = match next_block_line {
        Some(n) if n > 0 => n - 1,
        _ => lines.len().saturating_sub(1),
    };

    // Try replacing an existing key on this block.
    let mut replaced = false;
    for i in block_start_idx + 1..=block_end.min(lines.len().saturating_sub(1)) {
        let trimmed = lines[i].trim_start();
        if let Some(rest) = trimmed.strip_prefix(&format!("{key}::"))
        {
            let indent = &lines[i][..lines[i].len() - trimmed.len()];
            let _ = rest; // keep for readability — value is always rewritten
            lines[i] = format!("{indent}{key}:: {value}");
            replaced = true;
            break;
        }
    }
    if !replaced {
        // Append after the block's existing continuation lines, using
        // the indentation of the block-start line + 2 spaces.
        let lead = lines[block_start_idx]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect::<String>();
        let new_line = format!("{lead}  {key}:: {value}");
        let insert_at = (block_end + 1).min(lines.len());
        lines.insert(insert_at, new_line);
    }
    let new_body = lines.join("\n");
    format!("{fm}{new_body}")
}

fn split_frontmatter(content: &str) -> Option<(String, &str)> {
    if !content.starts_with("---") {
        return Some((String::new(), content));
    }
    let after_first = &content[3..];
    let end = after_first.find("\n---")?;
    // Include the trailing `---\n` in the frontmatter portion so we can
    // reassemble cleanly without losing it.
    let fm_end = 3 + end + 4; // first "---" + body before "---" + "\n---"
    let mut idx = fm_end;
    if content.as_bytes().get(idx) == Some(&b'\n') {
        idx += 1;
    }
    Some((content[..idx].to_string(), &content[idx..]))
}

unsafe fn push_one(
    event_store: &EKEventStore,
    calendar: &EKCalendar,
    cand: &Candidate,
) -> Result<SyncEffect> {
    let reminder = if let Some(existing_id) = &cand.existing_reminder_id {
        let id_ns = NSString::from_str(existing_id);
        match event_store.calendarItemWithIdentifier(&id_ns) {
            Some(item) => {
                // Returned as EKCalendarItem; downcast via Retained.
                Retained::downcast::<EKReminder>(item).map_err(|_| {
                    anyhow!("calendar item {existing_id} is not an EKReminder")
                })?
            }
            None => EKReminder::reminderWithEventStore(event_store),
        }
    } else {
        EKReminder::reminderWithEventStore(event_store)
    };

    let title = NSString::from_str(&cand.title);
    reminder.setTitle(Some(&title));
    reminder.setCalendar(Some(calendar));
    reminder.setPriority(cand.priority as usize);
    reminder.setCompleted(cand.completed);
    reminder.setDueDateComponents(Some(&date_components(cand.deadline)));

    event_store
        .saveReminder_commit_error(&reminder, true)
        .map_err(|nserr| anyhow!("save reminder: {}", nserr.localizedDescription()))?;

    let id = reminder.calendarItemIdentifier().to_string();
    if cand.existing_reminder_id.is_none() {
        Ok(SyncEffect::Created { reminder_id: id })
    } else {
        Ok(SyncEffect::Updated { reminder_id: id })
    }
}

fn date_components(d: Deadline) -> Retained<NSDateComponents> {
    use chrono::{Datelike, Timelike};
    let dc = NSDateComponents::new();
    dc.setYear(d.date.year() as isize);
    dc.setMonth(d.date.month() as isize);
    dc.setDay(d.date.day() as isize);
    if let Some(t) = d.time {
        dc.setHour(t.hour() as isize);
        dc.setMinute(t.minute() as isize);
    }
    // Touched to keep the explicit imports warning-free; both APIs are
    // used elsewhere in this module via the FFI layer.
    let _ = NSCalendar::currentCalendar();
    let _ = NSCalendarUnit::Year;
    dc
}

/// Request permission for Reminders access via the EventKit completion
/// callback. Bridges the callback to a tokio oneshot so the rest of the
/// pipeline can `.await` the result.
async fn request_access() -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<bool, String>>();
    let tx = std::sync::Mutex::new(Some(tx));

    // Spawn the EventKit call on the blocking pool — the completion
    // handler may fire on EventKit's internal queue, not our tokio
    // worker, and we want the async runtime free in the meantime.
    tokio::task::spawn_blocking(move || {
        let event_store = unsafe { EKEventStore::new() };
        let block = block2::RcBlock::new(move |granted: Bool, err: *mut objc2_foundation::NSError| {
            let mut guard = tx.lock().unwrap();
            if let Some(sender) = guard.take() {
                if !err.is_null() {
                    let nserr: &objc2_foundation::NSError = unsafe { &*err };
                    let desc = nserr.localizedDescription().to_string();
                    let _ = sender.send(Err(desc));
                } else {
                    let _ = sender.send(Ok(granted.as_bool()));
                }
            }
        });

        // macOS 14+ — modern API for Reminders permission. EventKit's
        // older requestAccessToEntityType: is now deprecated; we use
        // the dedicated reminders entry point so the prompt copy and
        // privacy panel descriptor match.
        let block_ptr = block2::RcBlock::as_ptr(&block);
        unsafe {
            event_store.requestFullAccessToRemindersWithCompletion(block_ptr);
        }
        // Keep the RcBlock alive past the call — the completion handler
        // may fire on a worker queue after this scope exits, and we
        // don't want the block dropped before then.
        std::mem::forget(block);
    });

    // Wait up to 60s for the permission dialog. If the user never
    // responds, fail loudly rather than blocking the request thread
    // forever.
    let granted = tokio::time::timeout(Duration::from_secs(60), rx)
        .await
        .map_err(|_| anyhow!("timed out waiting for Reminders permission"))?
        .map_err(|_| anyhow!("permission request channel dropped"))?
        .map_err(|e| anyhow!("EventKit error: {e}"))?;

    if !granted {
        anyhow::bail!("Reminders access denied. Grant in System Settings → Privacy & Security → Reminders.");
    }
    Ok(())
}

/// Walks the user's reminder calendars and returns the one titled
/// "Tesela", creating it on the first writable Source if missing.
unsafe fn find_or_create_tesela_calendar(
    event_store: &EKEventStore,
) -> Result<Retained<EKCalendar>> {
    let target = NSString::from_str("Tesela");
    let calendars: Retained<NSArray<EKCalendar>> =
        event_store.calendarsForEntityType(EKEntityType::Reminder);

    for cal in calendars.iter() {
        let title = cal.title();
        if title.isEqualToString(&target) {
            return Ok(cal);
        }
    }

    // Picking a Source that supports reminders is fiddlier than it
    // looks — `EKSource.sourceType` doesn't tell us whether a Source
    // actually accepts reminder-type calendars (CalDAV servers that
    // host events only will reject `saveCalendar:`). The cheapest way
    // to find a known-good Source is to pull it off whatever calendar
    // EventKit picks for new reminders by default.
    let source = if let Some(default_cal) = event_store.defaultCalendarForNewReminders() {
        default_cal
            .source()
            .ok_or_else(|| anyhow!("default reminders calendar has no source"))?
    } else {
        // Fallback: find any Source whose sourceType is one of the
        // ones that can host reminders. Prefer Local for headless
        // boxes; iCloud (`MobileMe` historically, now CalDAV with a
        // specific identifier) for typical user installs.
        let sources = event_store.sources();
        let mut chosen = None;
        for src in sources.iter() {
            let st = src.sourceType();
            if matches!(
                st,
                EKSourceType::Local | EKSourceType::CalDAV | EKSourceType::MobileMe
            ) {
                chosen = Some(src);
                break;
            }
        }
        chosen.ok_or_else(|| anyhow!("no writable EventKit source for reminders"))?
    };

    let new_cal: Retained<EKCalendar> =
        EKCalendar::calendarForEntityType_eventStore(EKEntityType::Reminder, event_store);
    new_cal.setTitle(&target);
    new_cal.setSource(Some(&source));

    event_store
        .saveCalendar_commit_error(&new_cal, true)
        .map_err(|nserr| anyhow!("save Tesela calendar: {}", nserr.localizedDescription()))?;
    Ok(new_cal)
}

// ─────────────────────────────────────────────────────────────────────
// Pull (Reminders → Tesela)
// ─────────────────────────────────────────────────────────────────────

/// Pull all reminders in the Tesela calendar back into Tesela. Walks
/// every reminder, looks up its matching block via the
/// `apple_reminder_id::` property, and writes back any field that's
/// drifted (status, deadline, priority, title) — gated on the EK
/// `lastModifiedDate` being newer than the block's
/// `apple_reminder_synced_at::`.
pub async fn pull_all(store: Arc<dyn NoteStore>) -> Result<PullOutcome> {
    request_access().await?;

    let snapshots = fetch_reminders().await?;
    if snapshots.is_empty() {
        return Ok(PullOutcome::default());
    }

    let index = collect_block_index(&store).await?;
    let mut outcome = PullOutcome::default();
    let mut writebacks: Vec<PullWriteback> = Vec::new();

    for snap in snapshots {
        match index.get(&snap.reminder_id) {
            Some(block) => {
                let diff = compute_diff(&snap, block);
                if diff.is_empty() {
                    continue;
                }
                writebacks.push(PullWriteback {
                    note_id: block.note_id.clone(),
                    block_id: block.block_id.clone(),
                    diff,
                    synced_at: Utc::now().to_rfc3339(),
                });
                outcome.updated.push(block.block_id.clone());
            }
            None => outcome.orphans.push(snap.reminder_id.clone()),
        }
    }

    if let Err(e) = apply_pull_writebacks(&store, &writebacks).await {
        // Surface the error per-block so partial progress is still
        // recorded. The note write may have partly succeeded for some
        // notes before failing; rather than guess we attribute the
        // error to every block in the failing batch.
        for wb in &writebacks {
            outcome.errors.push(PullError {
                reminder_id: wb.block_id.clone(),
                message: e.to_string(),
            });
        }
    }

    Ok(outcome)
}

/// Combined pull-then-push. The "Sync now" button hits this so external
/// edits flow back into Tesela before any push could clobber them.
pub async fn sync_all(store: Arc<dyn NoteStore>) -> Result<SyncOutcome> {
    let pull = pull_all(Arc::clone(&store)).await.unwrap_or_else(|e| {
        // If the pull half fails, surface as a single error and let
        // push run anyway — losing one direction is better than losing
        // both.
        let mut o = PullOutcome::default();
        o.errors.push(PullError {
            reminder_id: String::new(),
            message: format!("pull failed: {e}"),
        });
        o
    });
    let push = push_all(store).await?;
    Ok(SyncOutcome { pull, push })
}

#[derive(Default)]
struct PullDiff {
    title: Option<String>,
    status: Option<String>,
    deadline: Option<Deadline>,
    priority: Option<String>,
}

impl PullDiff {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.status.is_none()
            && self.deadline.is_none()
            && self.priority.is_none()
    }
}

struct PullWriteback {
    note_id: String,
    block_id: String,
    diff: PullDiff,
    synced_at: String,
}

struct ReminderSnapshot {
    reminder_id: String,
    title: String,
    completed: bool,
    due_deadline: Option<Deadline>,
    priority: u8,
    /// EK `lastModifiedDate` as Unix millis — used to decide whether the
    /// reminder has been touched since our last sync.
    last_modified_unix_ms: Option<i64>,
}

struct BlockRef {
    note_id: String,
    block_id: String,
    title: String,
    status: Option<String>,
    deadline: Option<Deadline>,
    priority_str: Option<String>,
    synced_at_unix_ms: Option<i64>,
}

async fn collect_block_index(
    store: &Arc<dyn NoteStore>,
) -> Result<HashMap<String, BlockRef>> {
    let notes = store
        .list(None, usize::MAX, 0)
        .await
        .map_err(|e| anyhow!("list notes: {e}"))?;
    let mut idx = HashMap::new();
    for note in notes {
        let body = extract_body(&note.content);
        let blocks = parse_blocks(note.id.as_str(), &body);
        for block in blocks {
            let Some(rid) = block
                .properties
                .get("apple_reminder_id")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let synced_at_unix_ms = block
                .properties
                .get("apple_reminder_synced_at")
                .and_then(|s| DateTime::parse_from_rfc3339(s.trim()).ok())
                .map(|dt| dt.timestamp_millis());
            idx.insert(
                rid,
                BlockRef {
                    note_id: note.id.to_string(),
                    block_id: block.id.clone(),
                    title: block.text.clone(),
                    status: block.properties.get("status").cloned(),
                    deadline: block
                        .properties
                        .get("deadline")
                        .and_then(|s| parse_deadline(s)),
                    priority_str: block.properties.get("priority").cloned(),
                    synced_at_unix_ms,
                },
            );
        }
    }
    Ok(idx)
}

fn compute_diff(snap: &ReminderSnapshot, block: &BlockRef) -> PullDiff {
    let mut diff = PullDiff::default();

    // Conflict gate: if EK hasn't been touched since our last sync, the
    // EK side has nothing newer to offer — Tesela's value (which may have
    // diverged via local edits) wins by default.
    if let (Some(synced), Some(modified)) =
        (block.synced_at_unix_ms, snap.last_modified_unix_ms)
    {
        if modified <= synced {
            return diff;
        }
    }

    if !snap.title.is_empty() && snap.title != block.title {
        diff.title = Some(snap.title.clone());
    }

    let target_status = if snap.completed { "done" } else { "todo" };
    if block.status.as_deref() != Some(target_status) {
        diff.status = Some(target_status.to_string());
    }

    // Only sync the deadline EK→Tesela when EK has a value. Don't
    // clear Tesela deadlines from the pull side — that would be
    // surprising and there's no clean way to delete a property line in
    // upsert_block_property right now.
    if let Some(due) = snap.due_deadline {
        if Some(due) != block.deadline {
            diff.deadline = Some(due);
        }
    }

    let target_priority = match snap.priority {
        1..=4 => Some("high"),
        5 => Some("medium"),
        6..=9 => Some("low"),
        _ => None,
    };
    if let Some(target) = target_priority {
        if block.priority_str.as_deref() != Some(target) {
            diff.priority = Some(target.to_string());
        }
    }

    diff
}

async fn apply_pull_writebacks(
    store: &Arc<dyn NoteStore>,
    items: &[PullWriteback],
) -> Result<()> {
    let mut by_note: HashMap<&str, Vec<&PullWriteback>> = HashMap::new();
    for wb in items {
        by_note.entry(wb.note_id.as_str()).or_default().push(wb);
    }
    for (note_id, writebacks) in by_note {
        let id = tesela_core::note::NoteId::new(note_id);
        let Some(mut note) = store
            .get(&id)
            .await
            .map_err(|e| anyhow!("get {note_id}: {e}"))?
        else {
            continue;
        };
        for wb in writebacks {
            if let Some(new_title) = &wb.diff.title {
                note.content = set_block_text(&note.content, note_id, &wb.block_id, new_title);
            }
            if let Some(status) = &wb.diff.status {
                note.content = upsert_block_property(
                    &note.content,
                    note_id,
                    &wb.block_id,
                    "status",
                    status,
                );
            }
            if let Some(deadline) = wb.diff.deadline {
                note.content = upsert_block_property(
                    &note.content,
                    note_id,
                    &wb.block_id,
                    "deadline",
                    &deadline.format_property(),
                );
            }
            if let Some(priority) = &wb.diff.priority {
                note.content = upsert_block_property(
                    &note.content,
                    note_id,
                    &wb.block_id,
                    "priority",
                    priority,
                );
            }
            note.content = upsert_block_property(
                &note.content,
                note_id,
                &wb.block_id,
                "apple_reminder_synced_at",
                &wb.synced_at,
            );
        }
        store
            .update(&note)
            .await
            .map_err(|e| anyhow!("update {note_id}: {e}"))?;
    }
    Ok(())
}

/// Rewrite the start line of a block to display `new_text`. Inline
/// `#tag` tokens on the line are preserved by re-appending them after
/// the new text — Tesela's `block.text` is the line with tags stripped,
/// so a naive overwrite would silently lose the tag.
fn set_block_text(content: &str, note_id: &str, block_id: &str, new_text: &str) -> String {
    let Some((fm, body)) = split_frontmatter(content) else {
        return content.to_string();
    };
    let blocks = parse_blocks(note_id, body);
    let Some(target) = blocks.iter().find(|b| b.id == block_id) else {
        return content.to_string();
    };
    let block_start_idx: usize = target
        .id
        .rsplit_once(':')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(0);

    let mut lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
    if block_start_idx >= lines.len() {
        return content.to_string();
    }
    let line = lines[block_start_idx].clone();
    let Some(bullet_pos) = line.find("- ") else {
        return content.to_string();
    };
    let prefix = &line[..bullet_pos + 2];
    let body_part = &line[bullet_pos + 2..];
    let inline_tags: Vec<&str> = body_part
        .split_whitespace()
        .filter(|w| w.starts_with('#'))
        .collect();
    let mut rebuilt = format!("{prefix}{new_text}");
    for tag in inline_tags {
        rebuilt.push(' ');
        rebuilt.push_str(tag);
    }
    lines[block_start_idx] = rebuilt;
    let new_body = lines.join("\n");
    format!("{fm}{new_body}")
}

async fn fetch_reminders() -> Result<Vec<ReminderSnapshot>> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Vec<ReminderSnapshot>, String>>();
    let tx = std::sync::Mutex::new(Some(tx));

    tokio::task::spawn_blocking(move || {
        let event_store = unsafe { EKEventStore::new() };
        let calendar = match unsafe { find_or_create_tesela_calendar(&event_store) } {
            Ok(c) => c,
            Err(e) => {
                if let Some(s) = tx.lock().unwrap().take() {
                    let _ = s.send(Err(format!("calendar lookup: {e}")));
                }
                return;
            }
        };

        // Wrap our single calendar in an NSArray for the predicate.
        let cal_array = NSArray::from_retained_slice(&[calendar]);
        let predicate =
            unsafe { event_store.predicateForRemindersInCalendars(Some(&cal_array)) };

        let block = block2::RcBlock::new(
            move |reminders_ptr: *mut NSArray<EKReminder>| {
                let mut snapshots = Vec::new();
                if !reminders_ptr.is_null() {
                    let reminders: &NSArray<EKReminder> = unsafe { &*reminders_ptr };
                    for rem in reminders.iter() {
                        snapshots.push(unsafe { snapshot_reminder(&rem) });
                    }
                }
                if let Some(s) = tx.lock().unwrap().take() {
                    let _ = s.send(Ok(snapshots));
                }
            },
        );

        // The completion handler is held by EventKit until it fires. We
        // forget the RcBlock so it isn't dropped when this scope exits;
        // the heap-allocated block remains alive for EventKit.
        unsafe {
            let _request = event_store
                .fetchRemindersMatchingPredicate_completion(&predicate, &block);
        }
        std::mem::forget(block);
    });

    let snapshots = tokio::time::timeout(Duration::from_secs(60), rx)
        .await
        .map_err(|_| anyhow!("timed out fetching reminders"))?
        .map_err(|_| anyhow!("fetch channel dropped"))?
        .map_err(|e| anyhow!("EventKit fetch: {e}"))?;

    Ok(snapshots)
}

unsafe fn snapshot_reminder(rem: &EKReminder) -> ReminderSnapshot {
    let reminder_id = rem.calendarItemIdentifier().to_string();
    let title = rem
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    let completed = rem.isCompleted();
    let priority = rem.priority() as u8;
    let due_deadline = rem.dueDateComponents().and_then(|dc| {
        let y = dc.year();
        let m = dc.month();
        let d = dc.day();
        // NSDateComponents uses a sentinel (NSDateComponentUndefined =
        // NSIntegerMax) when a field isn't set. A negative or zero day
        // is also a "no date" signal; treat anything that doesn't make
        // a valid Gregorian date as None.
        if y > 0 && m > 0 && d > 0 {
            let date = NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32)?;
            // Hour/minute may be NSDateComponentUndefined (== NSIntegerMax)
            // when the user picked a date-only reminder. Bound-check
            // before constructing a NaiveTime — the sentinel is way
            // outside 0..24 / 0..60.
            let h = dc.hour();
            let mn = dc.minute();
            let time = if (0..24).contains(&h) && (0..60).contains(&mn) {
                NaiveTime::from_hms_opt(h as u32, mn as u32, 0)
            } else {
                None
            };
            Some(Deadline { date, time })
        } else {
            None
        }
    });
    let last_modified_unix_ms = rem.lastModifiedDate().map(|d| ns_date_to_unix_ms(&d));

    ReminderSnapshot {
        reminder_id,
        title,
        completed,
        due_deadline,
        priority,
        last_modified_unix_ms,
    }
}

/// `NSDate` is seconds since the macOS reference date (2001-01-01
/// 00:00 UTC). Convert to Unix epoch millis so we can compare against
/// `chrono::DateTime::timestamp_millis()`.
fn ns_date_to_unix_ms(d: &NSDate) -> i64 {
    let mac_ref_seconds = d.timeIntervalSinceReferenceDate();
    let unix_ref_seconds = 978_307_200.0_f64; // 2001-01-01T00:00:00Z in unix time
    ((mac_ref_seconds + unix_ref_seconds) * 1000.0) as i64
}

/// Helpers from EKReminder's superclass (EKCalendarItem) and EKObject
/// that the generated bindings don't surface directly on the
/// `EKReminder` type.
#[allow(non_snake_case)]
trait EKReminderExt {
    unsafe fn title(&self) -> Option<Retained<NSString>>;
    unsafe fn lastModifiedDate(&self) -> Option<Retained<NSDate>>;
}
#[allow(non_snake_case)]
impl EKReminderExt for EKReminder {
    unsafe fn title(&self) -> Option<Retained<NSString>> {
        use objc2::msg_send;
        unsafe { msg_send![self, title] }
    }
    unsafe fn lastModifiedDate(&self) -> Option<Retained<NSDate>> {
        use objc2::msg_send;
        unsafe { msg_send![self, lastModifiedDate] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
    fn time(h: u32, mn: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, mn, 0).unwrap()
    }

    #[test]
    fn parse_deadline_bracketed_date_only() {
        assert_eq!(
            parse_deadline("[[2026-05-08]]"),
            Some(Deadline { date: date(2026, 5, 8), time: None })
        );
    }

    #[test]
    fn parse_deadline_bracketed_with_24h_time() {
        // The bug we shipped 12.1 with — `]] 10:00` made the suffix-strip
        // miss because it ran on the whole trimmed string. Now we
        // tokenize first.
        assert_eq!(
            parse_deadline("[[2026-05-08]] 10:00"),
            Some(Deadline { date: date(2026, 5, 8), time: Some(time(10, 0)) })
        );
    }

    #[test]
    fn parse_deadline_unbracketed() {
        assert_eq!(
            parse_deadline("2026-05-08"),
            Some(Deadline { date: date(2026, 5, 8), time: None })
        );
        assert_eq!(
            parse_deadline("2026-05-08 14:30"),
            Some(Deadline { date: date(2026, 5, 8), time: Some(time(14, 30)) })
        );
    }

    #[test]
    fn parse_deadline_12h_am_pm() {
        assert_eq!(
            parse_deadline("[[2026-05-08]] 9:30 AM"),
            Some(Deadline { date: date(2026, 5, 8), time: Some(time(9, 30)) })
        );
        assert_eq!(
            parse_deadline("[[2026-05-08]] 9:30 pm"),
            Some(Deadline { date: date(2026, 5, 8), time: Some(time(21, 30)) })
        );
    }

    #[test]
    fn parse_deadline_garbage_returns_none() {
        assert_eq!(parse_deadline(""), None);
        assert_eq!(parse_deadline("nonsense"), None);
        assert_eq!(parse_deadline("[[2026-13-99]]"), None);
    }

    #[test]
    fn deadline_format_round_trip() {
        let date_only = Deadline { date: date(2026, 5, 8), time: None };
        assert_eq!(date_only.format_property(), "[[2026-05-08]]");
        let with_time = Deadline { date: date(2026, 5, 8), time: Some(time(10, 0)) };
        assert_eq!(with_time.format_property(), "[[2026-05-08]] 10:00");
    }
}


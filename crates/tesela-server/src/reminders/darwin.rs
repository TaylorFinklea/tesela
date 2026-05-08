//! macOS EventKit bridge for Apple Reminders sync.
//!
//! v1 scope: smoke test + permission flow + Tesela calendar create/find.
//! The block-iteration / writeback / property mapping lands in the next
//! slice once we've validated the FFI and permission UX work end-to-end.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{
    EKCalendar, EKEntityType, EKEventStore, EKReminder, EKSourceType,
};
use objc2_foundation::{NSArray, NSCalendar, NSCalendarUnit, NSDateComponents, NSString};

use tesela_core::block::{parse_blocks, ParsedBlock};
use tesela_core::storage::markdown::parse_frontmatter;
use tesela_core::traits::note_store::NoteStore;

use super::{PushError, PushOutcome};

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
                Ok(SyncEffect::Created { reminder_id }) => {
                    plan.writebacks.push(Writeback {
                        note_id: cand.note_id.clone(),
                        block_id: cand.block_id.clone(),
                        reminder_id,
                    });
                    plan.outcome.created.push(cand.block_id.clone());
                    plan.outcome.synced.push(cand.block_id);
                }
                Ok(SyncEffect::Updated) => {
                    plan.outcome.updated.push(cand.block_id.clone());
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
}

enum SyncEffect {
    Created { reminder_id: String },
    Updated,
}

struct Candidate {
    note_id: String,
    block_id: String,
    title: String,
    deadline: NaiveDate,
    priority: u8,
    completed: bool,
    existing_reminder_id: Option<String>,
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
            let Some(deadline) = parse_iso_date_brackets(deadline_raw) else {
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

fn parse_iso_date_brackets(s: &str) -> Option<NaiveDate> {
    let trimmed = s.trim();
    let inner = trimmed
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(trimmed);
    // Strip trailing time component if present (e.g. `2026-05-08 10:00`).
    let date_part = inner.split_whitespace().next().unwrap_or(inner);
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
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
    use std::collections::HashMap;
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

    let was_new = cand.existing_reminder_id.is_none();
    if was_new {
        let id = reminder.calendarItemIdentifier().to_string();
        Ok(SyncEffect::Created { reminder_id: id })
    } else {
        Ok(SyncEffect::Updated)
    }
}

fn date_components(date: NaiveDate) -> Retained<NSDateComponents> {
    use chrono::Datelike;
    let dc = NSDateComponents::new();
    dc.setYear(date.year() as isize);
    dc.setMonth(date.month() as isize);
    dc.setDay(date.day() as isize);
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

